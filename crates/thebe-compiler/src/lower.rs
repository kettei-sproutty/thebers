use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use thebe_ast::DirectiveKind;
use thebe_ast::Element;
use thebe_ast::HtmlNode;
use thebe_ast::Span;
use thebe_ast::ThebeAst;

use crate::error::CompileError;
use crate::ir::ClientScript;
use crate::ir::CompiledComponent;
use crate::ir::EventModifier;
use crate::ir::IrAction;
use crate::ir::IrAttribute;
use crate::ir::IrBinding;
use crate::ir::IrClassToggle;
use crate::ir::IrComponentRef;
use crate::ir::IrEach;
use crate::ir::IrElement;
use crate::ir::IrEventHandler;
use crate::ir::IrExpr;
use crate::ir::IrIf;
use crate::ir::IrIfBranch;
use crate::ir::IrNode;
use crate::ir::IrSlot;
use crate::ir::IrStyleProp;
use crate::ir::ProcessedStyle;
use crate::ir::SetupBlock;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lower a parsed `.trs` AST and its raw source into a [`CompiledComponent`].
///
/// The `source` string must be the same input that was passed to
/// [`thebe_ast::parse`] to produce `ast`. It is needed to extract
/// template regions and parse them into the full HTML tree.
///
/// # Errors
///
/// Returns [`CompileError`] on invalid event modifiers or if template
/// HTML parsing fails.
pub fn lower(source: &str, ast: &ThebeAst) -> Result<CompiledComponent, CompileError> {
  let setup = ast.script_setup.as_ref().map(lower_setup);
  let client_script = ast.script.as_ref().map(lower_client_script);
  let styles = ast.styles.iter().map(lower_style).collect();
  let template = lower_template(source, ast)?;

  Ok(CompiledComponent {
    setup,
    client_script,
    styles,
    template,
  })
}

// ---------------------------------------------------------------------------
// Script / style lowering
// ---------------------------------------------------------------------------

fn lower_setup(script: &thebe_ast::ScriptBlock) -> SetupBlock {
  SetupBlock {
    content: script.content.clone(),
    span: script.span,
  }
}

fn lower_client_script(script: &thebe_ast::ScriptBlock) -> ClientScript {
  ClientScript {
    content: script.content.clone(),
    lang: script.lang.clone().unwrap_or_default(),
    span: script.span,
  }
}

fn lower_style(style: &thebe_ast::StyleBlock) -> ProcessedStyle {
  let scope_id = if style.scoped {
    Some(generate_scope_id(&style.content))
  } else {
    None
  };

  ProcessedStyle {
    content: style.content.clone(),
    scoped: style.scoped,
    scope_id,
    lang: style.lang.clone(),
    span: style.span,
  }
}

/// Generate a deterministic scope identifier from style content.
fn generate_scope_id(content: &str) -> String {
  let mut hasher = DefaultHasher::new();
  content.hash(&mut hasher);
  let hash = hasher.finish();
  // 8 hex chars is plenty for scoped style isolation.
  format!("s-{hash:08x}", hash = hash & 0xFFFF_FFFF)
}

// ---------------------------------------------------------------------------
// Template lowering
// ---------------------------------------------------------------------------

/// Extract template regions (gaps between script/style blocks) from the
/// raw source, parse each into an HTML tree, and lower to IR nodes.
fn lower_template(source: &str, ast: &ThebeAst) -> Result<Vec<IrNode>, CompileError> {
  let regions = extract_template_regions(source, ast);
  let mut nodes = Vec::new();

  for (offset, text) in regions {
    let trimmed = text.trim();
    if trimmed.is_empty() {
      continue;
    }
    let trim_offset = offset + (text.len() - text.trim_start().len());
    let html_nodes = thebe_ast::parse_html(trimmed, trim_offset)?;
    for html_node in html_nodes {
      nodes.push(lower_node(&html_node)?);
    }
  }

  Ok(nodes)
}

