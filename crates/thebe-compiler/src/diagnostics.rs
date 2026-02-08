//! Pretty error reporting for compiler errors and validation warnings
//! using [ariadne](https://docs.rs/ariadne).
//!
//! This module converts [`CompileError`] and [`ValidationWarning`] values
//! into richly-formatted, source-annotated diagnostics, following the
//! same pattern as `thebe_ast::diagnostics`.

use std::io;
use std::ops::Range;

use ariadne::Config;
use ariadne::IndexType;
use ariadne::Label;
use ariadne::Report;
use ariadne::ReportKind;
use ariadne::Source;

use crate::error::CompileError;
use crate::warning::ValidationWarning;

/// Describes the diagnostic style for a single [`CompileError`] variant.
struct DiagInfo {
  range: Range<usize>,
  message: &'static str,
  label: String,
  note: Option<&'static str>,
  help: Option<&'static str>,
}

/// Extract the diagnostic metadata from a [`CompileError`].
fn diag_info(error: &CompileError) -> DiagInfo {
  match error {
    CompileError::Parse(parse_err) => {
      // Delegate to the range from the underlying parse error span.
      let span = parse_err.span().unwrap_or(thebe_ast::Span::new(0, 0));
      DiagInfo {
        range: span.start..span.end,
        message: "parse error",
        label: parse_err.to_string(),
        note: None,
        help: None,
      }
    }
    CompileError::UnknownEventModifier { modifier, span } => DiagInfo {
      range: span.start..span.end,
      message: "unknown event modifier",
      label: format!("'{modifier}' is not a recognised modifier"),
      note: None,
      help: Some(
        "known modifiers: preventDefault, stopPropagation, once, capture, self, trusted, passive, nonpassive",
      ),
    },
  }
}

/// Build an ariadne [`Report`] from a [`CompileError`] using the given [`Config`].
fn make_report(
  error: &CompileError,
  fname: &str,
  config: Config,
) -> Report<'static, (String, Range<usize>)> {
  let info = diag_info(error);
  let fname = fname.to_string();

  let mut builder = Report::build(ReportKind::Error, (fname.clone(), info.range.clone()))
    .with_config(config)
    .with_message(info.message)
    .with_label(Label::new((fname, info.range)).with_message(info.label));

  if let Some(note) = info.note {
    builder = builder.with_note(note);
  }
  if let Some(help) = info.help {
    builder = builder.with_help(help);
  }

  builder.finish()
}

/// Build a colored ariadne [`Report`] from a [`CompileError`].
fn build_report(
  error: &CompileError,
  filename: Option<&str>,
) -> Report<'static, (String, Range<usize>)> {
  let fname = filename.unwrap_or("<unknown>");
  let config = Config::default().with_index_type(IndexType::Byte);
  make_report(error, fname, config)
}

/// Print a pretty diagnostic for `error` to **stderr**.
///
/// # Errors
///
/// Returns an I/O error if writing to stderr fails.
pub fn eprint_error(
  error: &CompileError,
  source: &str,
  filename: Option<&str>,
) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_report(error, filename);
  report.eprint((fname, Source::from(source)))
}

/// Print a pretty diagnostic for `error` to **stdout**.
///
/// # Errors
///
/// Returns an I/O error if writing to stdout fails.
pub fn print_error(
  error: &CompileError,
  source: &str,
  filename: Option<&str>,
) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_report(error, filename);
  report.print((fname, Source::from(source)))
}

/// Write a pretty diagnostic for `error` to an arbitrary [`io::Write`] destination.
///
/// # Errors
///
/// Returns an I/O error if writing to `writer` fails.
pub fn write_error<W: io::Write>(
  error: &CompileError,
  source: &str,
  filename: Option<&str>,
  writer: W,
) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_report(error, filename);
  report.write((fname, Source::from(source)), writer)
}

/// Render a pretty diagnostic to a [`String`] (no ANSI colors).
///
/// Useful for testing and logging.
///
/// # Panics
///
/// Panics if writing to an in-memory buffer fails (should never happen).
#[must_use]
pub fn error_to_string(error: &CompileError, source: &str, filename: Option<&str>) -> String {
  let fname = filename.unwrap_or("<unknown>");
  let config = Config::default()
    .with_index_type(IndexType::Byte)
    .with_color(false)
    .with_char_set(ariadne::CharSet::Ascii);

  let report = make_report(error, fname, config);
  let mut buf = Vec::new();
  report
    .write((fname.to_string(), Source::from(source)), &mut buf)
    .expect("writing to Vec<u8> should not fail");
  String::from_utf8(buf).expect("ariadne output should be valid UTF-8")
}

// ---------------------------------------------------------------------------
// Validation warning diagnostics
// ---------------------------------------------------------------------------

/// Describes the diagnostic style for a single [`ValidationWarning`].
struct WarnDiagInfo {
  range: Range<usize>,
  message: &'static str,
  label: String,
  first_range: Option<Range<usize>>,
  help: Option<&'static str>,
}

