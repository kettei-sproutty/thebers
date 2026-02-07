use crate::types::Attribute;
use crate::types::Directive;
use crate::types::DirectiveKind;
use crate::types::Element;
use crate::types::HtmlNode;
use crate::types::IfBranch;
use crate::types::ParseError;
use crate::types::Span;
use crate::types::TemplateNode;
use crate::types::is_whitespace;

/// Parse a template fragment into a sequence of [`TemplateNode`]s.
///
/// Splits `fragment` on `{{ ... }}` delimiters. Everything outside
/// becomes [`TemplateNode::Text`]; everything inside becomes
/// [`TemplateNode::Expr`] with the expression trimmed.
///
/// `base_offset` is the byte offset of `fragment` within the original
/// `.trs` input, used to produce correct [`Span`]s.
///
/// # Errors
///
/// Returns [`ParseError::UnclosedInterpolation`] if `{{` appears
/// without a matching `}}`.
pub fn parse_template(fragment: &str, base_offset: usize) -> Result<Vec<TemplateNode>, ParseError> {
  let mut nodes = Vec::new();
  let mut pos = 0;

  while pos < fragment.len() {
    match fragment[pos..].find("{{") {
      None => {
        // Rest is plain text.
        nodes.push(TemplateNode::Text(fragment[pos..].to_string()));
        break;
      }
      Some(text_len) => {
        // Push any text before the `{{`.
        if text_len > 0 {
          nodes.push(TemplateNode::Text(
            fragment[pos..pos + text_len].to_string(),
          ));
        }

        let open = pos + text_len;
        let after_open = open + 2; // skip `{{`

        // Find matching `}}`.
        let close =
          fragment[after_open..]
            .find("}}")
            .ok_or_else(|| ParseError::UnclosedInterpolation {
              span: Span::new(base_offset + open, base_offset + after_open),
            })?;

        let expr_raw = &fragment[after_open..after_open + close];
        let expr = expr_raw.trim().to_string();
        let span = Span::new(
          base_offset + open,
          base_offset + after_open + close + 2, // include `}}`
        );

        nodes.push(TemplateNode::Expr { expr, span });
        pos = after_open + close + 2;
      }
    }
  }

  Ok(nodes)
}

// ---------------------------------------------------------------------------
// Template HTML parser
// ---------------------------------------------------------------------------

/// Known HTML void elements that are self-closing (no closing tag required).
const VOID_ELEMENTS: &[&str] = &[
  "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
  "track", "wbr",
];

/// Parse a template fragment string into an HTML node tree.
///
/// This is a lightweight, non-validating parser that handles:
/// - Opening and closing tags with attributes
/// - Self-closing tags (`<br />`, `<img ... />`)
/// - HTML void elements (`<img>`, `<br>`, `<input>`)
/// - Event directives (`on:click="handler"`)
/// - `{{ expr }}` interpolations in text and attribute values
/// - Nested elements
///
/// `base_offset` is the byte offset of `fragment` within the original
/// `.trs` input, used to produce correct [`Span`]s.
///
/// # Errors
///
/// Returns [`ParseError::UnclosedInterpolation`] if `{{` appears
/// without a matching `}}`.
///
/// Returns [`ParseError::MalformedTag`] if a tag is opened but not
/// properly closed.
pub fn parse_html(fragment: &str, base_offset: usize) -> Result<Vec<HtmlNode>, ParseError> {
  let mut parser = HtmlParser {
    input: fragment,
    pos: 0,
    base_offset,
  };
  parser.parse_nodes(None)
}

struct HtmlParser<'a> {
  input: &'a str,
  pos: usize,
  base_offset: usize,
}

