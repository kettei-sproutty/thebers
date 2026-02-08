//! Semantic validation pass for the compiled IR.
//!
//! Walks the [`CompiledComponent`] tree and collects
//! [`ValidationWarning`]s — non-fatal diagnostics that highlight
//! likely mistakes without preventing code generation.

use std::collections::HashMap;

use thebe_ast::Span;

use crate::ir::CompiledComponent;
use crate::ir::EventModifier;
use crate::ir::IrAttribute;
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
      IrNode::Expr(e) | IrNode::RawHtml(e) => self.check_empty_expr(&e.expr, e.span),
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
    self.check_empty_style_prop_values(&el.style_props);
    self.check_empty_class_toggle_conditions(&el.class_toggles);
    self.check_empty_binding_expressions(&el.bindings);
    self.check_slot_attribute(&el.tag, &el.attributes);
    self.validate_nodes(&el.children);
  }

  fn validate_component(&mut self, comp: &IrComponentRef) {
    self.check_duplicate_attrs(&comp.name, &comp.props);
    self.check_duplicate_events(&comp.name, &comp.events);
    self.check_duplicate_bindings(&comp.name, &comp.bindings);
    self.check_event_modifiers(&comp.events);
    self.check_empty_handlers(&comp.events);
    self.check_empty_binding_expressions(&comp.bindings);
    self.check_empty_component_props(comp);
    self.check_directives_on_component(comp);
    self.check_slot_attribute(&comp.name, &comp.props);
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

  fn check_empty_style_prop_values(&mut self, props: &[crate::ir::IrStyleProp]) {
    for p in props {
      if p.value.trim().is_empty() {
        self
          .warnings
          .push(ValidationWarning::EmptyStylePropValue {
            property: p.property.clone(),
            span: p.span,
          });
      }
    }
  }

  fn check_empty_class_toggle_conditions(
    &mut self,
    toggles: &[crate::ir::IrClassToggle],
  ) {
    for t in toggles {
      if t.condition.trim().is_empty() {
        self
          .warnings
          .push(ValidationWarning::EmptyClassToggleCondition {
            class: t.class.clone(),
            span: t.span,
          });
      }
    }
  }

  fn check_empty_binding_expressions(
    &mut self,
    bindings: &[crate::ir::IrBinding],
  ) {
    for b in bindings {
      if b.expression.trim().is_empty() {
        self
          .warnings
          .push(ValidationWarning::EmptyBindingExpression {
            property: b.property.clone(),
            span: b.span,
          });
      }
    }
  }

  // ── Component prop checks ──────────────────────────────────────────

  /// Warn when a component prop has an empty value (e.g. `<Button label="">`).
  ///
  /// Boolean props (no `=` at all) are fine; this targets explicitly
  /// empty quoted values which are almost always a mistake.
  fn check_empty_component_props(&mut self, comp: &IrComponentRef) {
    for prop in &comp.props {
      // Skip the reserved `slot` attribute.
      if prop.name == "slot" {
        continue;
      }
      // An empty `value` vec means a boolean attribute — that's intentional.
      if prop.value.is_empty() {
        continue;
      }
      // Check whether all value segments are empty text.
      let all_empty = prop.value.iter().all(|v| match v {
        thebe_ast::TemplateNode::Text(t) => t.is_empty(),
        thebe_ast::TemplateNode::Expr { expr, .. } => expr.trim().is_empty(),
      });
      if all_empty {
        self
          .warnings
          .push(ValidationWarning::EmptyComponentProp {
            name: prop.name.clone(),
            component: comp.name.clone(),
            span: prop.span,
          });
      }
    }
  }

  /// Warn when `class:` or `style:` directives are used on a component.
  ///
  /// These element-level directives are silently ignored during codegen
  /// because components are rendered via their own `render()` function
  /// and control their own root elements.
  fn check_directives_on_component(&mut self, comp: &IrComponentRef) {
    for toggle in &comp.class_toggles {
      self
        .warnings
        .push(ValidationWarning::DirectiveOnComponent {
          directive: format!("class:{}", toggle.class),
          component: comp.name.clone(),
          span: toggle.span,
        });
    }
    for prop in &comp.style_props {
      self
        .warnings
        .push(ValidationWarning::DirectiveOnComponent {
          directive: format!("style:{}", prop.property),
          component: comp.name.clone(),
          span: prop.span,
        });
    }
  }

  // ── Slot attribute checks ──────────────────────────────────────────

  /// Validate that a `slot` attribute (if present) has a single, static,
  /// non-empty text value. Dynamic values, empty strings, whitespace-only
  /// values, and multi-segment values are flagged.
  fn check_slot_attribute(&mut self, tag: &str, attrs: &[IrAttribute]) {
    let Some(slot_attr) = attrs.iter().find(|a| a.name == "slot") else {
      return;
    };

    let reason = if slot_attr.value.is_empty() {
      // Boolean `slot` (no `=`) — not meaningful.
      Some("slot attribute requires a value (e.g. slot=\"header\")")
    } else if slot_attr.value.len() > 1 {
      // Mixed segments like slot="head{{x}}"
      Some("slot attribute must be a static string, not a dynamic expression")
    } else {
      match &slot_attr.value[0] {
        thebe_ast::TemplateNode::Text(t) if t.trim().is_empty() => {
          Some("slot attribute value must not be empty")
        }
        thebe_ast::TemplateNode::Expr { .. } => {
          Some("slot attribute must be a static string, not a dynamic expression")
        }
        thebe_ast::TemplateNode::Text(_) => None,
      }
    };

    if let Some(reason) = reason {
      self
        .warnings
        .push(ValidationWarning::InvalidSlotAttribute {
          tag: tag.to_string(),
          reason: reason.to_string(),
          span: slot_attr.span,
        });
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
