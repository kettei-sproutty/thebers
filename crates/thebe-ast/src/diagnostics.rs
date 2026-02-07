//! Pretty error reporting for `.trs` parse errors using [ariadne](https://docs.rs/ariadne).
//!
//! This module converts [`ParseError`] values into richly-formatted,
//! source-annotated diagnostics that can be printed to stderr, stdout,
//! or any [`std::io::Write`] destination.
//!
//! # Examples
//!
//! ```no_run
//! let source = "<script setup>\nlet x = 1;\n</script>\n<script setup>\noops\n</script>";
//! match thebe_ast::parse(source) {
//!     Ok(_) => {}
//!     Err(e) => thebe_ast::diagnostics::eprint_error(&e, source, None).unwrap(),
//! }
//! ```

use std::io;
use std::ops::Range;

use ariadne::Config;
use ariadne::IndexType;
use ariadne::Label;
use ariadne::Report;
use ariadne::ReportKind;
use ariadne::Source;

use crate::types::ParseError;

/// Describes the diagnostic style for a single [`ParseError`] variant.
struct DiagInfo {
  range: Range<usize>,
  message: &'static str,
  label: String,
  note: Option<&'static str>,
  help: Option<&'static str>,
}

/// Extract the diagnostic metadata from a [`ParseError`].
fn diag_info(error: &ParseError) -> DiagInfo {
  match error {
    ParseError::EmptyInput => DiagInfo {
      range: 0..0,
      message: "input is empty",
      label: "expected at least one block or template content".into(),
      note: None,
      help: None,
    },
    ParseError::DuplicateScriptSetup { span } => DiagInfo {
      range: span.start..span.end,
      message: "duplicate <script setup> block",
      label: "second <script setup> found here".into(),
      note: Some("only one <script setup> block is allowed per .trs file"),
      help: None,
    },
    ParseError::DuplicateScript { span } => DiagInfo {
      range: span.start..span.end,
      message: "duplicate <script> block",
      label: "second <script> found here".into(),
      note: Some("only one <script> block is allowed per .trs file"),
      help: None,
    },
    ParseError::NestedScript { span } => DiagInfo {
      range: span.start..span.end,
      message: "<script> tag inside template content",
      label: "nested <script> found here".into(),
      note: Some("<script> tags must be top-level blocks, not nested inside HTML"),
      help: None,
    },
    ParseError::InvalidSetupLang { span } => DiagInfo {
      range: span.start..span.end,
      message: "<script setup> does not accept a lang attribute",
      label: "lang attribute not allowed here".into(),
      note: Some("<script setup> is always Rust \u{2014} remove the lang attribute"),
      help: None,
    },
    ParseError::MissingScriptLang { span } => DiagInfo {
      range: span.start..span.end,
      message: "<script> requires a lang attribute",
      label: "missing lang attribute".into(),
      note: None,
      help: Some("add a lang attribute, e.g. <script lang=\"ts\">"),
    },
    ParseError::UnclosedInterpolation { span } => DiagInfo {
      range: span.start..span.end,
      message: "unclosed interpolation",
      label: "opening {{ has no matching }}".into(),
      note: None,
      help: None,
    },
    ParseError::MalformedTag { detail, span } => DiagInfo {
      range: span.start..span.end,
      message: "malformed tag",
      label: detail.clone(),
      note: None,
      help: None,
    },
    ParseError::UnclosedIfBlock { span } => DiagInfo {
      range: span.start..span.end,
      message: "unclosed {#if} block",
      label: "{#if} opened here has no matching {/if}".into(),
      note: None,
      help: Some("add a closing {/if} tag"),
    },
    ParseError::UnclosedEachBlock { span } => DiagInfo {
      range: span.start..span.end,
      message: "unclosed {#each} block",
      label: "{#each} opened here has no matching {/each}".into(),
      note: None,
      help: Some("add a closing {/each} tag"),
    },
    ParseError::InvalidEachExpression { detail, span } => DiagInfo {
      range: span.start..span.end,
      message: "invalid {#each} expression",
      label: format!("could not parse: {detail}"),
      note: None,
      help: Some(
        "expected {#each iterable as binding} or {#each iterable as binding, index}",
      ),
    },
    ParseError::InvalidSlotAttribute { detail, span } => DiagInfo {
      range: span.start..span.end,
      message: "unsupported attribute on <slot>",
      label: detail.clone(),
      note: Some("only the 'name' attribute is allowed on <slot> elements"),
      help: None,
    },
  }
}

/// Build an ariadne [`Report`] from a [`ParseError`] using the given [`Config`].
fn make_report(
  error: &ParseError,
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

/// Build a colored ariadne [`Report`] from a [`ParseError`].
fn build_report(
  error: &ParseError,
  filename: Option<&str>,
) -> Report<'static, (String, Range<usize>)> {
  let fname = filename.unwrap_or("<unknown>");
  let config = Config::default().with_index_type(IndexType::Byte);
  make_report(error, fname, config)
}

/// Print a pretty diagnostic for `error` to **stderr**.
///
/// `source` is the full `.trs` input text.  `filename` is shown in the
/// diagnostic header (pass `None` for `<unknown>`).
///
/// # Errors
///
/// Returns an I/O error if writing to stderr fails.
pub fn eprint_error(error: &ParseError, source: &str, filename: Option<&str>) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_report(error, filename);
  report.eprint((fname, Source::from(source)))
}

/// Print a pretty diagnostic for `error` to **stdout**.
///
/// `source` is the full `.trs` input text.  `filename` is shown in the
/// diagnostic header (pass `None` for `<unknown>`).
///
/// # Errors
///
/// Returns an I/O error if writing to stdout fails.
pub fn print_error(error: &ParseError, source: &str, filename: Option<&str>) -> io::Result<()> {
  let fname = filename.unwrap_or("<unknown>").to_string();
  let report = build_report(error, filename);
  report.print((fname, Source::from(source)))
}

/// Write a pretty diagnostic for `error` to an arbitrary [`io::Write`] destination.
///
/// `source` is the full `.trs` input text.  `filename` is shown in the
/// diagnostic header (pass `None` for `<unknown>`).
///
/// # Errors
///
/// Returns an I/O error if writing to `writer` fails.
pub fn write_error<W: io::Write>(
  error: &ParseError,
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
/// Useful for testing and logging. Uses ASCII box-drawing characters and
/// disables color output.
///
/// # Panics
///
/// Panics if writing to an in-memory buffer fails (should never happen).
#[must_use]
pub fn error_to_string(error: &ParseError, source: &str, filename: Option<&str>) -> String {
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