/// Extract the diagnostic metadata from a [`ValidationWarning`].
#[allow(clippy::too_many_lines)]
fn warn_diag_info(warning: &ValidationWarning) -> WarnDiagInfo {
  match warning {
    ValidationWarning::DuplicateAttribute {
      name,
      tag,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate attribute",
      label: format!("'{name}' already set on <{tag}>"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("remove one of the duplicate attributes"),
    },
    ValidationWarning::DuplicateEventHandler {
      event,
      tag,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate event handler",
      label: format!("'on:{event}' already bound on <{tag}>"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("remove one of the duplicate handlers"),
    },
    ValidationWarning::DuplicateBinding {
      property,
      tag,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate binding",
      label: format!("'bind:{property}' already bound on <{tag}>"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("remove one of the duplicate bindings"),
    },
    ValidationWarning::DuplicateClassToggle {
      class,
      tag,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate class toggle",
      label: format!("'class:{class}' already set on <{tag}>"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("remove one of the duplicate class toggles"),
    },
    ValidationWarning::DuplicateStyleProp {
      property,
      tag,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate style prop",
      label: format!("'style:{property}' already set on <{tag}>"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("remove one of the duplicate style properties"),
    },
    ValidationWarning::ConflictingPassivePreventDefault { event, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "conflicting event modifiers",
      label: format!("'passive' and 'preventDefault' on 'on:{event}'"),
      first_range: None,
      help: Some("passive listeners cannot call preventDefault — remove one modifier"),
    },
    ValidationWarning::ConflictingPassiveNonPassive { event, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "conflicting event modifiers",
      label: format!("'passive' and 'nonpassive' on 'on:{event}'"),
      first_range: None,
      help: Some("choose either passive or nonpassive, not both"),
    },
    ValidationWarning::EmptyExpression { span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "empty expression",
      label: "interpolation contains no expression".into(),
      first_range: None,
      help: Some("add an expression inside {{ }}"),
    },
    ValidationWarning::EmptyEventHandler { event, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "empty event handler",
      label: format!("'on:{event}' has no handler"),
      first_range: None,
      help: Some("provide a handler expression, e.g. on:click=\"handle_click\""),
    },
    ValidationWarning::EmptyStylePropValue { property, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "empty style prop value",
      label: format!("'style:{property}' has no value expression"),
      first_range: None,
      help: Some("provide a value expression, e.g. style:color=\"theme_color\""),
    },
    ValidationWarning::EmptyClassToggleCondition { class, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "empty class toggle condition",
      label: format!("'class:{class}' has no condition"),
      first_range: None,
      help: Some("provide a condition expression, e.g. class:active=\"is_active\""),
    },
    ValidationWarning::EmptyBindingExpression { property, span } => WarnDiagInfo {
      range: span.start..span.end,
      message: "empty binding expression",
      label: format!("'bind:{property}' has no expression"),
      first_range: None,
      help: Some("provide a binding expression, e.g. bind:value=\"name\""),
    },
    ValidationWarning::MultipleDefaultSlots {
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "multiple default slots",
      label: "second default <slot> found here".into(),
      first_range: Some(first_span.start..first_span.end),
      help: Some("only one default slot is allowed per component — name the others"),
    },
    ValidationWarning::DuplicateNamedSlot {
      name,
      first_span,
      dup_span,
    } => WarnDiagInfo {
      range: dup_span.start..dup_span.end,
      message: "duplicate named slot",
      label: format!("slot '{name}' already defined"),
      first_range: Some(first_span.start..first_span.end),
      help: Some("each named slot should appear only once"),
    },
  }
}

/// Build an ariadne [`Report`] (warning level) from a [`ValidationWarning`].
fn make_warning_report(
  warning: &ValidationWarning,
  fname: &str,
  config: Config,
) -> Report<'static, (String, Range<usize>)> {
  let info = warn_diag_info(warning);
  let fname = fname.to_string();

  let mut builder = Report::build(ReportKind::Warning, (fname.clone(), info.range.clone()))
    .with_config(config)
    .with_message(info.message)
    .with_label(
      Label::new((fname.clone(), info.range)).with_message(info.label),
    );

  if let Some(first) = info.first_range {
    builder = builder.with_label(
      Label::new((fname, first)).with_message("first defined here"),
    );
  }

  if let Some(help) = info.help {
    builder = builder.with_help(help);
  }

  builder.finish()
}

fn build_warning_report(
  warning: &ValidationWarning,
  filename: Option<&str>,
) -> Report<'static, (String, Range<usize>)> {
  let fname = filename.unwrap_or("<unknown>");
  let config = Config::default().with_index_type(IndexType::Byte);
  make_warning_report(warning, fname, config)
}

/// Print a pretty diagnostic for a validation `warning` to **stderr**.
///
/// # Errors
///
/// Returns an I/O error if writing to stderr fails.
pub fn eprint_warning(
  warning: &ValidationWarning,
  source: &str,
  filename: Option<&str>,
) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_warning_report(warning, filename);
  report.eprint((fname, Source::from(source)))
}

/// Print a pretty diagnostic for a validation `warning` to **stdout**.
///
/// # Errors
///
/// Returns an I/O error if writing to stdout fails.
pub fn print_warning(
  warning: &ValidationWarning,
  source: &str,
  filename: Option<&str>,
) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_warning_report(warning, filename);
  report.print((fname, Source::from(source)))
}

/// Render a validation warning diagnostic to a [`String`] (no ANSI colors).
///
/// # Panics
///
/// Panics if writing to an in-memory buffer fails (should never happen).
#[must_use]
pub fn warning_to_string(
  warning: &ValidationWarning,
  source: &str,
  filename: Option<&str>,
) -> String {
  let fname = filename.unwrap_or("<unknown>");
  let config = Config::default()
    .with_index_type(IndexType::Byte)
    .with_color(false)
    .with_char_set(ariadne::CharSet::Ascii);

  let report = make_warning_report(warning, fname, config);
  let mut buf = Vec::new();
  report
    .write((fname.to_string(), Source::from(source)), &mut buf)
    .expect("writing to Vec<u8> should not fail");
  String::from_utf8(buf).expect("ariadne output should be valid UTF-8")
}
