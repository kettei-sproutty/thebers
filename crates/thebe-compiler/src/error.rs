use thebe_ast::ParseError;
use thebe_ast::Span;

/// Errors produced during the lowering pass (AST → IR).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompileError {
  /// A parse error propagated from `thebe-ast`.
  #[error(transparent)]
  Parse(#[from] ParseError),

  /// An event modifier string is not a recognised modifier.
  #[error("unknown event modifier '{modifier}' at byte {}", span.start)]
  UnknownEventModifier {
    /// The unrecognised modifier text.
    modifier: String,
    /// Byte-offset span of the directive carrying the modifier.
    span: Span,
  },
}

impl CompileError {
  /// Returns the span associated with this error, if available.
  #[must_use]
  pub fn span(&self) -> Option<Span> {
    match self {
      CompileError::Parse(e) => e.span(),
      CompileError::UnknownEventModifier { span, .. } => Some(*span),
    }
  }
}
