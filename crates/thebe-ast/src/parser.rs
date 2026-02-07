use crate::template::parse_template;
use crate::types::ParseError;
use crate::types::ScriptBlock;
use crate::types::Span;
use crate::types::StyleBlock;
use crate::types::TemplateFragment;
use crate::types::ThebeAst;
use crate::types::is_whitespace;

/// Parse a `.trs` file into a [`ThebeAst`].
///
/// # Errors
///
/// Returns [`ParseError::EmptyInput`] if the input is empty or whitespace-only.
///
/// Returns [`ParseError::DuplicateScriptSetup`] if more than one
/// `<script setup>` block is found.
///
/// Returns [`ParseError::DuplicateScript`] if more than one
/// `<script>` block is found.
///
/// Returns [`ParseError::NestedScript`] if a `<script>` tag appears
/// inside template content.
///
/// Returns [`ParseError::MalformedTag`] if any recognized tag is
/// opened but not properly closed.
///
/// Returns [`ParseError::InvalidSetupLang`] if a `lang` attribute
/// is used on `<script setup>` (which is always Rust).
///
/// # Examples
///
/// ```
/// let input = r#"
/// <script setup>
/// let x: i32 = 1;
/// </script>
///
/// <div>Hello</div>
///
/// <style scoped>
/// .red { color: red; }
/// </style>
/// "#;
///
/// let ast = thebe_ast::parse(input).unwrap();
/// assert_eq!(ast.script_setup.as_ref().unwrap().content, "let x: i32 = 1;");
/// assert_eq!(ast.script_setup.as_ref().unwrap().lang, None);
/// assert!(!ast.template.is_empty());
/// assert!(ast.styles[0].scoped);
/// ```
pub fn parse(input: &str) -> Result<ThebeAst, ParseError> {
  if input.trim().is_empty() {
    return Err(ParseError::EmptyInput);
  }

  let mut ast = ThebeAst {
    script_setup: None,
    script: None,
    styles: Vec::new(),
    template: Vec::new(),
  };

  let mut remaining = input;
  // Track how far into `input` we've consumed, for byte-offset spans.
  let mut offset = 0usize;

  while !remaining.is_empty() {
    match find_next_block(remaining) {
      None => {
        push_template_fragment(&mut ast.template, remaining, offset)?;
        break;
      }
      Some((kind, tag_start, tag_end)) => {
        let abs_tag_start = offset + tag_start;

        // Text before the tag is a template fragment.
        let before = remaining[..tag_start].trim();
        check_nested_script(kind, before, Span::new(abs_tag_start, abs_tag_start))?;
        push_template_fragment(&mut ast.template, &remaining[..tag_start], offset)?;

        let open_tag = &remaining[tag_start..tag_end];
        let close_tag = kind.close_tag();
        let after_open = &remaining[tag_end..];

        let close_pos = after_open
          .find(close_tag)
          .ok_or_else(|| ParseError::MalformedTag {
            detail: format!("unclosed {open_tag}"),
            span: Span::new(abs_tag_start, offset + tag_end),
          })?;

        let abs_block_end = offset + tag_end + close_pos + close_tag.len();
        let block_span = Span::new(abs_tag_start, abs_block_end);

        let content = after_open[..close_pos].trim().to_string();
        remaining = &after_open[close_pos + close_tag.len()..];
        offset = abs_block_end;

        match kind {
          BlockKind::ScriptSetup => {
            if ast.script_setup.is_some() {
              return Err(ParseError::DuplicateScriptSetup { span: block_span });
            }
            if extract_attr(open_tag, "lang").is_some() {
              return Err(ParseError::InvalidSetupLang { span: block_span });
            }
            ast.script_setup = Some(ScriptBlock {
              lang: None,
              content,
              span: block_span,
            });
          }
          BlockKind::Script => {
            if ast.script.is_some() {
              return Err(ParseError::DuplicateScript { span: block_span });
            }
            let lang = extract_attr(open_tag, "lang")
              .ok_or(ParseError::MissingScriptLang { span: block_span })?;
            ast.script = Some(ScriptBlock {
              lang: Some(lang),
              content,
              span: block_span,
            });
          }
          BlockKind::Style => {
            ast.styles.push(StyleBlock {
              scoped: has_bool_attr(open_tag, "scoped"),
              lang: extract_attr(open_tag, "lang"),
              content,
              span: block_span,
            });
          }
        }
      }
    }
  }

  Ok(ast)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum BlockKind {
  ScriptSetup,
  Script,
  Style,
}

impl BlockKind {
  fn close_tag(self) -> &'static str {
    match self {
      BlockKind::ScriptSetup | BlockKind::Script => "</script>",
      BlockKind::Style => "</style>",
    }
  }
}

/// Tag names we recognise and their minimum prefix.
const TAG_CANDIDATES: &[(&str, BlockKind)] = &[
  ("script setup", BlockKind::ScriptSetup),
  ("script", BlockKind::Script),
  ("style", BlockKind::Style),
];

