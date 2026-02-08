use thebe_ast::Span;

/// A non-fatal diagnostic produced during semantic validation.
///
/// Warnings don't prevent code generation — they highlight likely
/// mistakes that may cause unexpected behaviour at runtime.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationWarning {
  /// The same attribute name appears more than once on an element.
  #[error("duplicate attribute '{name}' on <{tag}> at byte {}", dup_span.start)]
  DuplicateAttribute {
    /// The attribute name.
    name: String,
    /// The tag name of the element.
    tag: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// The same event name appears more than once on an element.
  #[error("duplicate event handler 'on:{event}' on <{tag}> at byte {}", dup_span.start)]
  DuplicateEventHandler {
    /// The event name (e.g. `"click"`).
    event: String,
    /// The tag name of the element.
    tag: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// The same binding property appears more than once on an element.
  #[error("duplicate binding 'bind:{property}' on <{tag}> at byte {}", dup_span.start)]
  DuplicateBinding {
    /// The property name.
    property: String,
    /// The tag name.
    tag: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// The same class toggle appears more than once on an element.
  #[error("duplicate class toggle 'class:{class}' on <{tag}> at byte {}", dup_span.start)]
  DuplicateClassToggle {
    /// The CSS class name.
    class: String,
    /// The tag name.
    tag: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// The same style property appears more than once on an element.
  #[error("duplicate style prop 'style:{property}' on <{tag}> at byte {}", dup_span.start)]
  DuplicateStyleProp {
    /// The CSS property name.
    property: String,
    /// The tag name.
    tag: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// Event modifiers `passive` and `preventDefault` conflict.
  #[error("conflicting modifiers: 'passive' and 'preventDefault' on 'on:{event}' at byte {}", span.start)]
  ConflictingPassivePreventDefault {
    /// The event name.
    event: String,
    /// Span of the directive.
    span: Span,
  },

  /// Event modifiers `passive` and `nonpassive` conflict.
  #[error("conflicting modifiers: 'passive' and 'nonpassive' on 'on:{event}' at byte {}", span.start)]
  ConflictingPassiveNonPassive {
    /// The event name.
    event: String,
    /// Span of the directive.
    span: Span,
  },

  /// An interpolation expression is empty (e.g. `{{ }}`).
  #[error("empty expression in interpolation at byte {}", span.start)]
  EmptyExpression {
    /// Span of the `{{ }}`.
    span: Span,
  },

  /// An event handler has an empty value (e.g. `on:click=""`).
  #[error("empty handler for 'on:{event}' at byte {}", span.start)]
  EmptyEventHandler {
    /// The event name.
    event: String,
    /// Span of the directive.
    span: Span,
  },

  /// A style prop has an empty value expression (e.g. `style:color=""`).
  #[error("empty value for 'style:{property}' at byte {}", span.start)]
  EmptyStylePropValue {
    /// The CSS property name.
    property: String,
    /// Span of the directive.
    span: Span,
  },

  /// A class toggle has an empty condition (e.g. `class:active=""`).
  #[error("empty condition for 'class:{class}' at byte {}", span.start)]
  EmptyClassToggleCondition {
    /// The CSS class name.
    class: String,
    /// Span of the directive.
    span: Span,
  },

  /// A binding has an empty expression (e.g. `bind:value=""`).
  #[error("empty expression for 'bind:{property}' at byte {}", span.start)]
  EmptyBindingExpression {
    /// The property name.
    property: String,
    /// Span of the directive.
    span: Span,
  },

  /// More than one unnamed `<slot />` (default slot) in a component.
  #[error("multiple default slots at byte {}", dup_span.start)]
  MultipleDefaultSlots {
    /// Span of the first default slot.
    first_span: Span,
    /// Span of the duplicate default slot.
    dup_span: Span,
  },

  /// Two or more `<slot name="X">` with the same name.
  #[error("duplicate named slot '{name}' at byte {}", dup_span.start)]
  DuplicateNamedSlot {
    /// The slot name.
    name: String,
    /// Span of the first occurrence.
    first_span: Span,
    /// Span of the duplicate occurrence.
    dup_span: Span,
  },

  /// A component prop has an empty value (e.g. `<Button label="">`).
  #[error("empty prop value for '{name}' on <{component}> at byte {}", span.start)]
  EmptyComponentProp {
    /// The prop name.
    name: String,
    /// The component name.
    component: String,
    /// Span of the attribute.
    span: Span,
  },

  /// A `class:toggle` or `style:prop` directive is used on a component,
  /// where it has no effect (components don't forward element directives).
  #[error("{directive} directive on component <{component}> has no effect at byte {}", span.start)]
  DirectiveOnComponent {
    /// The directive string, e.g. `"class:active"` or `"style:color"`.
    directive: String,
    /// The component name.
    component: String,
    /// Span of the directive.
    span: Span,
  },
}

impl ValidationWarning {
  /// Returns the primary span for this warning (the "duplicate" or offending location).
  #[must_use]
  pub fn span(&self) -> Span {
    match self {
      Self::DuplicateAttribute { dup_span, .. }
      | Self::DuplicateEventHandler { dup_span, .. }
      | Self::DuplicateBinding { dup_span, .. }
      | Self::DuplicateClassToggle { dup_span, .. }
      | Self::DuplicateStyleProp { dup_span, .. }
      | Self::MultipleDefaultSlots { dup_span, .. }
      | Self::DuplicateNamedSlot { dup_span, .. } => *dup_span,
      Self::ConflictingPassivePreventDefault { span, .. }
      | Self::ConflictingPassiveNonPassive { span, .. }
      | Self::EmptyExpression { span }
      | Self::EmptyEventHandler { span, .. }
      | Self::EmptyStylePropValue { span, .. }
      | Self::EmptyClassToggleCondition { span, .. }
      | Self::EmptyBindingExpression { span, .. }
      | Self::EmptyComponentProp { span, .. }
      | Self::DirectiveOnComponent { span, .. } => *span,
    }
  }
}
