/// A byte-offset span within the source input.
///
/// Both `start` and `end` are byte offsets (0-based). `end` is exclusive,
/// so the spanned text is `&input[span.start..span.end]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
  /// Start byte offset (inclusive).
  pub start: usize,
  /// End byte offset (exclusive).
  pub end: usize,
}

impl Span {
  /// Create a new span.
  #[must_use]
  pub fn new(start: usize, end: usize) -> Self {
    Self { start, end }
  }

  /// Compute the 1-based line and column of `self.start` within `source`.
  #[must_use]
  pub fn line_col(&self, source: &str) -> (usize, usize) {
    let before = &source[..self.start];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.len() - before.rfind('\n').map_or(0, |p| p + 1) + 1;
    (line, col)
  }
}

/// A script block (`<script>` or `<script setup>`), with optional `lang` attribute.
///
/// Note: `<script setup>` is always Rust. The `lang` attribute is only
/// valid on `<script>` blocks; using it on `<script setup>` is a parse error.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptBlock {
  /// Raw content of the script block.
  pub content: String,
  /// The `lang` attribute value, e.g. `"ts"` for `<script lang="ts">`.
  /// Always `None` for `<script setup>` blocks.
  pub lang: Option<String>,
  /// Byte-offset span of the entire block (from `<` of opening tag to `>` of closing tag).
  pub span: Span,
}

/// A style block (`<style>` or `<style scoped>`), with optional `lang` attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleBlock {
  /// Raw content of the style block.
  pub content: String,
  /// Whether this style block has the `scoped` attribute.
  pub scoped: bool,
  /// The `lang` attribute value, e.g. `"scss"` for `<style lang="scss">`.
  pub lang: Option<String>,
  /// Byte-offset span of the entire block (from `<` of opening tag to `>` of closing tag).
  pub span: Span,
}

/// A single node within a template fragment.
///
/// Template content is split into static text segments and interpolated
/// expressions delimited by `{{ ... }}`.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateNode {
  /// Raw text/HTML emitted as-is.
  Text(String),
  /// An expression inside `{{ expr }}`. Leading/trailing whitespace
  /// within the braces is trimmed.
  Expr {
    /// The trimmed expression text.
    expr: String,
    /// Byte-offset span of the `{{ ... }}` in the original fragment.
    span: Span,
  },
}

/// A parsed template fragment — the result of splitting raw template
/// text on `{{ ... }}` interpolations.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateFragment {
  /// The sequence of text and expression nodes in source order.
  pub nodes: Vec<TemplateNode>,
}

// ---------------------------------------------------------------------------
// Template HTML tree types
// ---------------------------------------------------------------------------

/// A branch in an `{#if}` / `{:else if}` / `{:else}` conditional chain.
#[derive(Debug, Clone, PartialEq)]
pub struct IfBranch {
  /// The condition expression. `None` for the `{:else}` branch.
  pub condition: Option<String>,
  /// Child nodes inside this branch.
  pub children: Vec<HtmlNode>,
  /// Byte-offset span of this branch's body.
  pub span: Span,
}

/// A node in the parsed template HTML tree.
#[derive(Debug, Clone, PartialEq)]
pub enum HtmlNode {
  /// An HTML-like element with tag, attributes, directives, and children.
  Element(Element),
  /// Raw text (whitespace, punctuation, words between tags).
  Text(String),
  /// An interpolated expression `{{ expr }}`.
  Expr {
    /// The trimmed expression text.
    expr: String,
    /// Byte-offset span of the `{{ ... }}`.
    span: Span,
  },
  /// A conditional block: `{#if cond}...{:else if cond}...{:else}...{/if}`.
  If {
    /// Branches in order: the initial `{#if}`, any `{:else if}`, and optional `{:else}`.
    branches: Vec<IfBranch>,
    /// Byte-offset span of the entire block.
    span: Span,
  },
  /// An iteration block: `{#each iterable as binding}...{/each}`.
  Each {
    /// The iterable expression, e.g. `"items"`.
    iterable: String,
    /// The loop variable binding, e.g. `"item"`.
    binding: String,
    /// Optional index variable, e.g. `"i"` from `{#each items as item, i}`.
    index: Option<String>,
    /// Child nodes inside the loop body.
    children: Vec<HtmlNode>,
    /// Byte-offset span of the entire block.
    span: Span,
  },
  /// A component reference — an element whose tag starts with an uppercase letter.
  ///
  /// Structurally identical to [`Element`], but distinguished in the tree
  /// so downstream passes can resolve component definitions and props.
  Component(Element),
  /// A `<slot>` element for component composition.
  ///
  /// Default slots have `name: None`; named slots carry the `name` attribute
  /// value (e.g. `<slot name="header" />`). Children are fallback content
  /// rendered when no slot content is provided by the parent.
  Slot {
    /// The slot name (`None` for the default slot).
    name: Option<String>,
    /// Fallback content rendered when the slot is not filled.
    children: Vec<HtmlNode>,
    /// Byte-offset span of the entire `<slot>...</slot>` or `<slot />`.
    span: Span,
  },
  /// A raw HTML injection `{@html expr}`.
  ///
  /// The expression result is injected into the output **without**
  /// HTML-escaping, so the caller is responsible for sanitisation.
  RawHtml {
    /// The expression that produces the raw HTML string.
    expr: String,
    /// Byte-offset span of the entire `{@html ...}` block.
    span: Span,
  },
  /// A local constant binding `{@const name = expr}`.
  ///
  /// Introduces a `let` binding scoped to the current template block.
  Const {
    /// The variable name (e.g. `"total"`).
    name: String,
    /// The expression to bind (e.g. `"a + b"`).
    expr: String,
    /// Byte-offset span of the entire `{@const ...}` block.
    span: Span,
  },
  /// A debug tag `{@debug expr}` — logs the expression at runtime.
  ///
  /// In SSR mode this emits an `eprintln!` call. The expression is
  /// evaluated and printed but not rendered into the HTML output.
  Debug {
    /// The expression to log.
    expr: String,
    /// Byte-offset span of the entire `{@debug ...}` block.
    span: Span,
  },
  /// A `<thebe:head>` block for per-page `<title>` and meta tags.
  ///
  /// Children are rendered into the document `<head>` instead of the body.
  Head {
    /// Child nodes (`<title>`, `<meta>`, `<link>`, etc.).
    children: Vec<HtmlNode>,
    /// Byte-offset span of the entire `<thebe:head>...</thebe:head>`.
    span: Span,
  },
}

