/// Helper: parse → lower → generate in one call.
fn codegen(source: &str) -> String {
  let ast = thebe_ast::parse(source).unwrap();
  let ir = thebe_compiler::lower(source, &ast).unwrap();
  thebe_compiler::generate(&ir, &[])
}

/// Helper: parse → lower → generate with route params.
fn codegen_with_params(source: &str, params: &[&str]) -> String {
  let ast = thebe_ast::parse(source).unwrap();
  let ir = thebe_compiler::lower(source, &ast).unwrap();
  let params: Vec<String> = params.iter().map(|s| (*s).to_string()).collect();
  thebe_compiler::generate(&ir, &params)
}

// ── Simple elements ─────────────────────────────────────────────────────

#[test]
fn simple_element() {
  let code = codegen("<div>hello</div>");
  assert!(code.contains(r#"__html.push_str("<div");"#));
  assert!(code.contains(r#"__html.push_str(">");"#));
  assert!(code.contains(r#"__html.push_str("hello");"#));
  assert!(code.contains(r#"__html.push_str("</div>");"#));
}

#[test]
fn self_closing_element() {
  let code = codegen("<br />");
  assert!(code.contains(r#"__html.push_str("<br");"#));
  assert!(code.contains(r#"__html.push_str(" />");"#));
  assert!(!code.contains("</br>"));
}

#[test]
fn void_element_without_slash() {
  let code = codegen("<img>");
  // Void elements are self-closing in the AST.
  assert!(code.contains(r#"__html.push_str("<img");"#));
  assert!(code.contains(r#"__html.push_str(" />");"#));
  assert!(!code.contains("</img>"));
}

#[test]
fn nested_elements() {
  let code = codegen("<div><p><span>x</span></p></div>");
  assert!(code.contains(r#"__html.push_str("<div");"#));
  assert!(code.contains(r#"__html.push_str("<p");"#));
  assert!(code.contains(r#"__html.push_str("<span");"#));
  assert!(code.contains(r#"__html.push_str("x");"#));
  assert!(code.contains(r#"__html.push_str("</span>");"#));
  assert!(code.contains(r#"__html.push_str("</p>");"#));
  assert!(code.contains(r#"__html.push_str("</div>");"#));
}

// ── Attributes ──────────────────────────────────────────────────────────

#[test]
fn static_attribute() {
  let code = codegen(r#"<div class="foo">x</div>"#);
  assert!(code.contains(r#"__html.push_str(" class=\"foo\"");"#));
}

#[test]
fn boolean_attribute() {
  let code = codegen("<input disabled />");
  assert!(code.contains(r#"__html.push_str(" disabled");"#));
}

#[test]
fn dynamic_attribute() {
  let code = codegen(r#"<a href="{{ url }}">link</a>"#);
  // Dynamic: opens attr, emits escaped expr, closes attr.
  assert!(code.contains(r#"__html.push_str(" href=\"");"#));
  assert!(code.contains(r#"__esc(&format!("{}", { url }))"#));
  assert!(code.contains(r#"__html.push_str("\"");"#));
}

#[test]
fn mixed_static_dynamic_attribute() {
  let code = codegen(r#"<img src="/img/{{ name }}.png" />"#);
  assert!(code.contains(r#"__html.push_str(" src=\"");"#));
  assert!(code.contains(r#"__html.push_str("/img/");"#));
  assert!(code.contains(r#"__esc(&format!("{}", { name }))"#));
  assert!(code.contains(r#"__html.push_str(".png");"#));
}

// ── Interpolation ───────────────────────────────────────────────────────

#[test]
fn text_interpolation() {
  let code = codegen("<p>{{ title }}</p>");
  assert!(code.contains(r#"__esc(&format!("{}", { title }))"#));
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn if_block() {
  let code = codegen("{#if show}<p>yes</p>{/if}");
  assert!(code.contains("if show {"));
  assert!(code.contains(r#"__html.push_str("<p");"#));
  assert!(code.contains(r#"__html.push_str("yes");"#));
}

#[test]
fn if_else_block() {
  let code = codegen("{#if show}<p>yes</p>{:else}<p>no</p>{/if}");
  assert!(code.contains("if show {"));
  assert!(code.contains("} else {"));
  assert!(code.contains(r#"__html.push_str("no");"#));
}

#[test]
fn if_else_if_else_block() {
  let code = codegen("{#if a}<p>a</p>{:else if b}<p>b</p>{:else}<p>c</p>{/if}");
  assert!(code.contains("if a {"));
  assert!(code.contains("} else if b {"));
  assert!(code.contains("} else {"));
}

#[test]
fn each_block() {
  let code = codegen("{#each items as item}<li>{{ item }}</li>{/each}");
  assert!(code.contains("for item in items {"));
  assert!(code.contains(r#"__html.push_str("<li");"#));
}

#[test]
fn each_with_index() {
  let code = codegen("{#each items as item, i}<li>{{ i }}</li>{/each}");
  assert!(code.contains("for (i, item) in (items).into_iter().enumerate() {"));
}

// ── Components ──────────────────────────────────────────────────────────

#[test]
fn component_call() {
  let code = codegen("<Button />");
  assert!(code.contains("__html.push_str(&Button::render());"));
}

// ── Slots ───────────────────────────────────────────────────────────────

#[test]
fn slot_fallback() {
  let code = codegen("<slot>default text</slot>");
  // Slot fallback content is rendered inline.
  assert!(code.contains(r#"__html.push_str("default text");"#));
}

#[test]
fn empty_slot() {
  let code = codegen("<slot />");
  // Self-closing slot with no fallback produces no push_str for children.
  assert!(code.contains("pub fn render()"));
  // No slot-specific output beyond the function structure.
}

// ── Scoped styles ───────────────────────────────────────────────────────

#[test]
fn scoped_style_injects_data_attr() {
  let code = codegen("<style scoped>.a { color: red; }</style>\n<div>x</div>");
  // Elements should have a data-s-XXXXXXXX attribute.
  assert!(code.contains("data-s-"));
  // The attribute appears on the element, not on the text.
  assert!(code.contains(r#"__html.push_str(" data-s-"#));
}

#[test]
fn unscoped_style_no_data_attr() {
  let code = codegen("<style>.a { color: red; }</style>\n<div>x</div>");
  assert!(!code.contains("data-s-"));
}

#[test]
fn scope_id_in_constants() {
  let code = codegen("<style scoped>.a{}</style>\n<p>x</p>");
  assert!(code.contains("SCOPE_IDS"));
  assert!(code.contains("s-"));
}

// ── Setup block ─────────────────────────────────────────────────────────

#[test]
fn setup_block_inlined() {
  let code = codegen("<script setup>let x = 42;</script>\n<p>{{ x }}</p>");
  assert!(code.contains("let x = 42;"));
  assert!(code.contains(r#"__esc(&format!("{}", { x }))"#));
}

// ── Style constants ─────────────────────────────────────────────────────

#[test]
fn styles_constant_populated() {
  let code = codegen("<style>.a { color: red; }</style>\n<div>x</div>");
  assert!(code.contains("pub const STYLES: &[&str]"));
  assert!(code.contains(".a { color: red; }"));
}

#[test]
fn styles_constant_empty_when_no_styles() {
  let code = codegen("<div>x</div>");
  assert!(code.contains("pub const STYLES: &[&str] = &[];"));
}

// ── Escape function ─────────────────────────────────────────────────────

#[test]
fn esc_function_included() {
  let code = codegen("<div>x</div>");
  assert!(code.contains("fn __esc(s: &str) -> String {"));
  assert!(code.contains("&amp;"));
  assert!(code.contains("&lt;"));
  assert!(code.contains("&gt;"));
}

// ── Module structure ────────────────────────────────────────────────────

#[test]
fn module_header() {
  let code = codegen("<div>x</div>");
  assert!(code.starts_with("// Auto-generated by thebe-compiler. Do not edit.\n"));
}

#[test]
fn render_fn_signature() {
  let code = codegen("<div>x</div>");
  assert!(code.contains("pub fn render() -> String {"));
  assert!(code.contains("let mut __html = String::new();"));
  // Function returns __html.
  assert!(code.contains("  __html\n"));
}

// ── Full component ──────────────────────────────────────────────────────

#[test]
fn full_component_smoke() {
  let source = r#"<script setup>
let title = "hello";
let show = true;
</script>

<div>
  <h1>{{ title }}</h1>
  {#if show}
    <p>visible</p>
  {/if}
</div>

<style scoped>
h1 { color: blue; }
</style>"#;

  let code = codegen(source);

  // Setup is inlined.
  assert!(code.contains(r#"let title = "hello";"#));
  assert!(code.contains("let show = true;"));

  // Template renders.
  assert!(code.contains(r#"__html.push_str("<div");"#));
  assert!(code.contains(r#"__html.push_str("<h1"#));
  assert!(code.contains("if show {"));

  // Scoped style data attribute on elements.
  assert!(code.contains("data-s-"));

  // Scoped style constant — selectors are rewritten with scope attribute.
  assert!(code.contains("h1[data-s-"));
  assert!(code.contains("{ color: blue; }"));

  // Escape function present.
  assert!(code.contains("fn __esc("));
}

// ── Route params ────────────────────────────────────────────────────────

#[test]
fn render_with_no_params() {
  let code = codegen("<div>x</div>");
  assert!(code.contains("pub fn render() -> String {"));
  assert!(code.contains("pub fn render_with_slot(__slot: &str) -> String {"));
}

#[test]
fn render_with_single_param() {
  let code = codegen_with_params("<p>{{ slug }}</p>", &["slug"]);
  assert!(code.contains("pub fn render(slug: &str) -> String {"));
  assert!(code.contains("pub fn render_with_slot(__slot: &str, slug: &str) -> String {"));
}

#[test]
fn render_with_multiple_params() {
  let code = codegen_with_params("<p>{{ org }}/{{ repo }}</p>", &["org", "repo"]);
  assert!(code.contains("pub fn render(org: &str, repo: &str) -> String {"));
  assert!(code.contains(
    "pub fn render_with_slot(__slot: &str, org: &str, repo: &str) -> String {"
  ));
}

#[test]
fn param_used_in_expression() {
  let code = codegen_with_params("<h1>{{ slug }}</h1>", &["slug"]);
  assert!(code.contains(r#"__esc(&format!("{}", { slug }))"#));
}

#[test]
fn param_with_setup_block() {
  let source = r#"<script setup>
let upper = slug.to_uppercase();
</script>

<h1>{{ upper }}</h1>"#;
  let code = codegen_with_params(source, &["slug"]);
  assert!(code.contains("pub fn render(slug: &str) -> String {"));
  assert!(code.contains("let upper = slug.to_uppercase();"));
}

// ── Event handlers (on:event) ───────────────────────────────────────────

#[test]
fn event_handler_emitted_as_data_attr() {
  let code = codegen(r#"<button on:click="handleClick">Go</button>"#);
  assert!(code.contains(r#"__html.push_str(" data-on-click=\"handleClick\"");"#));
}

#[test]
fn event_handler_with_modifiers() {
  let code = codegen(r#"<form on:submit|preventDefault="onSubmit">ok</form>"#);
  assert!(code.contains(r#"data-on-submit=\"onSubmit\""#));
  assert!(code.contains(r#"data-on-submit-mod=\"preventDefault\""#));
}

#[test]
fn event_handler_multiple_modifiers() {
  let code = codegen(r#"<button on:click|preventDefault|once="go">Go</button>"#);
  assert!(code.contains(r#"data-on-click=\"go\""#));
  assert!(code.contains(r#"data-on-click-mod=\"preventDefault|once\""#));
}

#[test]
fn multiple_event_handlers() {
  let code = codegen(r#"<div on:click="c" on:mouseover="m">x</div>"#);
  assert!(code.contains(r#"data-on-click=\"c\""#));
  assert!(code.contains(r#"data-on-mouseover=\"m\""#));
}

// ── Bindings (bind:prop) ────────────────────────────────────────────────

#[test]
fn binding_emitted_as_data_attr() {
  let code = codegen(r#"<input bind:value="name" />"#);
  assert!(code.contains(r#"data-bind-value=\"name\""#));
}

#[test]
fn multiple_bindings() {
  let code = codegen(r#"<input bind:value="text" bind:checked="on" />"#);
  assert!(code.contains(r#"data-bind-value=\"text\""#));
  assert!(code.contains(r#"data-bind-checked=\"on\""#));
}

// ── Class toggles (class:name) ──────────────────────────────────────────

#[test]
fn class_toggle_conditional() {
  let code = codegen(r#"<div class:active="is_active">x</div>"#);
  assert!(code.contains("if is_active { __classes.push(\"active\"); }"));
  assert!(code.contains(r#"__html.push_str(" class=\"");"#));
}

#[test]
fn multiple_class_toggles() {
  let code = codegen(r#"<div class:active="a" class:hidden="b">x</div>"#);
  assert!(code.contains("if a { __classes.push(\"active\"); }"));
  assert!(code.contains("if b { __classes.push(\"hidden\"); }"));
}

// ── Style props (style:prop) ────────────────────────────────────────────

#[test]
fn style_prop_emitted() {
  let code = codegen(r#"<div style:color="text_color">x</div>"#);
  assert!(code.contains(r#"__html.push_str(" style=\"");"#));
  assert!(code.contains(r#"__html.push_str("color: ");"#));
  assert!(code.contains(r#"__esc(&format!("{}", { text_color }))"#));
}

#[test]
fn multiple_style_props() {
  let code = codegen(r#"<div style:color="c" style:font-size="fs">x</div>"#);
  assert!(code.contains(r#"__html.push_str("color: ");"#));
  assert!(code.contains(r#"__html.push_str("font-size: ");"#));
  // Semicolon separator between properties.
  assert!(code.contains(r#"__html.push_str("; ");"#));
}

// ── Actions (use:name) ──────────────────────────────────────────────────

#[test]
fn action_emitted_as_data_attr() {
  let code = codegen(r#"<div use:tooltip="config">x</div>"#);
  assert!(code.contains(r#"data-use-tooltip=\"config\""#));
}

#[test]
fn action_without_argument() {
  let code = codegen(r#"<div use:autofocus="">x</div>"#);
  assert!(code.contains("data-use-autofocus"));
}

// ── Component props ─────────────────────────────────────────────────────

#[test]
fn component_with_static_props() {
  let code = codegen(r#"<Button label="Click me" />"#);
  assert!(code.contains(r#"Button::render("Click me")"#));
}

#[test]
fn component_with_dynamic_prop() {
  let code = codegen(r#"<Card title="{{ name }}" />"#);
  assert!(code.contains("Card::render("));
  assert!(code.contains("format!"));
}

#[test]
fn component_prop_literal_braces_escaped() {
  // Literal braces in a mixed prop value must be escaped so format! doesn't choke.
  let code = codegen(r#"<Card title="Hello {world} {{ name }}" />"#);
  // The format string should have {{ and }} for the literal braces.
  assert!(code.contains("Hello {{world}}"));
  // And a real placeholder for the expression.
  assert!(code.contains("format!"));
}

#[test]
fn component_with_children() {
  let code = codegen(r#"<Modal><p>content</p></Modal>"#);
  assert!(code.contains("Modal::render_with_slot(&__comp_slot)"));
  assert!(code.contains(r#"__html.push_str("<p");"#));
}

// ── Named slots ─────────────────────────────────────────────────────────

#[test]
fn named_slot_with_fallback() {
  let code = codegen(r#"<slot name="header">default header</slot>"#);
  // Variable must be declared.
  assert!(code.contains("let __slot_header: Option<&str> = None;"));
  // Lookup and fallback.
  assert!(code.contains("__slot_header.as_ref()"));
  assert!(code.contains(r#"__html.push_str("default header");"#));
}

#[test]
fn named_slot_without_fallback() {
  let code = codegen(r#"<slot name="footer" />"#);
  assert!(code.contains("let __slot_footer: Option<&str> = None;"));
  assert!(code.contains("__slot_footer.as_ref()"));
}

#[test]
fn named_slot_declaration_deduplicated() {
  // Two slots with the same name should produce only one `let` declaration
  // per render function (render + render_with_slot = 2 total).
  let code = codegen(
    r#"{#if show}<slot name="x">a</slot>{:else}<slot name="x">b</slot>{/if}"#,
  );
  assert_eq!(
    code.matches("let __slot_x: Option<&str> = None;").count(),
    2,
    "expected exactly one declaration of __slot_x per render function"
  );
}

// ── Scoped CSS rewriting ────────────────────────────────────────────────

#[test]
fn scoped_css_simple_selector() {
  let code = codegen("<style scoped>.card { display: flex; }</style>\n<div>x</div>");
  // The CSS should have the scope attribute appended to the selector.
  assert!(code.contains(".card[data-s-"));
  assert!(code.contains("{ display: flex; }"));
}

#[test]
fn scoped_css_element_selector() {
  let code = codegen("<style scoped>p { margin: 0; }</style>\n<p>x</p>");
  assert!(code.contains("p[data-s-"));
  assert!(code.contains("{ margin: 0; }"));
}

#[test]
fn scoped_css_compound_selector() {
  let code = codegen("<style scoped>div.card { color: red; }</style>\n<div>x</div>");
  assert!(code.contains("div.card[data-s-"));
}

#[test]
fn scoped_css_descendant_combinator() {
  let code = codegen("<style scoped>.card p { color: red; }</style>\n<div>x</div>");
  // Both parts of the descendant selector should be scoped.
  assert!(code.contains(".card[data-s-"));
  assert!(code.contains("p[data-s-"));
}

#[test]
fn scoped_css_child_combinator() {
  let code = codegen("<style scoped>.a > .b { color: red; }</style>\n<div>x</div>");
  assert!(code.contains(".a[data-s-"));
  assert!(code.contains("> .b[data-s-"));
}

#[test]
fn scoped_css_comma_separated() {
  let code = codegen("<style scoped>.a, .b { color: red; }</style>\n<div>x</div>");
  assert!(code.contains(".a[data-s-"));
  assert!(code.contains(".b[data-s-"));
}

#[test]
fn scoped_css_pseudo_class() {
  let code = codegen("<style scoped>.btn:hover { color: red; }</style>\n<div>x</div>");
  // Scope attr should be inserted before the pseudo-class.
  assert!(code.contains(".btn[data-s-"));
  assert!(code.contains(":hover"));
}

#[test]
fn scoped_css_pseudo_element() {
  let code = codegen("<style scoped>.btn::before { content: ''; }</style>\n<div>x</div>");
  assert!(code.contains(".btn[data-s-"));
  assert!(code.contains("::before"));
}

#[test]
fn unscoped_css_not_rewritten() {
  let code = codegen("<style>.card { display: flex; }</style>\n<div>x</div>");
  assert!(code.contains(".card { display: flex; }"));
  // No scope attribute in the CSS.
  assert!(!code.contains("[data-s-"));
}