/// Find the earliest opening tag for a recognised block in `input`.
///
/// Returns `(BlockKind, byte offset of '<', byte offset after '>')`.
fn find_next_block(input: &str) -> Option<(BlockKind, usize, usize)> {
  let mut pos = 0;
  while pos < input.len() {
    let Some(lt) = input[pos..].find('<') else {
      break;
    };
    let abs = pos + lt;
    let rest = &input[abs..];

    if let Some((kind, tag_end)) = try_match_tag(rest) {
      return Some((kind, abs, abs + tag_end));
    }

    pos = abs + 1;
  }
  None
}

/// Try to match `rest` (starting with `<`) against known tag names.
///
/// Returns `(BlockKind, offset after the closing '>')` on success.
fn try_match_tag(rest: &str) -> Option<(BlockKind, usize)> {
  for &(name, kind) in TAG_CANDIDATES {
    let prefix = &rest[1..]; // skip '<'
    if !prefix.starts_with(name) {
      continue;
    }
    // The char right after the name must be a tag boundary.
    if !is_tag_boundary(prefix, name.len()) {
      continue;
    }
    // Find the closing '>' of the opening tag.
    let tag_end = rest.find('>')? + 1;
    return Some((kind, tag_end));
  }
  None
}

/// Check that the character at `offset` in `s` is a valid tag-name
/// terminator (`>`, whitespace, or end-of-string).
fn is_tag_boundary(s: &str, offset: usize) -> bool {
  s[offset..]
    .chars()
    .next()
    .is_none_or(|c| c == '>' || is_whitespace(c))
}

/// Push a trimmed template fragment, rejecting nested `<script>` tags
/// and parsing `{{ expr }}` interpolations.
fn push_template_fragment(
  fragments: &mut Vec<TemplateFragment>,
  raw: &str,
  base_offset: usize,
) -> Result<(), ParseError> {
  let trimmed = raw.trim();
  if !trimmed.is_empty() {
    if contains_script_tag(trimmed) {
      return Err(ParseError::NestedScript {
        span: Span::new(0, 0),
      });
    }
    // Compute the byte offset of the trimmed content within the original input.
    let trim_start = base_offset + (raw.len() - raw.trim_start().len());
    let nodes = parse_template(trimmed, trim_start)?;
    fragments.push(TemplateFragment { nodes });
  }
  Ok(())
}

/// If we're about to consume a `<script>` block and the preceding
/// template text has unclosed HTML tags, the script is nested.
fn check_nested_script(kind: BlockKind, before: &str, span: Span) -> Result<(), ParseError> {
  if matches!(kind, BlockKind::Script | BlockKind::ScriptSetup)
    && !before.is_empty()
    && html_tag_depth(before) > 0
  {
    return Err(ParseError::NestedScript { span });
  }
  Ok(())
}

/// Extract a `name="value"` attribute from an opening tag string.
fn extract_attr(tag: &str, name: &str) -> Option<String> {
  let needle = format!("{name}=\"");
  let start = tag.find(&needle)? + needle.len();
  let rest = &tag[start..];
  let end = rest.find('"')?;
  Some(rest[..end].to_string())
}

/// Check if a tag string contains a standalone boolean attribute.
///
/// The attribute must be preceded by whitespace and followed by
/// whitespace, `>`, `/`, or end-of-string.  This prevents false
/// positives like `data-scoped` or `unscoped` matching `scoped`.
fn has_bool_attr(tag: &str, name: &str) -> bool {
  let mut pos = 0;
  while let Some(offset) = tag[pos..].find(name) {
    let abs = pos + offset;
    // Must be preceded by whitespace (not part of a longer word).
    let before_ok = abs > 0
      && tag[..abs]
        .ends_with(|c: char| c.is_ascii_whitespace());
    // Must be followed by boundary: whitespace, `>`, `/`, `=`, or end.
    let after = abs + name.len();
    let after_ok = after >= tag.len()
      || tag[after..]
        .starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/' || c == '=');
    if before_ok && after_ok {
      return true;
    }
    pos = abs + 1;
  }
  false
}

/// Check if text contains a top-level `<script` tag.
fn contains_script_tag(text: &str) -> bool {
  let mut pos = 0;
  while pos < text.len() {
    let Some(offset) = text[pos..].find("<script") else {
      break;
    };
    let abs = pos + offset;
    if is_tag_boundary(&text[abs + 1..], "script".len()) {
      return true;
    }
    pos = abs + 1;
  }
  false
}

/// Net HTML tag depth of a text fragment.
///
/// Opening tags count as +1, closing tags as −1,
/// self-closing tags (`<br/>`) as 0.
fn html_tag_depth(text: &str) -> i32 {
  let mut depth: i32 = 0;
  let mut pos = 0;
  while pos < text.len() {
    let Some(lt) = text[pos..].find('<') else {
      break;
    };
    let abs = pos + lt;
    let rest = &text[abs..];

    if rest.starts_with("</") || rest.starts_with("<!") {
      // Closing tag or comment/doctype.
      if rest.starts_with("</") {
        depth -= 1;
      }
      pos = abs + rest.find('>').map_or(rest.len(), |p| p + 1);
    } else if let Some(gt) = rest.find('>') {
      if !rest[..gt].ends_with('/') {
        depth += 1;
      }
      pos = abs + gt + 1;
    } else {
      pos = abs + 1;
    }
  }
  depth
}