/// An HTML-like element in the template tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
  /// The tag name, e.g. `"div"`, `"button"`, `"vstack"`.
  pub tag: String,
  /// Static and dynamic attributes (`class="foo"`, `src="{{ url }}"`).
  pub attributes: Vec<Attribute>,
  /// Directives such as event bindings (`on:click="handler"`).
  pub directives: Vec<Directive>,
  /// Child nodes (nested elements, text, expressions).
  pub children: Vec<HtmlNode>,
  /// Whether this element is self-closing (`<img />`, `<br>`).
  pub self_closing: bool,
  /// Byte-offset span of the entire element (opening tag through closing tag).
  pub span: Span,
}

/// An attribute on an HTML element.
///
/// The value is parsed for `{{ }}` interpolations, so
/// `src="{{ url }}"` produces a value with one `TemplateNode::Expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
  /// Attribute name, e.g. `"class"`, `"src"`.
  pub name: String,
  /// The parsed value segments (static text and expressions).
  pub value: Vec<TemplateNode>,
  /// Byte-offset span of the entire `name="value"` pair.
  pub span: Span,
}

/// The kind of directive on an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveKind {
  /// An event binding, e.g. `on:click`.
  On,
  /// A two-way binding, e.g. `bind:value`.
  Bind,
  /// A conditional CSS class, e.g. `class:active`.
  Class,
  /// An inline style property, e.g. `style:color`.
  Style,
  /// An action / directive hook, e.g. `use:tooltip`.
  Use,
}

/// A directive on an HTML element (e.g. `on:click="handler"`).
#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
  /// The directive kind.
  pub kind: DirectiveKind,
  /// The directive argument (e.g. `"click"` in `on:click`).
  pub name: String,
  /// Event modifiers (e.g. `["preventDefault", "stopPropagation"]`
  /// from `on:click|preventDefault|stopPropagation`).
  /// Always empty for non-`On` directives.
  pub modifiers: Vec<String>,
  /// The directive value (e.g. `"increment"` in `on:click="increment"`).
  pub value: String,
  /// Byte-offset span of the entire directive.
  pub span: Span,
}

/// Represents the parsed AST of a `.trs` (Thebers) file.
///
/// A `.trs` file can contain:
/// - At most one `<script setup>` block (build/compile/server-side logic).
/// - At most one `<script>` block (client-side reactivity).
/// - Multiple `<style>` blocks (global and/or scoped).
/// - Zero or more template fragments (everything outside the above blocks),
///   preserved in source order as separate strings.
///
/// Top-level `<script>` tags inside template content are not allowed.
#[derive(Debug, Clone, PartialEq)]
pub struct ThebeAst {
  /// The `<script setup>` block, if present.
  pub script_setup: Option<ScriptBlock>,
  /// The `<script>` block, if present.
  pub script: Option<ScriptBlock>,
  /// Style blocks in source order. Both `<style>` and `<style scoped>` are supported.
  pub styles: Vec<StyleBlock>,
  /// Template fragments in source order. Each fragment is parsed into
  /// a sequence of [`TemplateNode`]s (static text and `{{ expr }}` interpolations).
  /// Non-contiguous template content is preserved as separate entries.
  pub template: Vec<TemplateFragment>,
}

