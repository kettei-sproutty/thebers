use thebe_ast::Span;
use thebe_ast::TemplateNode;

// ---------------------------------------------------------------------------
// Top-level compiled component
// ---------------------------------------------------------------------------

/// A compiled `.trs` component ready for downstream code generation.
///
/// This is the output of the lowering pass: the AST has been validated,
/// directives decomposed into typed IR nodes, and scoped styles assigned
/// a deterministic scope identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledComponent {
  /// The `<script setup>` block (always Rust), if present.
  pub setup: Option<SetupBlock>,
  /// The client-side `<script lang="...">` block, if present.
  pub client_script: Option<ClientScript>,
  /// Processed style blocks in source order.
  pub styles: Vec<ProcessedStyle>,
  /// Template IR nodes in source order (the HTML tree, lowered).
  pub template: Vec<IrNode>,
  /// Nodes from `<thebe:head>` blocks — rendered into `<head>` instead of `<body>`.
  pub head: Vec<IrNode>,
}

/// The `<script setup>` block content.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupBlock {
  /// Raw Rust source code.
  pub content: String,
  /// Byte-offset span of the entire block.
  pub span: Span,
}

/// A client-side `<script lang="...">` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientScript {
  /// Raw script content.
  pub content: String,
  /// The `lang` attribute value (e.g. `"js"`, `"ts"`).
  pub lang: String,
  /// Byte-offset span of the entire block.
  pub span: Span,
}

/// A processed `<style>` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedStyle {
  /// Raw CSS/preprocessor content.
  pub content: String,
  /// Whether this style block has the `scoped` attribute.
  pub scoped: bool,
  /// Deterministic hash identifier for scoped style isolation.
  /// `None` for non-scoped styles.
  pub scope_id: Option<String>,
  /// The `lang` attribute value (e.g. `"scss"`).
  pub lang: Option<String>,
  /// Byte-offset span of the entire block.
  pub span: Span,
}

// ---------------------------------------------------------------------------
// Template IR nodes
// ---------------------------------------------------------------------------

/// A node in the lowered template tree.
///
/// Mirrors [`thebe_ast::HtmlNode`] but with directives decomposed into
/// typed, validated fields (events, bindings, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum IrNode {
  /// An HTML element with typed directives.
  Element(IrElement),
  /// Raw text between tags.
  Text(String),
  /// An interpolated expression `{{ expr }}`.
  Expr(IrExpr),
  /// A conditional block `{#if}` / `{:else if}` / `{:else}` / `{/if}`.
  If(IrIf),
  /// An iteration block `{#each iterable as binding}` / `{/each}`.
  Each(IrEach),
  /// A component reference (tag starting with uppercase).
  Component(IrComponentRef),
  /// A `<slot>` element for composition.
  Slot(IrSlot),
  /// A raw HTML injection `{@html expr}` — emitted without escaping.
  RawHtml(IrExpr),
  /// A local constant binding `{@const name = expr}` — emits a `let` statement.
  Const(IrConst),
  /// A debug tag `{@debug expr}` — emits `eprintln!` at runtime.
  Debug(IrExpr),
}

/// A lowered HTML element with directives split into typed fields.
#[derive(Debug, Clone, PartialEq)]
pub struct IrElement {
  /// The tag name, e.g. `"div"`, `"button"`.
  pub tag: String,
  /// Static and dynamic attributes.
  pub attributes: Vec<IrAttribute>,
  /// Event handlers (`on:click`, `on:submit`, …).
  pub events: Vec<IrEventHandler>,
  /// Two-way bindings (`bind:value`, …).
  pub bindings: Vec<IrBinding>,
  /// Conditional CSS classes (`class:active`, …).
  pub class_toggles: Vec<IrClassToggle>,
  /// Inline style properties (`style:color`, …).
  pub style_props: Vec<IrStyleProp>,
  /// Action hooks (`use:tooltip`, …).
  pub actions: Vec<IrAction>,
  /// Child IR nodes.
  pub children: Vec<IrNode>,
  /// Whether this element is self-closing.
  pub self_closing: bool,
  /// Byte-offset span of the entire element.
  pub span: Span,
}

/// A lowered component reference.
///
/// Structurally similar to [`IrElement`] but distinguished so that
/// downstream passes can resolve component definitions and validate props.
#[derive(Debug, Clone, PartialEq)]
pub struct IrComponentRef {
  /// The component name (uppercase tag), e.g. `"Button"`, `"Modal"`.
  pub name: String,
  /// Props passed as attributes.
  pub props: Vec<IrAttribute>,
  /// Event handlers on the component.
  pub events: Vec<IrEventHandler>,
  /// Two-way bindings on the component.
  pub bindings: Vec<IrBinding>,
  /// Conditional CSS class toggles specified on this component tag.
  ///
  /// These are retained for validation purposes — `class:` directives
  /// on a component have no effect (the component controls its own root
  /// elements) and a warning is emitted.
  pub class_toggles: Vec<IrClassToggle>,
  /// Inline style properties specified on this component tag.
  ///
  /// Retained for validation — `style:` directives on a component have
  /// no effect and a warning is emitted.
  pub style_props: Vec<IrStyleProp>,
  /// Action hooks on the component.
  pub actions: Vec<IrAction>,
  /// Default slot children.
  pub children: Vec<IrNode>,
  /// Whether the component tag is self-closing.
  pub self_closing: bool,
  /// Byte-offset span.
  pub span: Span,
}

