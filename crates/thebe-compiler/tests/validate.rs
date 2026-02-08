use thebe_compiler::ValidationWarning;

/// Helper: parse → lower → validate.
fn warnings(source: &str) -> Vec<ValidationWarning> {
  let ast = thebe_ast::parse(source).unwrap();
  let ir = thebe_compiler::lower(source, &ast).unwrap();
  thebe_compiler::validate(&ir)
}

// ── No warnings on clean input ──────────────────────────────────────────

#[test]
fn clean_component_no_warnings() {
  let w = warnings(r#"<div class="a" on:click="go">{{ x }}</div>"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

// ── Duplicate attributes ────────────────────────────────────────────────

#[test]
fn duplicate_attribute_warns() {
  let w = warnings(r#"<div class="a" class="b">x</div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateAttribute { name, tag, .. } if name == "class" && tag == "div"));
}

#[test]
fn different_attributes_no_warning() {
  let w = warnings(r#"<div class="a" id="b">x</div>"#);
  assert!(w.is_empty());
}

// ── Duplicate event handlers ────────────────────────────────────────────

#[test]
fn duplicate_event_handler_warns() {
  let w = warnings(r#"<button on:click="a" on:click="b">x</button>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateEventHandler { event, .. } if event == "click"));
}

#[test]
fn different_events_no_warning() {
  let w = warnings(r#"<button on:click="a" on:submit="b">x</button>"#);
  assert!(w.is_empty());
}

// ── Duplicate bindings ──────────────────────────────────────────────────

#[test]
fn duplicate_binding_warns() {
  let w = warnings(r#"<input bind:value="a" bind:value="b" />"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateBinding { property, .. } if property == "value"));
}

// ── Duplicate class toggles ─────────────────────────────────────────────

#[test]
fn duplicate_class_toggle_warns() {
  let w = warnings(r#"<div class:active="a" class:active="b">x</div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateClassToggle { class, .. } if class == "active"));
}

// ── Duplicate style props ───────────────────────────────────────────────

#[test]
fn duplicate_style_prop_warns() {
  let w = warnings(r#"<div style:color="red" style:color="blue">x</div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateStyleProp { property, .. } if property == "color"));
}

// ── Conflicting modifiers ───────────────────────────────────────────────

#[test]
fn passive_and_prevent_default_warns() {
  let w = warnings(r#"<button on:click|passive|preventDefault="go">x</button>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::ConflictingPassivePreventDefault { event, .. } if event == "click"));
}

#[test]
fn passive_and_nonpassive_warns() {
  let w = warnings(r#"<button on:click|passive|nonpassive="go">x</button>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::ConflictingPassiveNonPassive { event, .. } if event == "click"));
}

#[test]
fn prevent_default_alone_is_fine() {
  let w = warnings(r#"<button on:click|preventDefault="go">x</button>"#);
  assert!(w.is_empty());
}

// ── Empty expressions ───────────────────────────────────────────────────

#[test]
fn empty_expression_warns() {
  let w = warnings("<p>{{  }}</p>");
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::EmptyExpression { .. }));
}

#[test]
fn non_empty_expression_no_warning() {
  let w = warnings("<p>{{ x }}</p>");
  assert!(w.is_empty());
}

// ── Empty event handlers ────────────────────────────────────────────────

#[test]
fn empty_event_handler_warns() {
  let w = warnings(r#"<button on:click="">x</button>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::EmptyEventHandler { event, .. } if event == "click"));
}

// ── Multiple default slots ──────────────────────────────────────────────

#[test]
fn multiple_default_slots_warns() {
  let w = warnings("<div><slot /><slot /></div>");
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::MultipleDefaultSlots { .. }));
}

#[test]
fn single_default_slot_no_warning() {
  let w = warnings("<div><slot /></div>");
  assert!(w.is_empty());
}

// ── Duplicate named slots ───────────────────────────────────────────────

#[test]
fn duplicate_named_slot_warns() {
  let w = warnings(r#"<div><slot name="header" /><slot name="header" /></div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateNamedSlot { name, .. } if name == "header"));
}

#[test]
fn different_named_slots_no_warning() {
  let w = warnings(r#"<div><slot name="header" /><slot name="footer" /></div>"#);
  assert!(w.is_empty());
}

// ── Nested validation ───────────────────────────────────────────────────

#[test]
fn validation_recurses_into_children() {
  // Duplicate attr is inside a nested div.
  let w = warnings(r#"<div><p class="a" class="b">x</p></div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateAttribute { tag, .. } if tag == "p"));
}

#[test]
fn validation_recurses_into_if_branches() {
  let w = warnings(r#"{#if show}<div class="a" class="b">x</div>{/if}"#);
  assert_eq!(w.len(), 1);
}

#[test]
fn validation_recurses_into_each_body() {
  let w = warnings(r#"{#each items as item}<div class="a" class="b">x</div>{/each}"#);
  assert_eq!(w.len(), 1);
}

#[test]
fn validation_checks_components() {
  let w = warnings(r#"<Button on:click="a" on:click="b" />"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::DuplicateEventHandler { .. }));
}

// ── Multiple warnings ───────────────────────────────────────────────────

#[test]
fn multiple_issues_produce_multiple_warnings() {
  let w = warnings(r#"<div class="a" class="b" on:click="x" on:click="y">{{  }}</div>"#);
  assert_eq!(w.len(), 3);
}

// ── Slot duplicates across tree ─────────────────────────────────────────

#[test]
fn default_slots_in_different_branches_warn() {
  // Both branches have a default slot — still a duplicate at component level.
  let w = warnings("{#if show}<slot />{:else}<slot />{/if}");
  assert_eq!(w.len(), 1);
  assert!(matches!(&w[0], ValidationWarning::MultipleDefaultSlots { .. }));
}

#[test]
fn named_slot_plus_default_slot_no_warning() {
  let w = warnings(r#"<div><slot /><slot name="footer" /></div>"#);
  assert!(w.is_empty());
}

// ── Empty directive value warnings ──────────────────────────────────────

#[test]
fn empty_style_prop_value_warns() {
  let w = warnings(r#"<div style:color="">text</div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(
    &w[0],
    ValidationWarning::EmptyStylePropValue { property, .. } if property == "color"
  ));
}

#[test]
fn empty_class_toggle_condition_warns() {
  let w = warnings(r#"<div class:active="">text</div>"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(
    &w[0],
    ValidationWarning::EmptyClassToggleCondition { class, .. } if class == "active"
  ));
}

#[test]
fn empty_binding_expression_warns() {
  let w = warnings(r#"<input bind:value="" />"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(
    &w[0],
    ValidationWarning::EmptyBindingExpression { property, .. } if property == "value"
  ));
}

#[test]
fn nonempty_style_prop_no_warning() {
  let w = warnings(r#"<div style:color="theme_color">text</div>"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

// ── Component prop validation ───────────────────────────────────────────

#[test]
fn empty_component_prop_warns() {
  let w = warnings(r#"<Button label="">x</Button>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::EmptyComponentProp { name, component, .. }
    if name == "label" && component == "Button"
  ));
}

#[test]
fn non_empty_component_prop_no_warning() {
  let w = warnings(r#"<Button label="Click me" />"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

#[test]
fn boolean_component_prop_no_warning() {
  let w = warnings(r#"<Button disabled />"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

#[test]
fn slot_attribute_on_component_child_not_warned() {
  // slot="name" is intentional, not an empty prop.
  let w = warnings(r#"<Card><div slot="header">title</div></Card>"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

#[test]
fn class_directive_on_component_warns() {
  let w = warnings(r#"<Button class:active="is_active" />"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(
    &w[0],
    ValidationWarning::DirectiveOnComponent { directive, component, .. }
    if directive == "class:active" && component == "Button"
  ));
}

#[test]
fn style_directive_on_component_warns() {
  let w = warnings(r#"<Card style:color="red" />"#);
  assert_eq!(w.len(), 1);
  assert!(matches!(
    &w[0],
    ValidationWarning::DirectiveOnComponent { directive, component, .. }
    if directive == "style:color" && component == "Card"
  ));
}

#[test]
fn event_on_component_no_warning() {
  // Events on components are valid for bubbling.
  let w = warnings(r#"<Button on:click="handleClick" />"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

#[test]
fn multiple_component_prop_issues() {
  let w = warnings(r#"<Widget label="" class:active="true" style:color="c" />"#);
  // 1 empty prop + 1 class directive + 1 style directive = 3 warnings
  assert_eq!(w.len(), 3, "expected 3 warnings, got: {w:?}");
}

// ── Invalid slot attributes ─────────────────────────────────────────────

#[test]
fn valid_static_slot_attribute_no_warning() {
  let w = warnings(r#"<Card><div slot="header">title</div></Card>"#);
  assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

#[test]
fn empty_slot_attribute_warns() {
  let w = warnings(r#"<Card><div slot="">content</div></Card>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::InvalidSlotAttribute { tag, .. }
    if tag == "div"
  ));
}

#[test]
fn dynamic_slot_attribute_warns() {
  let w = warnings(r#"<Card><div slot="{{ name }}">content</div></Card>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::InvalidSlotAttribute { tag, .. }
    if tag == "div"
  ));
}

#[test]
fn mixed_slot_attribute_warns() {
  let w = warnings(r#"<Card><div slot="head{{ x }}">content</div></Card>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::InvalidSlotAttribute { tag, .. }
    if tag == "div"
  ));
}

#[test]
fn boolean_slot_attribute_warns() {
  // `slot` with no value (boolean style) is invalid — must have a name.
  let w = warnings(r#"<Card><div slot>content</div></Card>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::InvalidSlotAttribute { tag, .. }
    if tag == "div"
  ));
}

#[test]
fn whitespace_only_slot_attribute_warns() {
  let w = warnings(r#"<Card><div slot="  ">content</div></Card>"#);
  assert_eq!(w.len(), 1, "expected 1 warning, got: {w:?}");
  assert!(matches!(
    &w[0],
    ValidationWarning::InvalidSlotAttribute { tag, .. }
    if tag == "div"
  ));
}