impl<'a> HtmlParser<'a> {
  fn remaining(&self) -> &'a str {
    &self.input[self.pos..]
  }

  fn abs_pos(&self) -> usize {
    self.base_offset + self.pos
  }

  /// Parse nodes until we hit the closing tag for `parent` or end-of-input.
  fn parse_nodes(&mut self, parent: Option<&str>) -> Result<Vec<HtmlNode>, ParseError> {
    let mut nodes = Vec::new();

    while self.pos < self.input.len() {
      let remaining = self.remaining();

      // Check for closing tag.
      if let Some(parent_tag) = parent {
        let close = format!("</{parent_tag}");
        if remaining.starts_with(&close) {
          // Verify it's actually the closing tag (not e.g. `</divx`).
          let after_name = &remaining[close.len()..];
          if after_name.starts_with('>') || after_name.starts_with(char::is_whitespace) {
            // Don't consume it — the caller will.
            break;
          }
        }
      }

      // Control-flow boundaries end the current child list.
      if remaining.starts_with("{:") || remaining.starts_with("{/") {
        break;
      }

      // Control-flow opening blocks.
      if remaining.starts_with("{#if ") {
        let node = self.parse_if_block()?;
        nodes.push(node);
        continue;
      }
      if remaining.starts_with("{#each ") {
        let node = self.parse_each_block()?;
        nodes.push(node);
        continue;
      }

      // Check for opening tag.
      if remaining.starts_with('<') && !remaining.starts_with("</") {
        let element = self.parse_element()?;
        if element.tag == "slot" {
          nodes.push(Self::element_to_slot(element));
        } else if element.tag.starts_with(char::is_uppercase) {
          nodes.push(HtmlNode::Component(element));
        } else {
          nodes.push(HtmlNode::Element(element));
        }
        continue;
      }

      // Check for interpolation.
      if remaining.starts_with("{{") {
        let node = self.parse_expr()?;
        nodes.push(node);
        continue;
      }

      // Accumulate text until next special sequence.
      let text = self.parse_text();
      if !text.is_empty() {
        nodes.push(HtmlNode::Text(text));
      }
    }

    Ok(nodes)
  }

  /// Parse a `{{ expr }}` interpolation node.
  fn parse_expr(&mut self) -> Result<HtmlNode, ParseError> {
    let start = self.abs_pos();
    let after_open = self.pos + 2; // skip `{{`

    let close =
      self.input[after_open..]
        .find("}}")
        .ok_or_else(|| ParseError::UnclosedInterpolation {
          span: Span::new(start, start + 2),
        })?;

    let expr = self.input[after_open..after_open + close]
      .trim()
      .to_string();
    let end = after_open + close + 2;
    let span = Span::new(start, self.base_offset + end);

    self.pos = end;
    Ok(HtmlNode::Expr { expr, span })
  }

  /// Parse plain text until the next special sequence.
  fn parse_text(&mut self) -> String {
    let start = self.pos;
    while self.pos < self.input.len() {
      let remaining = self.remaining();
      if remaining.starts_with('<')
        || remaining.starts_with("{{")
        || remaining.starts_with("{#")
        || remaining.starts_with("{:")
        || remaining.starts_with("{/")
      {
        break;
      }
      self.pos += remaining.chars().next().unwrap().len_utf8();
    }
    self.input[start..self.pos].to_string()
  }

  /// Convert a parsed `<slot>` [`Element`] into an [`HtmlNode::Slot`].
  ///
  /// Extracts the `name` attribute (if any) and maps children/span.
  fn element_to_slot(element: Element) -> HtmlNode {
    let name = element
      .attributes
      .iter()
      .find(|a| a.name == "name")
      .and_then(|a| {
        a.value.first().and_then(|v| match v {
          TemplateNode::Text(t) => Some(t.clone()),
          TemplateNode::Expr { .. } => None,
        })
      });

    HtmlNode::Slot {
      name,
      children: element.children,
      span: element.span,
    }
  }

  /// Parse an opening tag, its children, and its closing tag.
  fn parse_element(&mut self) -> Result<Element, ParseError> {
    let elem_start = self.abs_pos();

    // Skip '<'
    self.pos += 1;

    // Parse tag name.
    let tag_start = self.pos;
    while self.pos < self.input.len() {
      let ch = self.input[self.pos..].chars().next().unwrap();
      if ch == '>' || ch == '/' || is_whitespace(ch) {
        break;
      }
      self.pos += ch.len_utf8();
    }
    let tag = self.input[tag_start..self.pos].to_string();

    // Parse attributes and directives.
    let mut attributes = Vec::new();
    let mut directives = Vec::new();
    self.skip_whitespace();

    while self.pos < self.input.len() {
      let remaining = self.remaining();
      if remaining.starts_with('>') || remaining.starts_with("/>") {
        break;
      }
      self.parse_attr_or_directive(&mut attributes, &mut directives)?;
      self.skip_whitespace();
    }

    // Check for self-closing `/>` or void element.
    let explicit_self_close = self.remaining().starts_with("/>");
    if explicit_self_close {
      self.pos += 2; // skip `/>`
    } else if self.remaining().starts_with('>') {
      self.pos += 1; // skip `>`
    } else {
      return Err(ParseError::MalformedTag {
        detail: format!("unclosed opening tag <{tag}>"),
        span: Span::new(elem_start, self.abs_pos()),
      });
    }

    let is_void = VOID_ELEMENTS.contains(&tag.as_str());
    let self_closing = explicit_self_close || is_void;

    let children = if self_closing {
      Vec::new()
    } else {
      // Parse children until closing tag.
      let children = self.parse_nodes(Some(&tag))?;

      // Consume closing tag.
      let close_tag = format!("</{tag}>");
      if !self.remaining().starts_with(&close_tag) {
        return Err(ParseError::MalformedTag {
          detail: format!("missing closing tag </{tag}>"),
          span: Span::new(elem_start, self.abs_pos()),
        });
      }
      self.pos += close_tag.len();

      children
    };

    let elem_end = self.abs_pos();

    Ok(Element {
      tag,
      attributes,
      directives,
      children,
      self_closing,
      span: Span::new(elem_start, elem_end),
    })
  }

  /// Parse a single attribute or directive from the current position.
  fn parse_attr_or_directive(
    &mut self,
    attributes: &mut Vec<Attribute>,
    directives: &mut Vec<Directive>,
  ) -> Result<(), ParseError> {
    let attr_start = self.abs_pos();

    // Parse attribute/directive name.
    let name_start = self.pos;
    while self.pos < self.input.len() {
      let ch = self.input[self.pos..].chars().next().unwrap();
      if ch == '=' || ch == '>' || ch == '/' || is_whitespace(ch) {
        break;
      }
      self.pos += ch.len_utf8();
    }
    let full_name = &self.input[name_start..self.pos];

    // Boolean attribute (no `=`)?
    if !self.remaining().starts_with('=') {
      // Could be a boolean attribute like `scoped` or a valueless directive.
      let span = Span::new(attr_start, self.abs_pos());
      if let Some((kind, name, modifiers)) = parse_directive_name(full_name) {
        directives.push(Directive {
          kind,
          name,
          modifiers,
          value: String::new(),
          span,
        });
      } else {
        attributes.push(Attribute {
          name: full_name.to_string(),
          value: vec![],
          span,
        });
      }
      return Ok(());
    }

    // Skip `=`
    self.pos += 1;

    // Parse quoted value.
    if !self.remaining().starts_with('"') {
      return Err(ParseError::MalformedTag {
        detail: format!("expected '\"' after {full_name}="),
        span: Span::new(attr_start, self.abs_pos()),
      });
    }
    self.pos += 1; // skip opening `"`

    let val_start = self.pos;
    // Find the closing quote.
    let close_quote = self.input[self.pos..]
      .find('"')
      .ok_or_else(|| ParseError::MalformedTag {
        detail: format!("unclosed attribute value for {full_name}"),
        span: Span::new(attr_start, self.abs_pos()),
      })?;
    let val_str = &self.input[val_start..val_start + close_quote];
    self.pos = val_start + close_quote + 1; // skip closing `"`

    let attr_end = self.abs_pos();
    let span = Span::new(attr_start, attr_end);

    if let Some((kind, name, modifiers)) = parse_directive_name(full_name) {
      directives.push(Directive {
        kind,
        name,
        modifiers,
        value: val_str.to_string(),
        span,
      });
    } else {
      // Parse the value for interpolations.
      let val_base_offset = self.base_offset + val_start;
      let value = parse_template(val_str, val_base_offset)?;
      attributes.push(Attribute {
        name: full_name.to_string(),
        value,
        span,
      });
    }

    Ok(())
  }

  // -----------------------------------------------------------------------
  // Control-flow blocks
  // -----------------------------------------------------------------------

  /// Parse a control-flow block tag like `{#if condition}` or `{:else if cond}`.
  ///
  /// `prefix` is the portion after `{`, e.g. `"#if"` or `":else if"`.
  /// Returns the trimmed content between the prefix and the closing `}`.
  fn parse_block_tag(&mut self, prefix: &str) -> Result<String, ParseError> {
    let start = self.abs_pos();
    // Skip `{` + prefix.
    self.pos += 1 + prefix.len();

    let rest = self.remaining();
    let close = rest.find('}').ok_or_else(|| ParseError::MalformedTag {
      detail: format!("unclosed {{{prefix}}}"),
      span: Span::new(start, self.abs_pos()),
    })?;

    let content = rest[..close].trim().to_string();
    self.pos += close + 1; // skip past `}`
    Ok(content)
  }

  /// Parse an `{#if}` / `{:else if}` / `{:else}` / `{/if}` block.
  fn parse_if_block(&mut self) -> Result<HtmlNode, ParseError> {
    let block_start = self.abs_pos();
    let mut branches = Vec::new();
    let mut had_else = false;

    // Parse initial `{#if condition}`.
    let mut current_condition: Option<String> = Some(self.parse_block_tag("#if")?);

    loop {
      let branch_start = self.abs_pos();
      let children = self.parse_nodes(None)?;
      let branch_end = self.abs_pos();

      branches.push(IfBranch {
        condition: current_condition.take(),
        children,
        span: Span::new(branch_start, branch_end),
      });

      if had_else {
        // After `{:else}`, only `{/if}` is valid.
        if !self.remaining().starts_with("{/if}") {
          return Err(ParseError::UnclosedIfBlock {
            span: Span::new(block_start, self.abs_pos()),
          });
        }
        self.pos += "{/if}".len();
        break;
      }

      let remaining = self.remaining();
      if remaining.starts_with("{:else if ") {
        current_condition = Some(self.parse_block_tag(":else if")?);
      } else if remaining.starts_with("{:else") {
        let _ = self.parse_block_tag(":else")?;
        current_condition = None;
        had_else = true;
      } else if remaining.starts_with("{/if}") {
        self.pos += "{/if}".len();
        break;
      } else {
        return Err(ParseError::UnclosedIfBlock {
          span: Span::new(block_start, self.abs_pos()),
        });
      }
    }

    let block_end = self.abs_pos();
    Ok(HtmlNode::If {
      branches,
      span: Span::new(block_start, block_end),
    })
  }

  /// Parse an `{#each iterable as binding}` / `{/each}` block.
  fn parse_each_block(&mut self) -> Result<HtmlNode, ParseError> {
    let block_start = self.abs_pos();
    let tag_content = self.parse_block_tag("#each")?;

    let (iterable, binding, index) =
      parse_each_expr(&tag_content).ok_or_else(|| ParseError::InvalidEachExpression {
        detail: tag_content.clone(),
        span: Span::new(block_start, self.abs_pos()),
      })?;

    let children = self.parse_nodes(None)?;

    if !self.remaining().starts_with("{/each}") {
      return Err(ParseError::UnclosedEachBlock {
        span: Span::new(block_start, self.abs_pos()),
      });
    }
    self.pos += "{/each}".len();

    let block_end = self.abs_pos();
    Ok(HtmlNode::Each {
      iterable,
      binding,
      index,
      children,
      span: Span::new(block_start, block_end),
    })
  }

  fn skip_whitespace(&mut self) {
    while self.pos < self.input.len() {
      let ch = self.input[self.pos..].chars().next().unwrap();
      if is_whitespace(ch) {
        self.pos += ch.len_utf8();
      } else {
        break;
      }
    }
  }
}