/// An attribute on an element or component.
#[derive(Debug, Clone, PartialEq)]
pub struct IrAttribute {
  /// Attribute name.
  pub name: String,
  /// Parsed value segments (static text and `{{ expr }}` expressions).
  pub value: Vec<TemplateNode>,
  /// Byte-offset span of the `name="value"` pair.
  pub span: Span,
}

/// An interpolated expression in template text.
#[derive(Debug, Clone, PartialEq)]
pub struct IrExpr {
  /// The trimmed expression text.
  pub expr: String,
  /// Byte-offset span of the `{{ ... }}`.
  pub span: Span,
}

/// A local constant binding from `{@const name = expr}`.
#[derive(Debug, Clone, PartialEq)]
pub struct IrConst {
  /// The variable name.
  pub name: String,
  /// The initializer expression.
  pub expr: String,
  /// Byte-offset span.
  pub span: Span,
}

/// An event handler decomposed from an `on:event` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct IrEventHandler {
  /// The DOM event name (e.g. `"click"`, `"submit"`).
  pub event: String,
  /// The handler expression.
  pub handler: String,
  /// Typed, validated modifiers.
  pub modifiers: Vec<EventModifier>,
  /// Byte-offset span of the directive.
  pub span: Span,
}

/// A validated event modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventModifier {
  /// `event.preventDefault()`
  PreventDefault,
  /// `event.stopPropagation()`
  StopPropagation,
  /// Fire the handler at most once.
  Once,
  /// Use capture phase instead of bubble.
  Capture,
  /// Only fire when `event.target === element`.
  Self_,
  /// Only fire for `event.isTrusted`.
  Trusted,
  /// Mark the listener as passive.
  Passive,
  /// Explicitly mark the listener as non-passive.
  NonPassive,
}

/// A two-way binding from a `bind:prop` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct IrBinding {
  /// The property being bound (e.g. `"value"`).
  pub property: String,
  /// The expression to bind to.
  pub expression: String,
  /// Byte-offset span of the directive.
  pub span: Span,
}

/// A conditional CSS class from a `class:name` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct IrClassToggle {
  /// The CSS class name (e.g. `"active"`).
  pub class: String,
  /// The condition expression.
  pub condition: String,
  /// Byte-offset span of the directive.
  pub span: Span,
}

/// An inline style property from a `style:prop` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct IrStyleProp {
  /// The CSS property name (e.g. `"color"`).
  pub property: String,
  /// The value expression.
  pub value: String,
  /// Byte-offset span of the directive.
  pub span: Span,
}

/// An action hook from a `use:action` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct IrAction {
  /// The action name (e.g. `"tooltip"`).
  pub name: String,
  /// The argument expression (may be empty).
  pub argument: String,
  /// Byte-offset span of the directive.
  pub span: Span,
}

/// A lowered conditional block.
#[derive(Debug, Clone, PartialEq)]
pub struct IrIf {
  /// Branches in order: `{#if}`, `{:else if}…`, optional `{:else}`.
  pub branches: Vec<IrIfBranch>,
  /// Byte-offset span of the entire block.
  pub span: Span,
}

/// A single branch in an `{#if}` chain.
#[derive(Debug, Clone, PartialEq)]
pub struct IrIfBranch {
  /// The condition expression (`None` for `{:else}`).
  pub condition: Option<String>,
  /// Child IR nodes.
  pub children: Vec<IrNode>,
  /// Byte-offset span of this branch's body.
  pub span: Span,
}

/// A lowered iteration block.
#[derive(Debug, Clone, PartialEq)]
pub struct IrEach {
  /// The iterable expression.
  pub iterable: String,
  /// The loop variable binding.
  pub binding: String,
  /// Optional index variable.
  pub index: Option<String>,
  /// Child IR nodes inside the loop body.
  pub children: Vec<IrNode>,
  /// Byte-offset span of the entire block.
  pub span: Span,
}

/// A lowered `<slot>` element.
#[derive(Debug, Clone, PartialEq)]
pub struct IrSlot {
  /// The slot name (`None` for the default slot).
  pub name: Option<String>,
  /// Fallback content rendered when the slot is not filled.
  pub fallback: Vec<IrNode>,
  /// Byte-offset span.
  pub span: Span,
}