/// Find the template-text gaps between script/style block spans.
fn extract_template_regions<'a>(source: &'a str, ast: &ThebeAst) -> Vec<(usize, &'a str)> {
  let mut block_spans: Vec<Span> = Vec::new();

  if let Some(s) = &ast.script_setup {
    block_spans.push(s.span);
  }
  if let Some(s) = &ast.script {
    block_spans.push(s.span);
  }
  for s in &ast.styles {
    block_spans.push(s.span);
  }

  block_spans.sort_by_key(|s| s.start);

  let mut regions = Vec::new();
  let mut pos = 0;

  for span in &block_spans {
    if span.start > pos {
      regions.push((pos, &source[pos..span.start]));
    }
    pos = span.end;
  }

  if pos < source.len() {
    regions.push((pos, &source[pos..]));
  }

  regions
}

// ---------------------------------------------------------------------------
// Node lowering
// ---------------------------------------------------------------------------

fn lower_node(node: &HtmlNode) -> Result<IrNode, CompileError> {
  match node {
    HtmlNode::Element(el) => Ok(IrNode::Element(lower_element(el)?)),
    HtmlNode::Text(t) => Ok(IrNode::Text(t.clone())),
    HtmlNode::Expr { expr, span } => Ok(IrNode::Expr(IrExpr {
      expr: expr.clone(),
      span: *span,
    })),
    HtmlNode::If { branches, span } => Ok(IrNode::If(lower_if(branches, *span)?)),
    HtmlNode::Each {
      iterable,
      binding,
      index,
      children,
      span,
    } => Ok(IrNode::Each(lower_each(
      iterable, binding, index.as_ref(), children, *span,
    )?)),
    HtmlNode::Component(el) => Ok(IrNode::Component(lower_component(el)?)),
    HtmlNode::Slot {
      name,
      children,
      span,
    } => Ok(IrNode::Slot(lower_slot(name.as_ref(), children, *span)?)),
  }
}

fn lower_element(el: &Element) -> Result<IrElement, CompileError> {
  let (events, bindings, class_toggles, style_props, actions) = decompose_directives(el)?;

  let attributes = el.attributes.iter().map(lower_attribute).collect();
  let children = el
    .children
    .iter()
    .map(lower_node)
    .collect::<Result<Vec<_>, _>>()?;

  Ok(IrElement {
    tag: el.tag.clone(),
    attributes,
    events,
    bindings,
    class_toggles,
    style_props,
    actions,
    children,
    self_closing: el.self_closing,
    span: el.span,
  })
}

fn lower_component(el: &Element) -> Result<IrComponentRef, CompileError> {
  let (events, bindings, _class_toggles, _style_props, actions) = decompose_directives(el)?;

  let props = el.attributes.iter().map(lower_attribute).collect();
  let children = el
    .children
    .iter()
    .map(lower_node)
    .collect::<Result<Vec<_>, _>>()?;

  Ok(IrComponentRef {
    name: el.tag.clone(),
    props,
    events,
    bindings,
    actions,
    children,
    self_closing: el.self_closing,
    span: el.span,
  })
}

fn lower_attribute(attr: &thebe_ast::Attribute) -> IrAttribute {
  IrAttribute {
    name: attr.name.clone(),
    value: attr.value.clone(),
    span: attr.span,
  }
}

fn lower_if(
  branches: &[thebe_ast::IfBranch],
  span: Span,
) -> Result<IrIf, CompileError> {
  let ir_branches = branches
    .iter()
    .map(|b| {
      let children = b
        .children
        .iter()
        .map(lower_node)
        .collect::<Result<Vec<_>, _>>()?;
      Ok(IrIfBranch {
        condition: b.condition.clone(),
        children,
        span: b.span,
      })
    })
    .collect::<Result<Vec<_>, CompileError>>()?;

  Ok(IrIf {
    branches: ir_branches,
    span,
  })
}

fn lower_each(
  iterable: &str,
  binding: &str,
  index: Option<&String>,
  children: &[HtmlNode],
  span: Span,
) -> Result<IrEach, CompileError> {
  let ir_children = children
    .iter()
    .map(lower_node)
    .collect::<Result<Vec<_>, _>>()?;

  Ok(IrEach {
    iterable: iterable.to_string(),
    binding: binding.to_string(),
    index: index.cloned(),
    children: ir_children,
    span,
  })
}

fn lower_slot(
  name: Option<&String>,
  children: &[HtmlNode],
  span: Span,
) -> Result<IrSlot, CompileError> {
  let fallback = children
    .iter()
    .map(lower_node)
    .collect::<Result<Vec<_>, _>>()?;

  Ok(IrSlot {
    name: name.cloned(),
    fallback,
    span,
  })
}

