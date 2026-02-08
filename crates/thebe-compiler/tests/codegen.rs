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

  // Style constant.
  assert!(code.contains("h1 { color: blue; }"));

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
