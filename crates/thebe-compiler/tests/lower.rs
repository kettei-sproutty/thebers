use thebe_compiler::CompileError;
use thebe_compiler::CompiledComponent;
use thebe_compiler::EventModifier;
use thebe_compiler::IrNode;

/// Helper: parse + lower in one step.
fn compile(source: &str) -> Result<CompiledComponent, CompileError> {
  let ast = thebe_ast::parse(source)?;
  thebe_compiler::lower(source, &ast)
}

// ── Setup / client script lowering ──────────────────────────────────────

#[test]
fn lower_script_setup() {
  let source = "<script setup>let x = 1;</script>";
  let comp = compile(source).unwrap();
  let setup = comp.setup.unwrap();
  assert_eq!(setup.content, "let x = 1;");
}

#[test]
fn lower_client_script() {
  let source = r#"<script lang="js">var y = 2;</script>"#;
  let comp = compile(source).unwrap();
  let cs = comp.client_script.unwrap();
  assert_eq!(cs.content, "var y = 2;");
  assert_eq!(cs.lang, "js");
}

#[test]
fn no_scripts_yields_none() {
  let source = "<div>hello</div>";
  let comp = compile(source).unwrap();
  assert!(comp.setup.is_none());
  assert!(comp.client_script.is_none());
}

// ── Style lowering ──────────────────────────────────────────────────────

#[test]
fn lower_unscoped_style() {
  let source = "<style>.a { color: red; }</style>";
  let comp = compile(source).unwrap();
  assert_eq!(comp.styles.len(), 1);
  assert!(!comp.styles[0].scoped);
  assert!(comp.styles[0].scope_id.is_none());
}

#[test]
fn lower_scoped_style_has_scope_id() {
  let source = "<style scoped>.a { color: red; }</style>";
  let comp = compile(source).unwrap();
  let style = &comp.styles[0];
  assert!(style.scoped);
  assert!(style.scope_id.is_some());
  let id = style.scope_id.as_ref().unwrap();
  assert!(id.starts_with("s-"), "scope_id should start with 's-': {id}");
}

#[test]
fn scoped_style_id_is_deterministic() {
  let source = "<style scoped>.a { color: red; }</style>";
  let c1 = compile(source).unwrap();
  let c2 = compile(source).unwrap();
  assert_eq!(c1.styles[0].scope_id, c2.styles[0].scope_id);
}

#[test]
fn style_lang_preserved() {
  let source = r#"<style lang="scss">.a { color: red; }</style>"#;
  let comp = compile(source).unwrap();
  assert_eq!(comp.styles[0].lang.as_deref(), Some("scss"));
}

// ── Template element lowering ───────────────────────────────────────────

#[test]
fn lower_simple_element() {
  let source = "<div>hello</div>";
  let comp = compile(source).unwrap();
  assert_eq!(comp.template.len(), 1);
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected IrNode::Element")
  };
  assert_eq!(el.tag, "div");
  assert_eq!(el.children.len(), 1);
  assert!(matches!(&el.children[0], IrNode::Text(t) if t == "hello"));
}

#[test]
fn lower_self_closing_element() {
  let source = "<br />";
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert!(el.self_closing);
  assert!(el.children.is_empty());
}

#[test]
fn lower_element_with_attributes() {
  let source = r#"<a href="/" class="link">go</a>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.attributes.len(), 2);
  assert_eq!(el.attributes[0].name, "href");
  assert_eq!(el.attributes[1].name, "class");
}

// ── Event handler lowering ──────────────────────────────────────────────

#[test]
fn lower_on_directive_to_event_handler() {
  let source = r#"<button on:click="handle">go</button>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.events.len(), 1);
  assert_eq!(el.events[0].event, "click");
  assert_eq!(el.events[0].handler, "handle");
  assert!(el.events[0].modifiers.is_empty());
}

#[test]
fn lower_event_modifiers() {
  let source = r#"<form on:submit|preventDefault="save">ok</form>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.events[0].modifiers, vec![EventModifier::PreventDefault]);
}

#[test]
fn lower_multiple_event_modifiers() {
  let source =
    r#"<button on:click|preventDefault|stopPropagation="h">x</button>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(
    el.events[0].modifiers,
    vec![EventModifier::PreventDefault, EventModifier::StopPropagation],
  );
}

#[test]
fn unknown_event_modifier_is_error() {
  let source = r#"<button on:click|yolo="h">x</button>"#;
  let err = compile(source).unwrap_err();
  assert!(
    matches!(err, CompileError::UnknownEventModifier { modifier, .. } if modifier == "yolo"),
  );
}

// ── Bind / class / style / use directive lowering ───────────────────────

#[test]
fn lower_bind_directive() {
  let source = r#"<input bind:value="name" />"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.bindings.len(), 1);
  assert_eq!(el.bindings[0].property, "value");
  assert_eq!(el.bindings[0].expression, "name");
}

#[test]
fn lower_class_directive() {
  let source = r#"<div class:active="is_active">x</div>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.class_toggles.len(), 1);
  assert_eq!(el.class_toggles[0].class, "active");
  assert_eq!(el.class_toggles[0].condition, "is_active");
}

