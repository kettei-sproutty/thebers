use std::fs;
use thebe_ast::DirectiveKind;
use thebe_ast::Element;
use thebe_ast::HtmlNode;
use thebe_ast::ParseError;
use thebe_ast::TemplateFragment;
use thebe_ast::TemplateNode;

/// Reconstruct the raw text of a template fragment for simpler assertions.
fn raw(frag: &TemplateFragment) -> String {
  frag
    .nodes
    .iter()
    .map(|n| match n {
      TemplateNode::Text(t) => t.as_str().to_string(),
      TemplateNode::Expr { expr, .. } => format!("{{{{ {expr} }}}}"),
    })
    .collect()
}

#[test]
fn parse_basic_trs_file() {
  let input = fs::read_to_string("tests/samples/basic.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  // script setup is present with expected content
  let setup = ast.script_setup.as_ref().unwrap();
  assert_eq!(setup.content, "let x: i32 = 42;");

  // script block is present (empty content)
  assert!(ast.script.is_some());

  // style block is present with expected content
  assert_eq!(ast.styles.len(), 1);
  assert!(!ast.styles[0].scoped);
  assert_eq!(ast.styles[0].content, ".title {\n    color: red;\n  }");

  // template fragments are preserved
  assert!(!ast.template.is_empty());
}

#[test]
fn panic_on_empty_trs_file() {
  let input = "";
  let result = thebe_ast::parse(input);
  assert!(result.is_err());
}

#[test]
fn should_panic_on_duplicate_script_setup() {
  let input = fs::read_to_string("tests/samples/duplicate_script_setup.trs").unwrap();
  let result = thebe_ast::parse(&input);
  assert!(matches!(
    result,
    Err(ParseError::DuplicateScriptSetup { .. })
  ));
  let span = result.unwrap_err().span().unwrap();
  assert!(span.start > 0, "span should point to the second occurrence");
}

#[test]
fn should_panic_on_duplicate_script() {
  let input = fs::read_to_string("tests/samples/duplicate_script.trs").unwrap();
  let result = thebe_ast::parse(&input);
  assert!(matches!(result, Err(ParseError::DuplicateScript { .. })));
  let span = result.unwrap_err().span().unwrap();
  assert!(span.start > 0, "span should point to the second occurrence");
}

#[test]
fn should_panic_on_malformed_tags() {
  let input = fs::read_to_string("tests/samples/malformed_tags.trs").unwrap();
  let result = thebe_ast::parse(&input);
  assert!(matches!(result, Err(ParseError::MalformedTag { .. })));
  let span = result.unwrap_err().span().unwrap();
  assert_eq!(span.start, 0, "malformed tag starts at the beginning");
}

#[test]
fn non_contiguous_template_preserved_as_fragments() {
  let input = fs::read_to_string("tests/samples/non_contiguous.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  assert_eq!(ast.template.len(), 3);
  assert_eq!(raw(&ast.template[0]), "<p>First</p>");
  assert_eq!(raw(&ast.template[1]), "<p>Second</p>");
  assert_eq!(raw(&ast.template[2]), "<p>Third</p>");
}

#[test]
fn whitespace_only_file_is_error() {
  let input = "   \n\n  \t  ";
  let result = thebe_ast::parse(input);
  assert!(result.is_err());
}

#[test]
fn template_only_file_is_valid() {
  let input = "<div>Hello World</div>";
  let ast = thebe_ast::parse(input).unwrap();

  assert!(ast.script_setup.is_none());
  assert!(ast.script.is_none());
  assert!(ast.styles.is_empty());
  assert_eq!(ast.template.len(), 1);
  assert_eq!(raw(&ast.template[0]), "<div>Hello World</div>");
}

// --- New rules ---

#[test]
fn multiple_style_blocks_allowed() {
  let input = fs::read_to_string("tests/samples/multiple_styles.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  assert_eq!(ast.styles.len(), 2);

  assert!(!ast.styles[0].scoped);
  assert_eq!(ast.styles[0].content, "body { margin: 0; }");

  assert!(ast.styles[1].scoped);
  assert_eq!(ast.styles[1].content, ".local { color: blue; }");
}

#[test]
fn script_with_lang_attribute() {
  let input = fs::read_to_string("tests/samples/script_lang.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  // script setup has no lang (always Rust)
  let setup = ast.script_setup.as_ref().unwrap();
  assert_eq!(setup.content, "let x: i32 = 42;");
  assert_eq!(setup.lang, None);

  // script block has lang
  let script = ast.script.as_ref().unwrap();
  assert_eq!(script.content, "export default {};");
  assert_eq!(script.lang, Some("ts".to_string()));
}

#[test]
fn script_setup_with_lang_is_error() {
  let input = r#"<script setup lang="ts">
const x = 1;
</script>"#;
  let result = thebe_ast::parse(input);
  assert!(matches!(result, Err(ParseError::InvalidSetupLang { .. })));
}

#[test]
fn style_with_lang_attribute() {
  let input = fs::read_to_string("tests/samples/style_lang.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  assert_eq!(ast.styles.len(), 2);

  assert!(!ast.styles[0].scoped);
  assert_eq!(ast.styles[0].lang, Some("scss".to_string()));
  assert_eq!(
    ast.styles[0].content,
    "$color: red;\n.title { color: $color; }"
  );

  assert!(ast.styles[1].scoped);
  assert_eq!(ast.styles[1].lang, Some("less".to_string()));
}

#[test]
fn style_without_lang_has_none() {
  let input = fs::read_to_string("tests/samples/style_no_lang.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  assert_eq!(ast.styles.len(), 1);
  assert_eq!(ast.styles[0].lang, None);
}

#[test]
fn script_in_template_is_error() {
  let input = fs::read_to_string("tests/samples/nested_script.trs").unwrap();
  let result = thebe_ast::parse(&input);
  assert!(matches!(result, Err(ParseError::NestedScript { .. })));
}

// --- Span tests ---

#[test]
fn script_setup_span_covers_full_block() {
  let input = fs::read_to_string("tests/samples/basic.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  let setup = ast.script_setup.as_ref().unwrap();
  let spanned = &input[setup.span.start..setup.span.end];
  assert!(spanned.starts_with("<script setup>"));
  assert!(spanned.ends_with("</script>"));
}

#[test]
fn style_span_covers_full_block() {
  let input = fs::read_to_string("tests/samples/basic.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  let style = &ast.styles[0];
  let spanned = &input[style.span.start..style.span.end];
  assert!(spanned.starts_with("<style>"));
  assert!(spanned.ends_with("</style>"));
}

#[test]
fn span_line_col_is_correct() {
  let input = "<script setup>\nlet x = 1;\n</script>\n";
  let ast = thebe_ast::parse(input).unwrap();
  let setup = ast.script_setup.as_ref().unwrap();
  let (line, col) = setup.span.line_col(input);
  assert_eq!(line, 1);
  assert_eq!(col, 1);
}

#[test]
fn script_without_lang_is_error() {
  let input = "<script>\nconsole.log('hi');\n</script>";
  let result = thebe_ast::parse(input);
  assert!(matches!(result, Err(ParseError::MissingScriptLang { .. })));
}

#[test]
fn parse_axum_full_example() {
  let input = fs::read_to_string("tests/samples/futures/axum.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  // script setup is Rust, no lang
  let setup = ast.script_setup.as_ref().unwrap();
  assert!(setup.content.contains("use axum::extract::State"));
  assert!(setup.content.contains("#[thebe::data]"));
  assert!(setup.content.contains("pub async fn handler"));
  assert_eq!(setup.lang, None);

  // client script has lang
  let script = ast.script.as_ref().unwrap();
  assert!(script.content.contains("getProps()"));
  assert!(script.content.contains("onMount"));

  // style block
  assert_eq!(ast.styles.len(), 1);
  assert!(ast.styles[0].content.contains(".profile"));
  assert!(ast.styles[0].content.contains(".profile-picture"));

  // template
  assert!(!ast.template.is_empty());
  assert!(
    ast
      .template
      .iter()
      .any(|t| raw(t).contains("{{ username }}"))
  );
}

#[test]
fn parse_egui_full_example() {
  let input = fs::read_to_string("tests/samples/futures/egui.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  // script setup: state struct + Default impl, no lang
  let setup = ast.script_setup.as_ref().unwrap();
  assert!(setup.content.contains("#[thebe::data]"));
  assert!(setup.content.contains("struct Counter"));
  assert!(setup.content.contains("impl Default for Counter"));
  assert_eq!(setup.lang, None);

  // script: reactivity handlers, lang = rs
  let script = ast.script.as_ref().unwrap();
  assert_eq!(script.lang.as_deref(), Some("rs"));
  assert!(script.content.contains("fn increment"));
  assert!(script.content.contains("fn decrement"));
  assert!(script.content.contains("fn reset"));
  assert!(script.content.contains("data: &mut Counter"));

  // style block
  assert_eq!(ast.styles.len(), 1);
  assert!(ast.styles[0].content.contains("font-size: 24"));

  // template uses egui-like elements
  assert!(!ast.template.is_empty());
  assert!(ast.template.iter().any(|t| raw(t).contains("<vstack>")));
  assert!(
    ast
      .template
      .iter()
      .any(|t| raw(t).contains("<button on:click=\"increment\">"))
  );
  assert!(ast.template.iter().any(|t| raw(t).contains("{{ count }}")));
}

#[test]
fn parse_bevy_full_example() {
  let input = fs::read_to_string("tests/samples/futures/bevy.trs").unwrap();
  let ast = thebe_ast::parse(&input).unwrap();

  // script setup: state struct + Default impl, no lang
  let setup = ast.script_setup.as_ref().unwrap();
  assert!(setup.content.contains("#[thebe::data]"));
  assert!(setup.content.contains("struct Counter"));
  assert!(setup.content.contains("impl Default for Counter"));
  assert_eq!(setup.lang, None);

  // script: reactivity handlers, lang = rs
  let script = ast.script.as_ref().unwrap();
  assert_eq!(script.lang.as_deref(), Some("rs"));
  assert!(script.content.contains("fn increment"));
  assert!(script.content.contains("fn decrement"));
  assert!(script.content.contains("fn reset"));
  assert!(script.content.contains("data: &mut Counter"));

  // style block
  assert_eq!(ast.styles.len(), 1);
  assert!(ast.styles[0].content.contains("font-size: 24"));

  // template uses standard HTML-like elements (Bevy UI node tree)
  assert!(!ast.template.is_empty());
  assert!(ast.template.iter().any(|t| raw(t).contains("<div>")));
  assert!(
    ast
      .template
      .iter()
      .any(|t| raw(t).contains("<button on:click=\"increment\">"))
  );
  assert!(ast.template.iter().any(|t| raw(t).contains("{{ count }}")));
  assert!(ast.template.iter().any(|t| raw(t).contains("{{ label }}")));
}

// --- Template interpolation tests ---

#[test]
fn interpolation_plain_text_only() {
  let nodes = thebe_ast::parse_template("<p>Hello</p>", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  assert!(matches!(&nodes[0], TemplateNode::Text(t) if t == "<p>Hello</p>"));
}

#[test]
fn interpolation_single_expr() {
  let nodes = thebe_ast::parse_template("Hello {{ name }}!", 0).unwrap();
  assert_eq!(nodes.len(), 3);
  assert!(matches!(&nodes[0], TemplateNode::Text(t) if t == "Hello "));
  assert!(matches!(&nodes[1], TemplateNode::Expr { expr, .. } if expr == "name"));
  assert!(matches!(&nodes[2], TemplateNode::Text(t) if t == "!"));
}

#[test]
fn interpolation_multiple_exprs() {
  let nodes = thebe_ast::parse_template("{{ a }} and {{ b }}", 0).unwrap();
  assert_eq!(nodes.len(), 3);
  assert!(matches!(&nodes[0], TemplateNode::Expr { expr, .. } if expr == "a"));
  assert!(matches!(&nodes[1], TemplateNode::Text(t) if t == " and "));
  assert!(matches!(&nodes[2], TemplateNode::Expr { expr, .. } if expr == "b"));
}

#[test]
fn interpolation_trims_whitespace() {
  let nodes = thebe_ast::parse_template("{{   spaced   }}", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  assert!(matches!(&nodes[0], TemplateNode::Expr { expr, .. } if expr == "spaced"));
}

#[test]
fn interpolation_unclosed_is_error() {
  let result = thebe_ast::parse_template("Hello {{ name", 0);
  assert!(result.is_err());
  // Via parse() too:
  let result = thebe_ast::parse("<div>{{ oops</div>");
  assert!(matches!(
    result,
    Err(ParseError::UnclosedInterpolation { .. })
  ));
}

#[test]
fn interpolation_span_byte_offsets() {
  let fragment = "Hi {{ x }}!";
  let nodes = thebe_ast::parse_template(fragment, 100).unwrap();
  // The expr node should span the `{{ x }}` portion.
  let expr_node = &nodes[1];
  match expr_node {
    TemplateNode::Expr { span, expr } => {
      assert_eq!(expr, "x");
      // "Hi " is 3 bytes, so `{{` starts at offset 3 within the fragment.
      assert_eq!(span.start, 103); // base_offset(100) + 3
      assert_eq!(span.end, 110); // base_offset(100) + 10
    }
    TemplateNode::Text(_) => panic!("expected Expr node"),
  }
}

#[test]
fn interpolation_empty_expr() {
  // Empty `{{ }}` is allowed — the compiler can reject it later.
  let nodes = thebe_ast::parse_template("{{ }}", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  assert!(matches!(&nodes[0], TemplateNode::Expr { expr, .. } if expr.is_empty()));
}

#[test]
fn interpolation_adjacent_exprs() {
  let nodes = thebe_ast::parse_template("{{ a }}{{ b }}", 0).unwrap();
  assert_eq!(nodes.len(), 2);
  assert!(matches!(&nodes[0], TemplateNode::Expr { expr, .. } if expr == "a"));
  assert!(matches!(&nodes[1], TemplateNode::Expr { expr, .. } if expr == "b"));
}

#[test]
fn interpolation_in_html_attributes() {
  let frag = r#"<img src="{{ url }}" alt="{{ alt }}">"#;
  let nodes = thebe_ast::parse_template(frag, 0).unwrap();
  assert_eq!(nodes.len(), 5);
  assert!(matches!(&nodes[0], TemplateNode::Text(t) if t == r#"<img src=""#));
  assert!(matches!(&nodes[1], TemplateNode::Expr { expr, .. } if expr == "url"));
  assert!(matches!(&nodes[2], TemplateNode::Text(t) if t == r#"" alt=""#));
  assert!(matches!(&nodes[3], TemplateNode::Expr { expr, .. } if expr == "alt"));
  assert!(matches!(&nodes[4], TemplateNode::Text(t) if t == r#"">"#));
}

#[test]
fn full_parse_creates_template_nodes() {
  let input = "<p>Hello {{ name }}!</p>";
  let ast = thebe_ast::parse(input).unwrap();

  assert_eq!(ast.template.len(), 1);
  let frag = &ast.template[0];
  assert_eq!(frag.nodes.len(), 3);
  assert!(matches!(&frag.nodes[0], TemplateNode::Text(t) if t == "<p>Hello "));
  assert!(matches!(&frag.nodes[1], TemplateNode::Expr { expr, .. } if expr == "name"));
  assert!(matches!(&frag.nodes[2], TemplateNode::Text(t) if t == "!</p>"));
}

// --- HTML tree parser tests ---

#[test]
fn html_simple_element() {
  let nodes = thebe_ast::parse_html("<div>hello</div>", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "div");
  assert!(!el.self_closing);
  assert_eq!(el.children.len(), 1);
  assert!(matches!(&el.children[0], HtmlNode::Text(t) if t == "hello"));
}

#[test]
fn html_nested_elements() {
  let nodes = thebe_ast::parse_html("<div><p>text</p></div>", 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected div")
  };
  assert_eq!(div.tag, "div");
  assert_eq!(div.children.len(), 1);
  let HtmlNode::Element(p) = &div.children[0] else {
    panic!("expected p")
  };
  assert_eq!(p.tag, "p");
  assert_eq!(p.children.len(), 1);
  assert!(matches!(&p.children[0], HtmlNode::Text(t) if t == "text"));
}

#[test]
fn html_attribute_static() {
  let nodes = thebe_ast::parse_html(r#"<div class="foo">x</div>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.attributes.len(), 1);
  assert_eq!(el.attributes[0].name, "class");
  assert_eq!(el.attributes[0].value.len(), 1);
  assert!(matches!(&el.attributes[0].value[0], TemplateNode::Text(t) if t == "foo"));
}

#[test]
fn html_attribute_with_interpolation() {
  let nodes = thebe_ast::parse_html(r#"<img src="{{ url }}" />"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert!(el.self_closing);
  assert_eq!(el.attributes.len(), 1);
  assert_eq!(el.attributes[0].name, "src");
  assert!(matches!(&el.attributes[0].value[0], TemplateNode::Expr { expr, .. } if expr == "url"));
}

#[test]
fn html_attribute_mixed_interpolation() {
  let nodes = thebe_ast::parse_html(r#"<p alt="Hello {{ name }}!">x</p>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  let val = &el.attributes[0].value;
  assert_eq!(val.len(), 3);
  assert!(matches!(&val[0], TemplateNode::Text(t) if t == "Hello "));
  assert!(matches!(&val[1], TemplateNode::Expr { expr, .. } if expr == "name"));
  assert!(matches!(&val[2], TemplateNode::Text(t) if t == "!"));
}

#[test]
fn html_event_directive() {
  let nodes = thebe_ast::parse_html(r#"<button on:click="increment">+</button>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "button");
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::On);
  assert_eq!(el.directives[0].name, "click");
  assert_eq!(el.directives[0].value, "increment");
  // No attributes (directive is separate)
  assert!(el.attributes.is_empty());
}

#[test]
fn html_self_closing_explicit() {
  let nodes = thebe_ast::parse_html("<br />", 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "br");
  assert!(el.self_closing);
  assert!(el.children.is_empty());
}

#[test]
fn html_void_element_without_slash() {
  let nodes = thebe_ast::parse_html(r#"<img src="pic.png">"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "img");
  assert!(el.self_closing);
  assert!(el.children.is_empty());
}

#[test]
fn html_interpolation_in_text() {
  let nodes = thebe_ast::parse_html("<p>Hello {{ name }}!</p>", 0).unwrap();
  let HtmlNode::Element(p) = &nodes[0] else {
    panic!("expected p")
  };
  assert_eq!(p.children.len(), 3);
  assert!(matches!(&p.children[0], HtmlNode::Text(t) if t == "Hello "));
  assert!(matches!(&p.children[1], HtmlNode::Expr { expr, .. } if expr == "name"));
  assert!(matches!(&p.children[2], HtmlNode::Text(t) if t == "!"));
}

#[test]
fn html_multiple_children() {
  let html = r"<div><p>one</p><p>two</p></div>";
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected div")
  };
  assert_eq!(div.children.len(), 2);
  let HtmlNode::Element(p1) = &div.children[0] else {
    panic!("expected p")
  };
  let HtmlNode::Element(p2) = &div.children[1] else {
    panic!("expected p")
  };
  assert!(matches!(&p1.children[0], HtmlNode::Text(t) if t == "one"));
  assert!(matches!(&p2.children[0], HtmlNode::Text(t) if t == "two"));
}

#[test]
fn html_custom_elements() {
  let nodes = thebe_ast::parse_html("<vstack><label>hi</label></vstack>", 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "vstack");
  let HtmlNode::Element(label) = &el.children[0] else {
    panic!("expected label")
  };
  assert_eq!(label.tag, "label");
}

#[test]
fn html_multiple_attributes() {
  let html = r#"<img src="{{ url }}" alt="{{ alt }}" class="pic" />"#;
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.attributes.len(), 3);
  assert_eq!(el.attributes[0].name, "src");
  assert_eq!(el.attributes[1].name, "alt");
  assert_eq!(el.attributes[2].name, "class");
}

#[test]
fn html_span_covers_full_element() {
  let frag = "  <div>hello</div>  ";
  // parse_html on the trimmed portion
  let trimmed = frag.trim();
  let nodes = thebe_ast::parse_html(trimmed, 2).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.span.start, 2);
  assert_eq!(el.span.end, 18); // 2 + len("<div>hello</div>") = 2 + 16 = 18
}

#[test]
fn html_unclosed_interpolation_in_text() {
  let result = thebe_ast::parse_html("<p>{{ oops</p>", 0);
  assert!(matches!(
    result,
    Err(ParseError::UnclosedInterpolation { .. })
  ));
}

#[test]
fn html_unclosed_tag() {
  let result = thebe_ast::parse_html("<div>no close", 0);
  assert!(matches!(result, Err(ParseError::MalformedTag { .. })));
}

#[test]
fn html_axum_template() {
  let html = r#"<div class="profile">
    <img src="{{ profile_picture }}" alt="Profile Picture {{ username }}" class="profile-picture">
    <p>Hello, {{ username }}!</p>
</div>"#;
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected div")
  };
  assert_eq!(div.tag, "div");

  // Find the img element (skip whitespace text nodes)
  let img = div
    .children
    .iter()
    .find_map(|n| {
      if let HtmlNode::Element(e) = n
        && e.tag == "img"
      {
        return Some(e);
      }
      None
    })
    .expect("should have img");
  assert!(img.self_closing);
  assert_eq!(img.attributes.len(), 3); // src, alt, class

  // Find the p element
  let p = div
    .children
    .iter()
    .find_map(|n| {
      if let HtmlNode::Element(e) = n
        && e.tag == "p"
      {
        return Some(e);
      }
      None
    })
    .expect("should have p");
  assert_eq!(p.children.len(), 3); // "Hello, " + {{ username }} + "!"
}

#[test]
fn html_egui_template() {
  let html = r#"<vstack>
    <label>{{ label }}</label>
    <label>Count: {{ count }}</label>
    <hstack>
        <button on:click="decrement">-</button>
        <button on:click="reset">Reset</button>
        <button on:click="increment">+</button>
    </hstack>
</vstack>"#;
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Element(vstack) = &nodes[0] else {
    panic!("expected vstack")
  };
  assert_eq!(vstack.tag, "vstack");

  // Find hstack
  let hstack = vstack
    .children
    .iter()
    .find_map(|n| {
      if let HtmlNode::Element(e) = n
        && e.tag == "hstack"
      {
        return Some(e);
      }
      None
    })
    .expect("should have hstack");

  // Find buttons with directives
  let buttons: Vec<&Element> = hstack
    .children
    .iter()
    .filter_map(|n| {
      if let HtmlNode::Element(e) = n
        && e.tag == "button"
      {
        return Some(e);
      }
      None
    })
    .collect();
  assert_eq!(buttons.len(), 3);
  assert_eq!(buttons[0].directives[0].value, "decrement");
  assert_eq!(buttons[1].directives[0].value, "reset");
  assert_eq!(buttons[2].directives[0].value, "increment");
}

// --- Control-flow block tests ---

#[test]
fn html_if_basic() {
  let nodes = thebe_ast::parse_html("{#if show}<p>visible</p>{/if}", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(branches.len(), 1);
  assert_eq!(branches[0].condition.as_deref(), Some("show"));
  assert_eq!(branches[0].children.len(), 1);
  let HtmlNode::Element(p) = &branches[0].children[0] else {
    panic!("expected p")
  };
  assert_eq!(p.tag, "p");
}

#[test]
fn html_if_else() {
  let nodes =
    thebe_ast::parse_html("{#if logged_in}<p>Welcome</p>{:else}<p>Log in</p>{/if}", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(branches.len(), 2);
  assert_eq!(branches[0].condition.as_deref(), Some("logged_in"));
  assert_eq!(branches[0].children.len(), 1);
  assert!(branches[1].condition.is_none());
  assert_eq!(branches[1].children.len(), 1);
}

#[test]
fn html_if_else_if_else() {
  let html = "{#if x > 0}<p>positive</p>{:else if x < 0}<p>negative</p>{:else}<p>zero</p>{/if}";
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(branches.len(), 3);
  assert_eq!(branches[0].condition.as_deref(), Some("x > 0"));
  assert_eq!(branches[1].condition.as_deref(), Some("x < 0"));
  assert!(branches[2].condition.is_none());
}

#[test]
fn html_each_basic() {
  let nodes = thebe_ast::parse_html("{#each items as item}<li>{{ item }}</li>{/each}", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::Each {
    iterable,
    binding,
    index,
    children,
    ..
  } = &nodes[0]
  else {
    panic!("expected Each")
  };
  assert_eq!(iterable, "items");
  assert_eq!(binding, "item");
  assert!(index.is_none());
  assert_eq!(children.len(), 1);
  let HtmlNode::Element(li) = &children[0] else {
    panic!("expected li")
  };
  assert_eq!(li.tag, "li");
}

#[test]
fn html_each_with_index() {
  let nodes = thebe_ast::parse_html("{#each items as item, i}<li>{{ i }}</li>{/each}", 0).unwrap();
  let HtmlNode::Each {
    iterable,
    binding,
    index,
    ..
  } = &nodes[0]
  else {
    panic!("expected Each")
  };
  assert_eq!(iterable, "items");
  assert_eq!(binding, "item");
  assert_eq!(index.as_deref(), Some("i"));
}

#[test]
fn html_nested_if_in_each() {
  let html = "{#each items as item}{#if item.visible}<p>{{ item.name }}</p>{/if}{/each}";
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Each { children, .. } = &nodes[0] else {
    panic!("expected Each")
  };
  assert_eq!(children.len(), 1);
  assert!(matches!(&children[0], HtmlNode::If { .. }));
}

#[test]
fn html_if_with_text_children() {
  let nodes = thebe_ast::parse_html("{#if greeting}Hello!{/if}", 0).unwrap();
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(branches[0].children.len(), 1);
  assert!(matches!(&branches[0].children[0], HtmlNode::Text(t) if t == "Hello!"));
}

#[test]
fn html_if_with_interpolation() {
  let nodes = thebe_ast::parse_html("{#if user}Hello {{ name }}!{/if}", 0).unwrap();
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(branches[0].children.len(), 3);
  assert!(matches!(&branches[0].children[0], HtmlNode::Text(t) if t == "Hello "));
  assert!(matches!(&branches[0].children[1], HtmlNode::Expr { expr, .. } if expr == "name"));
  assert!(matches!(&branches[0].children[2], HtmlNode::Text(t) if t == "!"));
}

#[test]
fn html_unclosed_if_is_error() {
  let result = thebe_ast::parse_html("{#if show}<p>oops</p>", 0);
  assert!(matches!(result, Err(ParseError::UnclosedIfBlock { .. })));
}

#[test]
fn html_unclosed_each_is_error() {
  let result = thebe_ast::parse_html("{#each items as item}<li>x</li>", 0);
  assert!(matches!(result, Err(ParseError::UnclosedEachBlock { .. })));
}

#[test]
fn html_if_span_covers_full_block() {
  let frag = "{#if show}<p>hi</p>{/if}";
  let nodes = thebe_ast::parse_html(frag, 0).unwrap();
  let HtmlNode::If { span, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert_eq!(span.start, 0);
  assert_eq!(span.end, frag.len());
}

#[test]
fn html_each_span_covers_full_block() {
  let frag = "{#each xs as x}<li>{{ x }}</li>{/each}";
  let nodes = thebe_ast::parse_html(frag, 0).unwrap();
  let HtmlNode::Each { span, .. } = &nodes[0] else {
    panic!("expected Each")
  };
  assert_eq!(span.start, 0);
  assert_eq!(span.end, frag.len());
}

#[test]
fn html_if_inside_element() {
  let nodes = thebe_ast::parse_html("<div>{#if x}<p>yes</p>{/if}</div>", 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected div")
  };
  assert_eq!(div.children.len(), 1);
  assert!(matches!(&div.children[0], HtmlNode::If { .. }));
}

#[test]
fn html_each_inside_element() {
  let html = "<ul>{#each items as item}<li>{{ item }}</li>{/each}</ul>";
  let nodes = thebe_ast::parse_html(html, 0).unwrap();
  let HtmlNode::Element(ul) = &nodes[0] else {
    panic!("expected ul")
  };
  assert_eq!(ul.children.len(), 1);
  let HtmlNode::Each { children, .. } = &ul.children[0] else {
    panic!("expected Each")
  };
  assert_eq!(children.len(), 1);
}

// ---------------------------------------------------------------------------
// Directive kinds: bind:, class:, style:, use:
// ---------------------------------------------------------------------------

#[test]
fn html_bind_directive_with_value() {
  let nodes = thebe_ast::parse_html(r#"<input bind:value="name" />"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.tag, "input");
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Bind);
  assert_eq!(el.directives[0].name, "value");
  assert_eq!(el.directives[0].value, "name");
}

#[test]
fn html_bind_directive_shorthand() {
  // bind:value with no ="..." → value is empty (shorthand for same-named binding)
  let nodes = thebe_ast::parse_html(r"<input bind:value />", 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Bind);
  assert_eq!(el.directives[0].name, "value");
  assert_eq!(el.directives[0].value, "");
}

#[test]
fn html_class_directive() {
  let nodes = thebe_ast::parse_html(r#"<div class:active="is_active">hi</div>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Class);
  assert_eq!(el.directives[0].name, "active");
  assert_eq!(el.directives[0].value, "is_active");
}

#[test]
fn html_class_directive_shorthand() {
  // class:active with no value → condition is the class name itself
  let nodes = thebe_ast::parse_html(r"<div class:active>hi</div>", 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Class);
  assert_eq!(el.directives[0].name, "active");
  assert_eq!(el.directives[0].value, "");
}

#[test]
fn html_style_directive() {
  let nodes = thebe_ast::parse_html(r#"<p style:color="highlight_color">text</p>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Style);
  assert_eq!(el.directives[0].name, "color");
  assert_eq!(el.directives[0].value, "highlight_color");
}

#[test]
fn html_use_directive() {
  let nodes = thebe_ast::parse_html(r#"<div use:tooltip="opts">hover me</div>"#, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Use);
  assert_eq!(el.directives[0].name, "tooltip");
  assert_eq!(el.directives[0].value, "opts");
}

#[test]
fn html_use_directive_no_value() {
  let nodes = thebe_ast::parse_html(r"<div use:tooltip>hover me</div>", 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 1);
  assert_eq!(el.directives[0].kind, DirectiveKind::Use);
  assert_eq!(el.directives[0].name, "tooltip");
  assert_eq!(el.directives[0].value, "");
}

#[test]
fn html_multiple_directives_mixed() {
  let nodes = thebe_ast::parse_html(
    r#"<input bind:value="name" on:input="handle_input" class:error="has_error" />"#,
    0,
  )
  .unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  assert_eq!(el.directives.len(), 3);

  assert_eq!(el.directives[0].kind, DirectiveKind::Bind);
  assert_eq!(el.directives[0].name, "value");

  assert_eq!(el.directives[1].kind, DirectiveKind::On);
  assert_eq!(el.directives[1].name, "input");

  assert_eq!(el.directives[2].kind, DirectiveKind::Class);
  assert_eq!(el.directives[2].name, "error");
}

#[test]
fn html_directive_span_covers_full_attr() {
  let src = r#"<div on:click="go" bind:value="x">ok</div>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected element")
  };
  // on:click="go"
  let d0 = &el.directives[0];
  assert_eq!(&src[d0.span.start..d0.span.end], r#"on:click="go""#);
  // bind:value="x"
  let d1 = &el.directives[1];
  assert_eq!(&src[d1.span.start..d1.span.end], r#"bind:value="x""#);
}

// ---------------------------------------------------------------------------
// Component references (capital-letter tags → HtmlNode::Component)
// ---------------------------------------------------------------------------

#[test]
fn component_self_closing() {
  let nodes = thebe_ast::parse_html("<MyButton />", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component, got {:?}", nodes[0])
  };
  assert_eq!(el.tag, "MyButton");
  assert!(el.self_closing);
  assert!(el.children.is_empty());
}

#[test]
fn component_with_children() {
  let nodes = thebe_ast::parse_html("<Card><p>hello</p></Card>", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(el.tag, "Card");
  assert!(!el.self_closing);
  assert_eq!(el.children.len(), 1);
  let HtmlNode::Element(p) = &el.children[0] else {
    panic!("expected Element child")
  };
  assert_eq!(p.tag, "p");
}

#[test]
fn component_with_props() {
  let src = r#"<Button label="click me" disabled="true" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(el.tag, "Button");
  assert_eq!(el.attributes.len(), 2);
  assert_eq!(el.attributes[0].name, "label");
  assert_eq!(el.attributes[1].name, "disabled");
}

#[test]
fn component_with_directives() {
  let src = r#"<Form on:submit="handleSubmit" bind:value="name">ok</Form>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(el.tag, "Form");
  assert_eq!(el.directives.len(), 2);
  assert_eq!(el.directives[0].kind, DirectiveKind::On);
  assert_eq!(el.directives[0].name, "submit");
  assert_eq!(el.directives[1].kind, DirectiveKind::Bind);
  assert_eq!(el.directives[1].name, "value");
}

#[test]
fn component_nested_in_element() {
  let src = "<div><MyWidget /></div>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected Element")
  };
  assert_eq!(div.tag, "div");
  let HtmlNode::Component(w) = &div.children[0] else {
    panic!("expected Component child")
  };
  assert_eq!(w.tag, "MyWidget");
}

#[test]
fn element_nested_in_component() {
  let src = "<Layout><main>content</main></Layout>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(layout) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(layout.tag, "Layout");
  let HtmlNode::Element(main) = &layout.children[0] else {
    panic!("expected Element child")
  };
  assert_eq!(main.tag, "main");
}

#[test]
fn component_with_interpolated_prop() {
  let src = r#"<Avatar src="{{ user.avatar }}" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(el.tag, "Avatar");
  assert_eq!(el.attributes.len(), 1);
  assert_eq!(el.attributes[0].name, "src");
  // value should contain an Expr node
  assert!(el.attributes[0].value.iter().any(|n| matches!(n, TemplateNode::Expr { .. })));
}

#[test]
fn component_span_covers_full_element() {
  let src = "<Dialog>body</Dialog>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(&src[el.span.start..el.span.end], src);
}

#[test]
fn component_inside_if_block() {
  let src = r"{#if show}<Modal />{/if}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  let HtmlNode::Component(modal) = &branches[0].children[0] else {
    panic!("expected Component inside if branch")
  };
  assert_eq!(modal.tag, "Modal");
}

#[test]
fn component_inside_each_block() {
  let src = r"{#each items as item}<ListItem />{/each}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Each { children, .. } = &nodes[0] else {
    panic!("expected Each")
  };
  let HtmlNode::Component(li) = &children[0] else {
    panic!("expected Component inside each")
  };
  assert_eq!(li.tag, "ListItem");
}

#[test]
fn lowercase_tag_is_element_not_component() {
  let nodes = thebe_ast::parse_html("<div>text</div>", 0).unwrap();
  assert!(matches!(&nodes[0], HtmlNode::Element(_)));
}

#[test]
fn full_parse_with_component_in_template() {
  let src = r#"<script setup>let x = 1;</script>
<Header title="hello" />
<main>content</main>"#;
  let ast = thebe_ast::parse(src).unwrap();
  assert!(ast.script_setup.is_some());
  // template should contain the component and main element
  let tpl = &ast.template;
  assert!(!tpl.is_empty());
}

// ---------------------------------------------------------------------------
// Slot support (<slot> and <slot name="...">)
// ---------------------------------------------------------------------------

#[test]
fn default_slot() {
  let nodes = thebe_ast::parse_html("<slot />", 0).unwrap();
  assert_eq!(nodes.len(), 1);
  let HtmlNode::Slot {
    name, children, ..
  } = &nodes[0]
  else {
    panic!("expected Slot, got {:?}", nodes[0])
  };
  assert_eq!(*name, None);
  assert!(children.is_empty());
}

#[test]
fn named_slot() {
  let nodes = thebe_ast::parse_html(r#"<slot name="header" />"#, 0).unwrap();
  let HtmlNode::Slot { name, .. } = &nodes[0] else {
    panic!("expected Slot")
  };
  assert_eq!(name.as_deref(), Some("header"));
}

#[test]
fn slot_with_fallback_content() {
  let nodes = thebe_ast::parse_html("<slot>default text</slot>", 0).unwrap();
  let HtmlNode::Slot { children, name, .. } = &nodes[0] else {
    panic!("expected Slot")
  };
  assert_eq!(*name, None);
  assert_eq!(children.len(), 1);
  assert!(matches!(&children[0], HtmlNode::Text(t) if t == "default text"));
}

#[test]
fn named_slot_with_fallback() {
  let src = r#"<slot name="footer"><p>fallback</p></slot>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Slot { name, children, .. } = &nodes[0] else {
    panic!("expected Slot")
  };
  assert_eq!(name.as_deref(), Some("footer"));
  assert_eq!(children.len(), 1);
  let HtmlNode::Element(p) = &children[0] else {
    panic!("expected Element child")
  };
  assert_eq!(p.tag, "p");
}

#[test]
fn slot_inside_component() {
  let src = "<Card><slot /></Card>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(card) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(card.tag, "Card");
  assert_eq!(card.children.len(), 1);
  assert!(matches!(&card.children[0], HtmlNode::Slot { .. }));
}

#[test]
fn slot_inside_element() {
  let src = "<div><slot name=\"sidebar\" /></div>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(div) = &nodes[0] else {
    panic!("expected Element")
  };
  let HtmlNode::Slot { name, .. } = &div.children[0] else {
    panic!("expected Slot child")
  };
  assert_eq!(name.as_deref(), Some("sidebar"));
}

#[test]
fn slot_inside_if_block() {
  let src = r"{#if show}<slot />{/if}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::If { branches, .. } = &nodes[0] else {
    panic!("expected If")
  };
  assert!(matches!(&branches[0].children[0], HtmlNode::Slot { .. }));
}

#[test]
fn multiple_named_slots() {
  let src = r#"<Layout><slot name="header" /><slot /><slot name="footer" /></Layout>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(layout) = &nodes[0] else {
    panic!("expected Component")
  };
  assert_eq!(layout.children.len(), 3);

  let HtmlNode::Slot { name: n0, .. } = &layout.children[0] else {
    panic!("expected Slot")
  };
  assert_eq!(n0.as_deref(), Some("header"));

  let HtmlNode::Slot { name: n1, .. } = &layout.children[1] else {
    panic!("expected Slot")
  };
  assert_eq!(*n1, None);

  let HtmlNode::Slot { name: n2, .. } = &layout.children[2] else {
    panic!("expected Slot")
  };
  assert_eq!(n2.as_deref(), Some("footer"));
}

#[test]
fn slot_span_covers_full_tag() {
  let src = "<slot />";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Slot { span, .. } = &nodes[0] else {
    panic!("expected Slot")
  };
  assert_eq!(&src[span.start..span.end], src);
}

#[test]
fn slot_with_interpolation_fallback() {
  let src = "<slot>{{ default_value }}</slot>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Slot { children, .. } = &nodes[0] else {
    panic!("expected Slot")
  };
  assert_eq!(children.len(), 1);
  let HtmlNode::Expr { expr, .. } = &children[0] else {
    panic!("expected Expr child")
  };
  assert_eq!(expr, "default_value");
}

// ---------------------------------------------------------------------------
// Event modifiers (on:click|preventDefault|stopPropagation)
// ---------------------------------------------------------------------------

#[test]
fn single_event_modifier() {
  let src = r#"<form on:submit|preventDefault="handle">ok</form>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  assert_eq!(el.directives.len(), 1);
  let d = &el.directives[0];
  assert_eq!(d.kind, DirectiveKind::On);
  assert_eq!(d.name, "submit");
  assert_eq!(d.modifiers, vec!["preventDefault"]);
  assert_eq!(d.value, "handle");
}

#[test]
fn multiple_event_modifiers() {
  let src = r#"<button on:click|stopPropagation|preventDefault="go">x</button>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(d.kind, DirectiveKind::On);
  assert_eq!(d.name, "click");
  assert_eq!(d.modifiers, vec!["stopPropagation", "preventDefault"]);
}

#[test]
fn event_without_modifiers_has_empty_vec() {
  let src = r#"<button on:click="go">x</button>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(d.kind, DirectiveKind::On);
  assert_eq!(d.name, "click");
  assert!(d.modifiers.is_empty());
}

#[test]
fn non_event_directive_has_no_modifiers() {
  let src = r#"<input bind:value="x" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(d.kind, DirectiveKind::Bind);
  assert!(d.modifiers.is_empty());
}

#[test]
fn modifier_on_self_closing_element() {
  let src = r#"<input on:focus|once="handler" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(d.name, "focus");
  assert_eq!(d.modifiers, vec!["once"]);
}

#[test]
fn modifier_with_no_value() {
  let src = "<button on:click|preventDefault>x</button>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(d.name, "click");
  assert_eq!(d.modifiers, vec!["preventDefault"]);
  assert!(d.value.is_empty());
}

#[test]
fn modifier_span_covers_full_directive() {
  let src = r#"<button on:click|prevent="go">x</button>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Element(el) = &nodes[0] else {
    panic!("expected Element")
  };
  let d = &el.directives[0];
  assert_eq!(&src[d.span.start..d.span.end], r#"on:click|prevent="go""#);
}

#[test]
fn modifier_on_component() {
  let src = r#"<Form on:submit|preventDefault="save">ok</Form>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let HtmlNode::Component(el) = &nodes[0] else {
    panic!("expected Component")
  };
  let d = &el.directives[0];
  assert_eq!(d.name, "submit");
  assert_eq!(d.modifiers, vec!["preventDefault"]);
}
