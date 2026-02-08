//! Semantic validation pass for the compiled IR.
//!
//! Walks the [`CompiledComponent`] tree and collects
//! [`ValidationWarning`]s — non-fatal diagnostics that highlight
//! likely mistakes without preventing code generation.

use std::collections::HashMap;

use thebe_ast::Span;

use crate::ir::CompiledComponent;
use crate::ir::EventModifier;
use crate::ir::IrComponentRef;
use crate::ir::IrElement;
use crate::ir::IrNode;
use crate::warning::ValidationWarning;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate a compiled component and return all warnings.
///
/// An empty `Vec` means no problems were detected.
#[must_use]
pub fn validate(component: &CompiledComponent) -> Vec<ValidationWarning> {
  let mut ctx = ValidationCtx::default();
  ctx.validate_nodes(&component.template);
  ctx.warnings
}

// ---------------------------------------------------------------------------
// Internal context
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ValidationCtx {
  warnings: Vec<ValidationWarning>,
  /// Tracks default slot spans across the entire component.
  default_slot: Option<Span>,
  /// Tracks named slot spans across the entire component.
  named_slots: HashMap<String, Span>,
}

impl ValidationCtx {
  fn validate_nodes(&mut self, nodes: &[IrNode]) {
    for node in nodes {
      self.validate_node(node);
    }
  }

  fn validate_node(&mut self, node: &IrNode) {
    match node {
      IrNode::Element(el) => self.validate_element(el),
      IrNode::Component(comp) => self.validate_component(comp),
      IrNode::Expr(e) => self.check_empty_expr(&e.expr, e.span),
      IrNode::If(ib) => {
        for branch in &ib.branches {
          self.validate_nodes(&branch.children);
        }
      }
      IrNode::Each(each) => self.validate_nodes(&each.children),
      IrNode::Slot(slot) => {
        self.check_slot_duplicates(slot.name.as_deref(), slot.span);
        self.validate_nodes(&slot.fallback);
      }
      IrNode::Text(_) => {}
    }
  }

  // ── Element validation ────────────────────────────────────────────

  fn validate_element(&mut self, el: &IrElement) {
    self.check_duplicate_attrs(&el.tag, &el.attributes);
    self.check_duplicate_events(&el.tag, &el.events);
    self.check_duplicate_bindings(&el.tag, &el.bindings);
    self.check_duplicate_class_toggles(&el.tag, &el.class_toggles);
    self.check_duplicate_style_props(&el.tag, &el.style_props);
    self.check_event_modifiers(&el.events);
    self.check_empty_handlers(&el.events);
    self.validate_nodes(&el.children);
  }

  fn validate_component(&mut self, comp: &IrComponentRef) {
    self.check_duplicate_attrs(&comp.name, &comp.props);
    self.check_duplicate_events(&comp.name, &comp.events);
    self.check_duplicate_bindings(&comp.name, &comp.bindings);
    self.check_event_modifiers(&comp.events);
    self.check_empty_handlers(&comp.events);
    self.validate_nodes(&comp.children);
  }

  // ── Duplicate checks ─────────────────────────────────────────────

  fn check_duplicate_attrs(
    &mut self,
    tag: &str,
    attrs: &[crate::ir::IrAttribute],
  ) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for attr in attrs {
      if let Some(&first_span) = seen.get(attr.name.as_str()) {
        self.warnings.push(ValidationWarning::DuplicateAttribute {
          name: attr.name.clone(),
          tag: tag.to_string(),
          first_span,
          dup_span: attr.span,
        });
      } else {
        seen.insert(&attr.name, attr.span);
      }
    }
  }

  fn check_duplicate_events(
    &mut self,
    tag: &str,
    events: &[crate::ir::IrEventHandler],
  ) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for ev in events {
      if let Some(&first_span) = seen.get(ev.event.as_str()) {
        self.warnings.push(ValidationWarning::DuplicateEventHandler {
          event: ev.event.clone(),
          tag: tag.to_string(),
          first_span,
          dup_span: ev.span,
        });
      } else {
        seen.insert(&ev.event, ev.span);
      }
    }
  }

  fn check_duplicate_bindings(
    &mut self,
    tag: &str,
    bindings: &[crate::ir::IrBinding],
  ) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for b in bindings {
      if let Some(&first_span) = seen.get(b.property.as_str()) {
        self.warnings.push(ValidationWarning::DuplicateBinding {
          property: b.property.clone(),
          tag: tag.to_string(),
          first_span,
          dup_span: b.span,
        });
      } else {
        seen.insert(&b.property, b.span);
      }
    }
  }

  fn check_duplicate_class_toggles(
    &mut self,
    tag: &str,
    toggles: &[crate::ir::IrClassToggle],
  ) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for t in toggles {
      if let Some(&first_span) = seen.get(t.class.as_str()) {
        self.warnings.push(ValidationWarning::DuplicateClassToggle {
          class: t.class.clone(),
          tag: tag.to_string(),
          first_span,
          dup_span: t.span,
        });
      } else {
        seen.insert(&t.class, t.span);
      }
    }
  }

  fn check_duplicate_style_props(
    &mut self,
    tag: &str,
    props: &[crate::ir::IrStyleProp],
  ) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for p in props {
      if let Some(&first_span) = seen.get(p.property.as_str()) {
        self.warnings.push(ValidationWarning::DuplicateStyleProp {
          property: p.property.clone(),
          tag: tag.to_string(),
          first_span,
          dup_span: p.span,
        });
      } else {
        seen.insert(&p.property, p.span);
      }
    }
  }

  // ── Modifier conflict checks ─────────────────────────────────────

  fn check_event_modifiers(&mut self, events: &[crate::ir::IrEventHandler]) {
    for ev in events {
      let has_passive = ev.modifiers.contains(&EventModifier::Passive);
      let has_prevent = ev.modifiers.contains(&EventModifier::PreventDefault);
      let has_non_passive = ev.modifiers.contains(&EventModifier::NonPassive);

      if has_passive && has_prevent {
        self
          .warnings
          .push(ValidationWarning::ConflictingPassivePreventDefault {
            event: ev.event.clone(),
            span: ev.span,
          });
      }
      if has_passive && has_non_passive {
        self
          .warnings
          .push(ValidationWarning::ConflictingPassiveNonPassive {
            event: ev.event.clone(),
            span: ev.span,
          });
      }
    }
  }

  // ── Empty value checks ────────────────────────────────────────────

  fn check_empty_expr(&mut self, expr: &str, span: Span) {
    if expr.trim().is_empty() {
      self.warnings.push(ValidationWarning::EmptyExpression { span });
    }
  }

  fn check_empty_handlers(&mut self, events: &[crate::ir::IrEventHandler]) {
    for ev in events {
      if ev.handler.trim().is_empty() {
        self.warnings.push(ValidationWarning::EmptyEventHandler {
          event: ev.event.clone(),
          span: ev.span,
        });
      }
    }
  }

  // ── Slot duplicate checks ────────────────────────────────────────

  fn check_slot_duplicates(&mut self, name: Option<&str>, span: Span) {
    match name {
      None => {
        if let Some(first_span) = self.default_slot {
          self.warnings.push(ValidationWarning::MultipleDefaultSlots {
            first_span,
            dup_span: span,
          });
        } else {
          self.default_slot = Some(span);
        }
      }
      Some(n) => {
        if let Some(&first_span) = self.named_slots.get(n) {
          self.warnings.push(ValidationWarning::DuplicateNamedSlot {
            name: n.to_string(),
            first_span,
            dup_span: span,
          });
        } else {
          self.named_slots.insert(n.to_string(), span);
        }
      }
    }
  }
}