#[test]
fn lower_style_directive() {
  let source = r#"<div style:color="red">x</div>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.style_props.len(), 1);
  assert_eq!(el.style_props[0].property, "color");
  assert_eq!(el.style_props[0].value, "red");
}

#[test]
fn lower_use_directive() {
  let source = r#"<div use:tooltip="msg">x</div>"#;
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.actions.len(), 1);
  assert_eq!(el.actions[0].name, "tooltip");
  assert_eq!(el.actions[0].argument, "msg");
}

// ── Expression lowering ─────────────────────────────────────────────────

#[test]
fn lower_interpolation() {
  let source = "<p>{{ name }}</p>";
  let comp = compile(source).unwrap();
  let IrNode::Element(el) = &comp.template[0] else {
    panic!("expected Element")
  };
  let IrNode::Expr(expr) = &el.children[0] else {
    panic!("expected Expr")
  };
  assert_eq!(expr.expr, "name");
}

// ── Control-flow lowering ───────────────────────────────────────────────

#[test]
fn lower_if_block() {
  let source = "{#if show}<p>hi</p>{/if}";
  let comp = compile(source).unwrap();
  let IrNode::If(ir_if) = &comp.template[0] else {
    panic!("expected If")
  };
  assert_eq!(ir_if.branches.len(), 1);
  assert_eq!(ir_if.branches[0].condition.as_deref(), Some("show"));
}

#[test]
fn lower_if_else_block() {
  let source = "{#if show}<p>a</p>{:else}<p>b</p>{/if}";
  let comp = compile(source).unwrap();
  let IrNode::If(ir_if) = &comp.template[0] else {
    panic!("expected If")
  };
  assert_eq!(ir_if.branches.len(), 2);
  assert!(ir_if.branches[1].condition.is_none());
}

#[test]
fn lower_each_block() {
  let source = "{#each items as item, i}<p>{{ item }}</p>{/each}";
  let comp = compile(source).unwrap();
  let IrNode::Each(each) = &comp.template[0] else {
    panic!("expected Each")
  };
  assert_eq!(each.iterable, "items");
  assert_eq!(each.binding, "item");
  assert_eq!(each.index.as_deref(), Some("i"));
}

// ── Component lowering ──────────────────────────────────────────────────

#[test]
fn lower_component() {
  let source = r#"<Button on:click="go">ok</Button>"#;
  let comp = compile(source).unwrap();
  let IrNode::Component(c) = &comp.template[0] else {
    panic!("expected Component")
  };
  assert_eq!(c.name, "Button");
  assert_eq!(c.events.len(), 1);
  assert_eq!(c.events[0].event, "click");
}

#[test]
fn lower_component_with_props() {
  let source = r#"<Modal title="hello" />"#;
  let comp = compile(source).unwrap();
  let IrNode::Component(c) = &comp.template[0] else {
    panic!("expected Component")
  };
  assert_eq!(c.name, "Modal");
  assert_eq!(c.props.len(), 1);
  assert_eq!(c.props[0].name, "title");
  assert!(c.self_closing);
}

// ── Slot lowering ───────────────────────────────────────────────────────

#[test]
fn lower_default_slot() {
  let source = "<slot />";
  let comp = compile(source).unwrap();
  let IrNode::Slot(s) = &comp.template[0] else {
    panic!("expected Slot")
  };
  assert!(s.name.is_none());
  assert!(s.fallback.is_empty());
}

#[test]
fn lower_named_slot_with_fallback() {
  let source = r#"<slot name="header"><p>default</p></slot>"#;
  let comp = compile(source).unwrap();
  let IrNode::Slot(s) = &comp.template[0] else {
    panic!("expected Slot")
  };
  assert_eq!(s.name.as_deref(), Some("header"));
  assert_eq!(s.fallback.len(), 1);
}

// ── Full component lowering ─────────────────────────────────────────────

#[test]
fn lower_full_component() {
  let source = r#"<script setup>let count = 0;</script>
<script lang="js">var x = 1;</script>
<style scoped>.active { color: red; }</style>
<div>
  <h1>{{ title }}</h1>
  <Button on:click|preventDefault="increment">Count: {{ count }}</Button>
</div>"#;

  let comp = compile(source).unwrap();
  assert!(comp.setup.is_some());
  assert!(comp.client_script.is_some());
  assert_eq!(comp.styles.len(), 1);
  assert!(comp.styles[0].scope_id.is_some());
  assert!(!comp.template.is_empty());

  // Template should have the <div> element.
  let IrNode::Element(div) = &comp.template[0] else {
    panic!("expected Element")
  };
  assert_eq!(div.tag, "div");
}

// ── Diagnostics ─────────────────────────────────────────────────────────

#[test]
fn diagnostic_for_unknown_modifier() {
  let source = r#"<button on:click|banana="h">x</button>"#;
  let err = compile(source).unwrap_err();
  let output = thebe_compiler::diagnostics::error_to_string(&err, source, None);
  assert!(output.contains("unknown event modifier"));
  assert!(output.contains("banana"));
}

#[test]
fn diagnostic_for_parse_error() {
  let source = "<div><div>";
  let err = compile(source).unwrap_err();
  let output = thebe_compiler::diagnostics::error_to_string(&err, source, None);
  assert!(output.contains("parse error") || output.contains("malformed"));
}