/// Errors that can occur when parsing a `.trs` file.
///
/// Variants that reference a specific location carry a [`Span`]
/// pointing to the problematic byte range in the source input.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
  /// The input is empty.
  #[error("input is empty")]
  EmptyInput,
  /// A `<script setup>` block appears more than once.
  /// The span points to the second occurrence.
  #[error("duplicate <script setup> block at byte {}", span.start)]
  DuplicateScriptSetup { span: Span },
  /// A `<script>` block appears more than once.
  /// The span points to the second occurrence.
  #[error("duplicate <script> block at byte {}", span.start)]
  DuplicateScript { span: Span },
  /// A `<script>` tag was found inside template content.
  /// The span points to the nested `<script>` tag.
  #[error("<script> tag found inside template content at byte {}", span.start)]
  NestedScript { span: Span },
  /// A `lang` attribute was used on `<script setup>`, which is always Rust.
  /// The span points to the `<script setup>` opening tag.
  #[error("<script setup> does not accept a lang attribute (always Rust) at byte {}", span.start)]
  InvalidSetupLang { span: Span },
  /// A `<script>` block is missing the required `lang` attribute.
  /// The span points to the `<script>` opening tag.
  #[error("<script> requires a lang attribute at byte {}", span.start)]
  MissingScriptLang { span: Span },
  /// An unclosed `{{` without a matching `}}`.
  /// The span points to the opening `{{`.
  #[error("unclosed interpolation \"{{{{\" at byte {}", span.start)]
  UnclosedInterpolation { span: Span },
  /// A tag is not properly closed or is malformed.
  /// The span points to the opening tag.
  #[error("malformed tag at byte {}: {detail}", span.start)]
  MalformedTag { detail: String, span: Span },
  /// An `{#if}` block is missing its closing `{/if}` tag.
  /// The span covers from the opening `{#if}` to the end of parsed content.
  #[error("unclosed {{#if}} block at byte {}", span.start)]
  UnclosedIfBlock { span: Span },
  /// An `{#each}` block is missing its closing `{/each}` tag.
  /// The span covers from the opening `{#each}` to the end of parsed content.
  #[error("unclosed {{#each}} block at byte {}", span.start)]
  UnclosedEachBlock { span: Span },
  /// The expression inside `{#each ...}` could not be parsed.
  /// Expected `{#each iterable as binding}` or `{#each iterable as binding, index}`.
  /// The span points to the `{#each ...}` tag.
  #[error("invalid {{#each}} expression at byte {}: {detail}", span.start)]
  InvalidEachExpression { detail: String, span: Span },
  /// An `{@html}` block is missing its closing `}`.
  /// The span points to the opening `{@html`.
  #[error("unclosed {{@html}} block at byte {}", span.start)]
  UnclosedRawHtml { span: Span },
  /// An `{@const}` block is missing its closing `}`.
  #[error("unclosed {{@const}} block at byte {}", span.start)]
  UnclosedConst { span: Span },
  /// An `{@const}` block has an invalid expression (missing `=`).
  #[error("invalid {{@const}} expression at byte {}: {detail}", span.start)]
  InvalidConstExpression { detail: String, span: Span },
  /// An `{@debug}` block is missing its closing `}`.
  #[error("unclosed {{@debug}} block at byte {}", span.start)]
  UnclosedDebug { span: Span },
  /// A `<slot>` element has an unsupported attribute or directive.
  /// Only the `name` attribute is allowed on `<slot>` elements.
  /// The span points to the offending attribute or directive.
  #[error("unsupported attribute on <slot> at byte {}: {detail}", span.start)]
  InvalidSlotAttribute { detail: String, span: Span },
  /// A `<thebe:head>` element appears nested inside another element.
  /// The span points to the nested `<thebe:head>` tag.
  #[error("<thebe:head> cannot be nested inside other elements at byte {}", span.start)]
  NestedHead { span: Span },
}

impl ParseError {
  /// Returns the span of the error, if available.
  #[must_use]
  pub fn span(&self) -> Option<Span> {
    match self {
      ParseError::EmptyInput => None,
      ParseError::DuplicateScriptSetup { span }
      | ParseError::DuplicateScript { span }
      | ParseError::NestedScript { span }
      | ParseError::InvalidSetupLang { span }
      | ParseError::MissingScriptLang { span }
      | ParseError::UnclosedInterpolation { span }
      | ParseError::MalformedTag { span, .. }
      | ParseError::UnclosedIfBlock { span }
      | ParseError::UnclosedEachBlock { span }
      | ParseError::UnclosedRawHtml { span }
      | ParseError::UnclosedConst { span }
      | ParseError::UnclosedDebug { span }
      | ParseError::InvalidConstExpression { span, .. }
      | ParseError::InvalidEachExpression { span, .. }
      | ParseError::InvalidSlotAttribute { span, .. }
      | ParseError::NestedHead { span } => Some(*span),
    }
  }
}

/// Check whether a character is whitespace for tag parsing purposes.
pub(crate) fn is_whitespace(c: char) -> bool {
  matches!(c, ' ' | '\n' | '\t' | '\r')
}
