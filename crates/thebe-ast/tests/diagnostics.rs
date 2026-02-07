use thebe_ast::diagnostics::error_to_string;

#[test]
fn diag_empty_input() {
  let source = "";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("empty.trs"));
  assert!(output.contains("input is empty"), "got:\n{output}");
  assert!(output.contains("empty.trs"), "got:\n{output}");
}

#[test]
fn diag_duplicate_script_setup() {
  let source = "<script setup>\nlet a = 1;\n</script>\n<script setup>\nlet b = 2;\n</script>";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("app.trs"));
  assert!(
    output.contains("duplicate <script setup>"),
    "got:\n{output}"
  );
  assert!(
    output.contains("second <script setup> found here"),
    "got:\n{output}"
  );
}

#[test]
fn diag_duplicate_script() {
  let source = "<script lang=\"ts\">\nfoo();\n</script>\n<script lang=\"ts\">\nbar();\n</script>";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("app.trs"));
  assert!(output.contains("duplicate <script>"), "got:\n{output}");
}

#[test]
fn diag_missing_script_lang() {
  let source = "<script>\nfoo();\n</script>";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("app.trs"));
  assert!(
    output.contains("<script> requires a lang attribute"),
    "got:\n{output}"
  );
  assert!(output.contains("missing lang attribute"), "got:\n{output}");
  // Should include the help hint.
  assert!(output.contains("Help"), "got:\n{output}");
}

#[test]
fn diag_invalid_setup_lang() {
  let source = "<script setup lang=\"ts\">\nfoo();\n</script>";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("app.trs"));
  assert!(
    output.contains("does not accept a lang attribute"),
    "got:\n{output}"
  );
}

#[test]
fn diag_unclosed_interpolation() {
  let err = thebe_ast::parse_template("Hello {{ name", 0).unwrap_err();
  let output = error_to_string(&err, "Hello {{ name", Some("tmpl.trs"));
  assert!(output.contains("unclosed interpolation"), "got:\n{output}");
  assert!(output.contains("no matching }}"), "got:\n{output}");
}

#[test]
fn diag_malformed_tag() {
  let source = "<script setup>\nlet x = 1;";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("broken.trs"));
  assert!(output.contains("malformed tag"), "got:\n{output}");
}

#[test]
fn diag_no_filename_shows_unknown() {
  let source = "";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, None);
  assert!(output.contains("<unknown>"), "got:\n{output}");
}

#[test]
fn diag_nested_script() {
  let source = "<div>\n<script>\nfoo();\n</script>\n</div>";
  let err = thebe_ast::parse(source).unwrap_err();
  let output = error_to_string(&err, source, Some("nested.trs"));
  assert!(
    output.contains("<script> tag inside template content"),
    "got:\n{output}"
  );
}

#[test]
fn diag_write_to_vec() {
  let source = "";
  let err = thebe_ast::parse(source).unwrap_err();
  let mut buf = Vec::new();
  thebe_ast::diagnostics::write_error(&err, source, Some("test.trs"), &mut buf).unwrap();
  let output = String::from_utf8(buf).unwrap();
  assert!(!output.is_empty());
}

#[test]
fn diag_unclosed_if_block() {
  let source = "{#if show}<p>oops</p>";
  let err = thebe_ast::parse_html(source, 0).unwrap_err();
  let output = error_to_string(&err, source, Some("cf.trs"));
  assert!(output.contains("unclosed {#if} block"), "got:\n{output}");
  assert!(output.contains("no matching {/if}"), "got:\n{output}");
  assert!(output.contains("Help"), "got:\n{output}");
}

#[test]
fn diag_unclosed_each_block() {
  let source = "{#each items as item}<li>x</li>";
  let err = thebe_ast::parse_html(source, 0).unwrap_err();
  let output = error_to_string(&err, source, Some("cf.trs"));
  assert!(output.contains("unclosed {#each} block"), "got:\n{output}");
  assert!(output.contains("no matching {/each}"), "got:\n{output}");
  assert!(output.contains("Help"), "got:\n{output}");
}

#[test]
fn diag_invalid_each_expression() {
  let source = "{#each items}<li>x</li>{/each}";
  let err = thebe_ast::parse_html(source, 0).unwrap_err();
  let output = error_to_string(&err, source, Some("cf.trs"));
  assert!(
    output.contains("invalid {#each} expression"),
    "got:\n{output}"
  );
  assert!(output.contains("could not parse"), "got:\n{output}");
  assert!(output.contains("Help"), "got:\n{output}");
}