/// Try to parse a directive name like `on:click` or `on:click|preventDefault`
/// into `(DirectiveKind, arg, modifiers)`.
#[allow(clippy::manual_map)]
fn parse_directive_name(name: &str) -> Option<(DirectiveKind, String, Vec<String>)> {
  if let Some(rest) = name.strip_prefix("on:") {
    let mut parts = rest.split('|');
    let event = parts.next().unwrap_or_default().to_string();
    let modifiers: Vec<String> = parts.map(String::from).collect();
    Some((DirectiveKind::On, event, modifiers))
  } else if let Some(prop) = name.strip_prefix("bind:") {
    Some((DirectiveKind::Bind, prop.to_string(), Vec::new()))
  } else if let Some(cls) = name.strip_prefix("class:") {
    Some((DirectiveKind::Class, cls.to_string(), Vec::new()))
  } else if let Some(prop) = name.strip_prefix("style:") {
    Some((DirectiveKind::Style, prop.to_string(), Vec::new()))
  } else if let Some(action) = name.strip_prefix("use:") {
    Some((DirectiveKind::Use, action.to_string(), Vec::new()))
  } else {
    None
  }
}

/// Parse `"iterable as binding"` or `"iterable as binding, index"`.
fn parse_each_expr(expr: &str) -> Option<(String, String, Option<String>)> {
  let (iterable, rest) = expr.split_once(" as ")?;
  let iterable = iterable.trim().to_string();
  let rest = rest.trim();

  if let Some((binding, index)) = rest.split_once(',') {
    Some((
      iterable,
      binding.trim().to_string(),
      Some(index.trim().to_string()),
    ))
  } else {
    Some((iterable, rest.to_string(), None))
  }
}