// ---------------------------------------------------------------------------
// Directive decomposition
// ---------------------------------------------------------------------------

type DecomposedDirectives = (
  Vec<IrEventHandler>,
  Vec<IrBinding>,
  Vec<IrClassToggle>,
  Vec<IrStyleProp>,
  Vec<IrAction>,
);

/// Split an element's generic directives into typed IR fields.
fn decompose_directives(el: &Element) -> Result<DecomposedDirectives, CompileError> {
  let mut events = Vec::new();
  let mut bindings = Vec::new();
  let mut class_toggles = Vec::new();
  let mut style_props = Vec::new();
  let mut actions = Vec::new();

  for dir in &el.directives {
    match dir.kind {
      DirectiveKind::On => {
        let modifiers = dir
          .modifiers
          .iter()
          .map(|m| parse_event_modifier(m, dir.span))
          .collect::<Result<Vec<_>, _>>()?;

        events.push(IrEventHandler {
          event: dir.name.clone(),
          handler: dir.value.clone(),
          modifiers,
          span: dir.span,
        });
      }
      DirectiveKind::Bind => {
        bindings.push(IrBinding {
          property: dir.name.clone(),
          expression: dir.value.clone(),
          span: dir.span,
        });
      }
      DirectiveKind::Class => {
        class_toggles.push(IrClassToggle {
          class: dir.name.clone(),
          condition: dir.value.clone(),
          span: dir.span,
        });
      }
      DirectiveKind::Style => {
        style_props.push(IrStyleProp {
          property: dir.name.clone(),
          value: dir.value.clone(),
          span: dir.span,
        });
      }
      DirectiveKind::Use => {
        actions.push(IrAction {
          name: dir.name.clone(),
          argument: dir.value.clone(),
          span: dir.span,
        });
      }
    }
  }

  Ok((events, bindings, class_toggles, style_props, actions))
}

/// Parse a string event modifier into its typed representation.
fn parse_event_modifier(modifier: &str, span: Span) -> Result<EventModifier, CompileError> {
  match modifier {
    "preventDefault" => Ok(EventModifier::PreventDefault),
    "stopPropagation" => Ok(EventModifier::StopPropagation),
    "once" => Ok(EventModifier::Once),
    "capture" => Ok(EventModifier::Capture),
    "self" => Ok(EventModifier::Self_),
    "trusted" => Ok(EventModifier::Trusted),
    "passive" => Ok(EventModifier::Passive),
    "nonpassive" => Ok(EventModifier::NonPassive),
    _ => Err(CompileError::UnknownEventModifier {
      modifier: modifier.to_string(),
      span,
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_template_between_blocks() {
    let source = "<script setup>let x = 1;</script>\n<div>hello</div>\n<style>.a{}</style>";
    let ast = thebe_ast::parse(source).unwrap();
    let regions = extract_template_regions(source, &ast);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].1.trim(), "<div>hello</div>");
  }

  #[test]
  fn extract_template_no_blocks() {
    let source = "<div>only template</div>";
    let ast = thebe_ast::parse(source).unwrap();
    let regions = extract_template_regions(source, &ast);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].1, source);
  }

  #[test]
  fn scope_id_deterministic() {
    let id1 = generate_scope_id(".a { color: red; }");
    let id2 = generate_scope_id(".a { color: red; }");
    assert_eq!(id1, id2);
    assert!(id1.starts_with("s-"));
  }

  #[test]
  fn scope_id_differs_for_different_content() {
    let id1 = generate_scope_id(".a { color: red; }");
    let id2 = generate_scope_id(".b { color: blue; }");
    assert_ne!(id1, id2);
  }

  #[test]
  fn parse_known_modifier() {
    let span = Span::new(0, 10);
    assert_eq!(
      parse_event_modifier("preventDefault", span).unwrap(),
      EventModifier::PreventDefault,
    );
    assert_eq!(
      parse_event_modifier("stopPropagation", span).unwrap(),
      EventModifier::StopPropagation,
    );
    assert_eq!(
      parse_event_modifier("once", span).unwrap(),
      EventModifier::Once,
    );
  }

  #[test]
  fn parse_unknown_modifier_errors() {
    let span = Span::new(0, 10);
    let err = parse_event_modifier("yolo", span).unwrap_err();
    assert!(matches!(err, CompileError::UnknownEventModifier { .. }));
  }
}
