//! Code emission for the build command.
//!
//! Takes discovered route entries (pages, layouts, error pages, components),
//! compiles each `.trs` file through the full pipeline, and writes all
//! generated Rust source files including:
//!
//! - A compiled module for each route / layout / error / component
//! - `mod.rs` files mirroring the directory tree
//! - An HTML document shell helper (`shell.rs`)
//! - A top-level `mod.rs` with an Axum router function
//!
//! Layouts wrap page output. Error pages are registered as 404 fallbacks.
//! Dynamic route segments use Axum `Path<…>` extraction.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use anyhow::Result;

use crate::discover::RouteEntry;
use crate::discover::RouteKind;

// ---------------------------------------------------------------------------
// Component metadata
// ---------------------------------------------------------------------------

/// A discovered component with its lowercase Rust module name and
/// `PascalCase` alias matching the tag name used in templates.
///
/// For example, a file `Header.trs` or `header.trs` both produce
/// `module_name = "header"` and `alias = "Header"`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ComponentInfo {
  /// Lowercase module name used for file names and `mod` declarations.
  module_name: String,
  /// `PascalCase` alias matching the component tag in templates
  /// (e.g. `Header`, `NavBar`).
  alias: String,
}

impl ComponentInfo {
  /// Derive component info from a raw filename stem.
  ///
  /// The module name is always lowercased. The alias is derived by
  /// converting to `PascalCase` so that it matches the tag name the
  /// compiler emits (e.g. `<Header />` → `Header::render()`).
  fn from_stem(stem: &str) -> Self {
    Self {
      module_name: stem.to_lowercase(),
      alias: to_pascal_case(stem),
    }
  }
}

/// Convert a string to `PascalCase`.
///
/// Rules:
/// - Already `PascalCase` (`Header`) → returned as-is.
/// - `snake_case` or `kebab-case` → split on `_`/`-`, capitalise each word.
/// - All-lowercase single word (`header`) → capitalise first letter.
fn to_pascal_case(s: &str) -> String {
  s.split(['_', '-'])
    .filter(|part| !part.is_empty())
    .map(|part| {
      let mut chars = part.chars();
      match chars.next() {
        Some(c) => {
          let mut word = c.to_uppercase().to_string();
          word.extend(chars);
          word
        }
        None => String::new(),
      }
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compile all entries and write generated code to `output_dir`.
///
/// # Safety guard
///
/// The output directory is wiped before writing. To prevent accidental
/// data loss the path must satisfy **all** of the following:
///
/// 1. It is **not** an absolute path.
/// 2. It is **not** empty, `.`, `..`, or `/`.
/// 3. Its final component starts with a dot (e.g. `.thebe`), **or**
///    `force` is `true`.
///
/// # Errors
///
/// Returns an error if any `.trs` file fails to parse or compile,
/// if file I/O fails, or if the output path fails the safety check.
#[allow(clippy::too_many_lines)]
pub fn emit_all(entries: &[RouteEntry], output_dir: &Path, force: bool) -> Result<()> {
  validate_output_dir(output_dir, force)?;

  // Clean and recreate output directory.
  if output_dir.exists() {
    fs::remove_dir_all(output_dir)?;
  }
  fs::create_dir_all(output_dir)?;

  let routes_dir = output_dir.join("routes");
  fs::create_dir_all(&routes_dir)?;

  let comp_dir = output_dir.join("components");

  // Partition entries.
  let pages: Vec<_> = entries
    .iter()
    .filter(|e| e.kind == RouteKind::Page)
    .collect();
  let layouts: Vec<_> = entries
    .iter()
    .filter(|e| e.kind == RouteKind::Layout)
    .collect();
  let errors: Vec<_> = entries
    .iter()
    .filter(|e| e.kind == RouteKind::Error)
    .collect();
  let components: Vec<_> = entries
    .iter()
    .filter(|e| e.kind == RouteKind::Component)
    .collect();

  // Create components directory early if needed.
  if !components.is_empty() {
    fs::create_dir_all(&comp_dir)?;
  }

  // Build component metadata: lowercase module name + PascalCase alias.
  let component_infos: Vec<ComponentInfo> = components
    .iter()
    .map(|c| {
      let stem = c.module_segments.last().expect("empty module segments");
      ComponentInfo::from_stem(stem)
    })
    .collect();

  // Compile and write each entry as a module.
  // Track which components each route module references (for style collection).
  let mut component_refs: HashMap<PathBuf, Vec<String>> = HashMap::new();
  for entry in entries {
    if entry.kind == RouteKind::Component {
      compile_trs(entry, &comp_dir, &[], true)?;
    } else {
      let refs = compile_trs(entry, &routes_dir, &component_infos, false)?;
      if !refs.is_empty() {
        component_refs.insert(entry.relative_path.clone(), refs);
      }
    }
  }

  // Module tree for routes/.
  let route_entries: Vec<_> = entries
    .iter()
    .filter(|e| e.kind != RouteKind::Component)
    .cloned()
    .collect();
  let mod_tree = build_mod_tree(&route_entries);
  emit_mod_tree(&routes_dir, &mod_tree)?;

  // Module tree for components/ (lowercase module names).
  if !components.is_empty() {
    let mut comp_mod = String::from("// Auto-generated by thebe. Do not edit.\n");
    comp_mod.push_str("#![allow(clippy::all, unused)]\n\n");
    for ci in &component_infos {
      writeln!(comp_mod, "pub mod {};", ci.module_name).expect("write to String");
    }
    fs::write(comp_dir.join("mod.rs"), &comp_mod)?;
  }

  // shell.rs
  emit_shell(output_dir)?;

  // Build layout lookup: directory path → module path.
  let layout_map = build_layout_map(&layouts);

  // Build a mapping from layout directory → layout relative path, so we
  // can look up which components a layout references.
  let layout_source_map: HashMap<PathBuf, PathBuf> = layouts
    .iter()
    .map(|l| {
      let dir = l.relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
      (dir, l.relative_path.clone())
    })
    .collect();

  // Build per-page component sets: merge each page's own component refs
  // with its layout's refs so only the actually-used styles are emitted.
  let mut page_component_refs: HashMap<PathBuf, Vec<String>> = HashMap::new();
  for page in &pages {
    let mut used: Vec<String> = component_refs
      .get(&page.relative_path)
      .cloned()
      .unwrap_or_default();

    // Walk up to find the page's layout and merge its refs too.
    if let Some(layout_segs) = find_layout_for_page(page, &layout_map) {
      let layout_dir = layout_segs[..layout_segs.len() - 1]
        .iter()
        .fold(PathBuf::new(), |acc, seg| acc.join(seg));
      if let Some(layout_rel) = layout_source_map.get(&layout_dir)
        && let Some(layout_refs) = component_refs.get(layout_rel)
      {
        for r in layout_refs {
          if !used.contains(r) {
            used.push(r.clone());
          }
        }
      }
    }

    if !used.is_empty() {
      page_component_refs.insert(page.relative_path.clone(), used);
    }
  }

  // Find the root error page (if any).
  let root_error = errors.iter().find(|e| e.url_path == "/");

  // Top-level mod.rs with router.
  emit_root_mod(
    output_dir,
    &pages,
    &layout_map,
    root_error.copied(),
    !component_infos.is_empty(),
    &page_component_refs,
  )?;

  Ok(())
}

// ---------------------------------------------------------------------------
// Output directory validation
// ---------------------------------------------------------------------------

/// Validate that the output path is safe to wipe.
///
/// Works on both relative and resolved (absolute) paths. Checks:
/// 1. The path has a final component (not empty or `/`).
/// 2. That component is not `.` or `..`.
/// 3. The path has at least two components (prevents wiping mount roots).
/// 4. Unless `force` is set, the final component starts with `.` (dotdir).
fn validate_output_dir(output_dir: &Path, force: bool) -> Result<()> {
  let dir_name = output_dir
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("");

  anyhow::ensure!(
    !dir_name.is_empty(),
    "output directory must not be a filesystem root or empty path",
  );

  anyhow::ensure!(
    dir_name != "." && dir_name != "..",
    "output directory must not be `.` or `..`",
  );

  // An absolute path with only one real component (e.g. `/tmp`) is dangerous.
  // A relative dot-prefixed path (`.thebe`) is fine even with one component.
  if output_dir.is_absolute() {
    anyhow::ensure!(
      output_dir.components().count() >= 3,
      "absolute output path is too shallow; refusing to wipe `{}`",
      output_dir.display(),
    );
  }

  if !force {
    anyhow::ensure!(
      dir_name.starts_with('.'),
      "output directory name `{dir_name}` does not start with a dot; \
       use a dot-prefixed name (e.g. `.thebe`) or pass --force",
    );
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// Single-file compilation
// ---------------------------------------------------------------------------

/// Compile a single `.trs` file and write the generated Rust module.
///
/// `component_infos` provides the `PascalCase` alias and lowercase module
/// name for each known component. For route modules (pages, layouts,
/// errors), `use` imports are injected so that `Alias::render()` calls in
/// the generated code resolve to the compiled component modules.
///
/// When `lowercase_leaf` is true, the leaf module filename is lowercased
/// (used for component files to follow Rust naming conventions).
///
/// Returns the list of component **module names** that this entry
/// references (empty for entries that don't use any components).
fn compile_trs(
  entry: &RouteEntry,
  base_dir: &Path,
  component_infos: &[ComponentInfo],
  lowercase_leaf: bool,
) -> Result<Vec<String>> {
  let source = fs::read_to_string(&entry.source_path)
    .with_context(|| format!("reading {}", entry.source_path.display()))?;
  let filename = entry.relative_path.to_str().unwrap_or("<unknown>");

  let ast = thebe_ast::parse(&source).map_err(|e| {
    let _ = thebe_ast::diagnostics::eprint_error(&e, &source, Some(filename));
    anyhow::anyhow!("parse error in {filename}")
  })?;

  let ir = thebe_compiler::lower(&source, &ast).map_err(|e| {
    let _ = thebe_compiler::diagnostics::eprint_error(&e, &source, Some(filename));
    anyhow::anyhow!("compile error in {filename}")
  })?;

  let warnings = thebe_compiler::validate(&ir);
  for w in &warnings {
    let _ = thebe_compiler::diagnostics::eprint_warning(w, &source, Some(filename));
  }

  let mut code = thebe_compiler::generate(&ir, &entry.params).map_err(|e| {
    let _ = thebe_compiler::diagnostics::eprint_error(&e, &source, Some(filename));
    anyhow::anyhow!("codegen error in {filename}")
  })?;

  // Inject `use` imports for any component this module references.
  // The codegen emits `Alias::render()` (PascalCase tag name), so we match
  // on the alias and map it to the lowercase module name.
  let referenced: Vec<&ComponentInfo> = component_infos
    .iter()
    .filter(|ci| code.contains(&format!("{}::render()", ci.alias)))
    .collect();

  let ref_module_names: Vec<String> = referenced.iter().map(|ci| ci.module_name.clone()).collect();

  if !referenced.is_empty() {
    // Module depth within `routes/`: segments give us how deep the module is
    // within the directory tree. We need one extra `super::` to escape the
    // `routes` module itself and reach the root where `components` lives.
    let supers = "super::".repeat(entry.module_segments.len() + 1);
    let mut imports = String::new();
    for ci in &referenced {
      writeln!(
        imports,
        "use {supers}components::{mod_name} as {alias};",
        mod_name = ci.module_name,
        alias = ci.alias,
      )
      .expect("write to String");
    }
    imports.push('\n');
    code.insert_str(0, &imports);
  }

  // Build target file path, creating intermediate directories.
  let mut target_dir = base_dir.to_path_buf();
  for segment in &entry.module_segments[..entry.module_segments.len() - 1] {
    target_dir = target_dir.join(segment);
    fs::create_dir_all(&target_dir)?;
  }

  let leaf = entry.module_segments.last().expect("empty module segments");
  let out_name = if lowercase_leaf {
    format!("{}.rs", leaf.to_lowercase())
  } else {
    format!("{leaf}.rs")
  };
  let file_path = target_dir.join(out_name);

  fs::write(&file_path, &code)
    .with_context(|| format!("writing {}", file_path.display()))?;

  Ok(ref_module_names)
}

// ---------------------------------------------------------------------------
// Module tree generation
// ---------------------------------------------------------------------------

/// A tree of module names used to generate `mod.rs` files.
#[derive(Default, Debug)]
struct ModTree {
  children: BTreeMap<String, ModTree>,
}

fn build_mod_tree(routes: &[RouteEntry]) -> ModTree {
  let mut root = ModTree::default();
  for route in routes {
    let mut node = &mut root;
    for segment in &route.module_segments {
      node = node.children.entry(segment.clone()).or_default();
    }
  }
  root
}

fn emit_mod_tree(dir: &Path, tree: &ModTree) -> Result<()> {
  if tree.children.is_empty() {
    return Ok(());
  }

  let mut content = String::from("// Auto-generated by thebe. Do not edit.\n");
  content.push_str("#![allow(clippy::all, unused)]\n\n");

  for name in tree.children.keys() {
    writeln!(content, "pub mod {name};").expect("write to String");
  }

  fs::write(dir.join("mod.rs"), &content)?;

  for (name, child) in &tree.children {
    if !child.children.is_empty() {
      let child_dir = dir.join(name);
      fs::create_dir_all(&child_dir)?;
      emit_mod_tree(&child_dir, child)?;
    }
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// Layout map
// ---------------------------------------------------------------------------

/// Build a mapping from directory relative path → layout module path.
///
/// `_layout.trs` (root) → `["_layout"]`
/// `blog/_layout.trs` → `["blog", "_layout"]`
fn build_layout_map(layouts: &[&RouteEntry]) -> HashMap<PathBuf, Vec<String>> {
  let mut map = HashMap::new();
  for layout in layouts {
    let dir = layout
      .relative_path
      .parent()
      .unwrap_or(Path::new(""))
      .to_path_buf();
    map.insert(dir, layout.module_segments.clone());
  }
  map
}

/// Find the most specific layout for a page route.
///
/// Walks from the page's directory up to the root, returning the first
/// matching layout module path.
fn find_layout_for_page(
  page: &RouteEntry,
  layout_map: &HashMap<PathBuf, Vec<String>>,
) -> Option<Vec<String>> {
  let mut dir = page
    .relative_path
    .parent()
    .unwrap_or(Path::new(""))
    .to_path_buf();

  loop {
    if let Some(layout_mod) = layout_map.get(&dir) {
      return Some(layout_mod.clone());
    }
    if !dir.pop() {
      // Check root.
      return layout_map.get(&PathBuf::new()).cloned();
    }
  }
}

// ---------------------------------------------------------------------------
// Shell (HTML document wrapper)
// ---------------------------------------------------------------------------

fn emit_shell(output_dir: &Path) -> Result<()> {
  let content = r#"// Auto-generated by thebe. Do not edit.

/// Wrap a rendered component body in a full HTML document.
pub fn document(body: &str, styles: &[&str]) -> String {
  let mut html = String::from("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
  html.push_str("<meta charset=\"utf-8\">\n");
  html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
  for css in styles {
    if !css.is_empty() {
      html.push_str("<style>");
      html.push_str(css);
      html.push_str("</style>\n");
    }
  }
  html.push_str("</head>\n<body>\n");
  html.push_str(body);
  html.push_str("\n</body>\n</html>");
  html
}
"#;

  fs::write(output_dir.join("shell.rs"), content)?;
  Ok(())
}

// ---------------------------------------------------------------------------
// Root module + router
// ---------------------------------------------------------------------------

/// Write the top-level `mod.rs` with the Axum router function.
#[allow(clippy::too_many_lines)]
fn emit_root_mod(
  output_dir: &Path,
  pages: &[&RouteEntry],
  layout_map: &HashMap<PathBuf, Vec<String>>,
  root_error: Option<&RouteEntry>,
  has_components: bool,
  component_refs: &HashMap<PathBuf, Vec<String>>,
) -> Result<()> {
  let mut code = String::from("// Auto-generated by thebe. Do not edit.\n");
  code.push_str("#![allow(clippy::all, unused)]\n\n");
  code.push_str("pub mod routes;\nmod shell;\n");

  if has_components {
    code.push_str("pub mod components;\n");
  }

  code.push('\n');

  // Router function.
  code.push_str("/// Build an [`axum::Router`] with all discovered routes.\n");
  code.push_str("pub fn router() -> axum::Router {\n");
  code.push_str("  axum::Router::new()\n");

  for page in pages {
    let handler = handler_name(&page.module_segments);
    writeln!(
      code,
      "    .route(\"{}\", axum::routing::get({handler}))",
      page.url_path
    )
    .expect("write to String");
  }

  // 404 fallback if there's a root _error.trs.
  if root_error.is_some() {
    code.push_str("    .fallback(__fallback)\n");
  }

  code.push_str("}\n");

  // Handler functions for each page.
  for page in pages {
    let handler = handler_name(&page.module_segments);
    let mod_path = page.module_segments.join("::");
    let layout_mod = find_layout_for_page(page, layout_map);

    code.push('\n');

    if page.params.is_empty() {
      // Static route — no path params.
      writeln!(
        code,
        "async fn {handler}() -> axum::response::Html<String> {{"
      )
      .expect("write to String");
    } else {
      // Dynamic route — extract path params.
      if page.params.len() == 1 {
        // Single param: Path<String>
        writeln!(
          code,
          "async fn {handler}(\n  axum::extract::Path({}): axum::extract::Path<String>,\n) -> axum::response::Html<String> {{",
          page.params[0]
        )
        .expect("write to String");
      } else {
        // Multiple params: Path<(String, String, ...)>
        let param_types = build_param_types(&page.params);
        writeln!(
          code,
          "async fn {handler}(\n  axum::extract::Path(({})): axum::extract::Path<({param_types})>,\n) -> axum::response::Html<String> {{",
          page.params.join(", ")
        )
        .expect("write to String");
      }
    }

    // Render the page body.
    if page.params.is_empty() {
      writeln!(code, "  let body = routes::{mod_path}::render();")
        .expect("write to String");
    } else {
      let args = page
        .params
        .iter()
        .map(|p| format!("&{p}"))
        .collect::<Vec<_>>()
        .join(", ");
      writeln!(code, "  let body = routes::{mod_path}::render({args});")
        .expect("write to String");
    }

    // Wrap in layout if one exists.
    if let Some(layout_segs) = &layout_mod {
      let layout_path = layout_segs.join("::");
      writeln!(
        code,
        "  let body = routes::{layout_path}::render_with_slot(&body, &[]);"
      )
      .expect("write to String");
    }

    // Collect styles.
    writeln!(code, "  let mut all_styles: Vec<&str> = Vec::new();")
      .expect("write to String");
    writeln!(
      code,
      "  all_styles.extend_from_slice(routes::{mod_path}::STYLES);"
    )
    .expect("write to String");
    if let Some(layout_segs) = &layout_mod {
      let layout_path = layout_segs.join("::");
      writeln!(
        code,
        "  all_styles.extend_from_slice(routes::{layout_path}::STYLES);"
      )
      .expect("write to String");
    }
    // Collect component styles only for components actually used by this
    // page and its layout.
    if let Some(used) = component_refs.get(&page.relative_path) {
      for comp_mod in used {
        writeln!(
          code,
          "  all_styles.extend_from_slice(components::{comp_mod}::STYLES);",
        )
        .expect("write to String");
      }
    }

    writeln!(
      code,
      "  axum::response::Html(shell::document(&body, &all_styles))"
    )
    .expect("write to String");
    code.push_str("}\n");
  }

  // 404 fallback handler.
  if let Some(err) = root_error {
    let err_mod = err.module_segments.join("::");
    code.push('\n');
    code.push_str("async fn __fallback() -> (axum::http::StatusCode, axum::response::Html<String>) {\n");
    writeln!(code, "  let body = routes::{err_mod}::render();")
      .expect("write to String");
    writeln!(
      code,
      "  let styles = routes::{err_mod}::STYLES;"
    )
    .expect("write to String");
    code.push_str(
      "  (axum::http::StatusCode::NOT_FOUND, axum::response::Html(shell::document(&body, styles)))\n",
    );
    code.push_str("}\n");
  }

  fs::write(output_dir.join("mod.rs"), &code)?;
  Ok(())
}

/// Build Axum `Path` type tuple: all `String`.
fn build_param_types(params: &[String]) -> String {
  std::iter::repeat_n("String", params.len())
    .collect::<Vec<_>>()
    .join(", ")
}

/// Build a unique handler function name from module segments.
///
/// Strips leading underscores from dynamic segments to avoid double
/// underscores in the name (e.g. `["blog", "_slug"]` → `__route_blog_slug`).
fn handler_name(segments: &[String]) -> String {
  let clean: Vec<_> = segments
    .iter()
    .map(|s| s.trim_start_matches('_'))
    .collect();
  format!("__route_{}", clean.join("_"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::BTreeSet;

  #[test]
  fn handler_name_flat() {
    let segs = vec!["index".to_string()];
    assert_eq!(handler_name(&segs), "__route_index");
  }

  #[test]
  fn handler_name_nested() {
    let segs = vec!["blog".to_string(), "post".to_string()];
    assert_eq!(handler_name(&segs), "__route_blog_post");
  }

  #[test]
  fn handler_name_dynamic() {
    let segs = vec!["blog".to_string(), "_id".to_string()];
    assert_eq!(handler_name(&segs), "__route_blog_id");
  }

  #[test]
  fn mod_tree_flat() {
    let routes = vec![entry(&["index"], RouteKind::Page), entry(&["about"], RouteKind::Page)];
    let tree = build_mod_tree(&routes);
    let names: Vec<_> = tree.children.keys().collect();
    assert_eq!(names, vec!["about", "index"]);
  }

  #[test]
  fn mod_tree_nested() {
    let routes = vec![
      entry(&["index"], RouteKind::Page),
      entry(&["blog", "index"], RouteKind::Page),
      entry(&["blog", "post"], RouteKind::Page),
    ];
    let tree = build_mod_tree(&routes);
    assert!(tree.children.contains_key("blog"));
    let blog = &tree.children["blog"];
    let blog_children: BTreeSet<_> = blog.children.keys().cloned().collect();
    assert!(blog_children.contains("index"));
    assert!(blog_children.contains("post"));
  }

  #[test]
  fn param_types_single() {
    assert_eq!(build_param_types(&["id".into()]), "String");
  }

  #[test]
  fn param_types_multiple() {
    assert_eq!(
      build_param_types(&["org".into(), "repo".into()]),
      "String, String"
    );
  }

  #[test]
  fn layout_map_root() {
    let layout = route_entry(&["_layout"], RouteKind::Layout, "_layout.trs");
    let map = build_layout_map(&[&layout]);
    assert!(map.contains_key(&PathBuf::new()));
  }

  #[test]
  fn layout_map_nested() {
    let layout = route_entry(&["blog", "_layout"], RouteKind::Layout, "blog/_layout.trs");
    let map = build_layout_map(&[&layout]);
    assert!(map.contains_key(&PathBuf::from("blog")));
  }

  #[test]
  fn find_layout_root() {
    let layout = route_entry(&["_layout"], RouteKind::Layout, "_layout.trs");
    let map = build_layout_map(&[&layout]);
    let page = route_entry(&["index"], RouteKind::Page, "index.trs");
    let found = find_layout_for_page(&page, &map);
    assert_eq!(found, Some(vec!["_layout".to_string()]));
  }

  #[test]
  fn find_layout_nested_inherits_parent() {
    let layout = route_entry(&["_layout"], RouteKind::Layout, "_layout.trs");
    let map = build_layout_map(&[&layout]);
    let page = route_entry(&["blog", "post"], RouteKind::Page, "blog/post.trs");
    let found = find_layout_for_page(&page, &map);
    assert_eq!(found, Some(vec!["_layout".to_string()]));
  }

  #[test]
  fn find_layout_nearest_wins() {
    let root_layout = route_entry(&["_layout"], RouteKind::Layout, "_layout.trs");
    let blog_layout =
      route_entry(&["blog", "_layout"], RouteKind::Layout, "blog/_layout.trs");
    let map = build_layout_map(&[&root_layout, &blog_layout]);
    let page = route_entry(&["blog", "post"], RouteKind::Page, "blog/post.trs");
    let found = find_layout_for_page(&page, &map);
    assert_eq!(
      found,
      Some(vec!["blog".to_string(), "_layout".to_string()])
    );
  }

  #[test]
  fn find_layout_none() {
    let map = HashMap::new();
    let page = route_entry(&["about"], RouteKind::Page, "about.trs");
    assert_eq!(find_layout_for_page(&page, &map), None);
  }

  /// Helper with minimal fields.
  fn entry(segments: &[&str], kind: RouteKind) -> RouteEntry {
    RouteEntry {
      source_path: PathBuf::new(),
      relative_path: PathBuf::new(),
      url_path: String::new(),
      module_segments: segments.iter().map(|s| (*s).to_string()).collect(),
      kind,
      params: Vec::new(),
    }
  }

  /// Helper with a relative path for layout lookups.
  fn route_entry(segments: &[&str], kind: RouteKind, rel: &str) -> RouteEntry {
    RouteEntry {
      source_path: PathBuf::new(),
      relative_path: PathBuf::from(rel),
      url_path: String::new(),
      module_segments: segments.iter().map(|s| (*s).to_string()).collect(),
      kind,
      params: Vec::new(),
    }
  }

  // ── to_pascal_case ───────────────────────────────────────────────────

  #[test]
  fn pascal_already_pascal() {
    assert_eq!(to_pascal_case("Header"), "Header");
  }

  #[test]
  fn pascal_lowercase_single() {
    assert_eq!(to_pascal_case("header"), "Header");
  }

  #[test]
  fn pascal_snake_case() {
    assert_eq!(to_pascal_case("nav_bar"), "NavBar");
  }

  #[test]
  fn pascal_kebab_case() {
    assert_eq!(to_pascal_case("nav-bar"), "NavBar");
  }

  #[test]
  fn pascal_mixed_snake_pascal() {
    assert_eq!(to_pascal_case("my_Button"), "MyButton");
  }

  #[test]
  fn pascal_single_char() {
    assert_eq!(to_pascal_case("x"), "X");
  }

  // ── ComponentInfo ───────────────────────────────────────────────────

  #[test]
  fn component_info_from_pascal() {
    let ci = ComponentInfo::from_stem("Header");
    assert_eq!(ci.module_name, "header");
    assert_eq!(ci.alias, "Header");
  }

  #[test]
  fn component_info_from_lowercase() {
    let ci = ComponentInfo::from_stem("header");
    assert_eq!(ci.module_name, "header");
    assert_eq!(ci.alias, "Header");
  }

  #[test]
  fn component_info_from_snake() {
    let ci = ComponentInfo::from_stem("nav_bar");
    assert_eq!(ci.module_name, "nav_bar");
    assert_eq!(ci.alias, "NavBar");
  }

  // ── Output directory validation ─────────────────────────────────────

  #[test]
  fn validate_rejects_root() {
    assert!(validate_output_dir(Path::new("/"), false).is_err());
  }

  #[test]
  fn validate_rejects_dot() {
    assert!(validate_output_dir(Path::new("."), false).is_err());
  }

  #[test]
  fn validate_rejects_dotdot() {
    assert!(validate_output_dir(Path::new(".."), false).is_err());
  }

  #[test]
  fn validate_rejects_shallow_absolute() {
    // Single-component absolute path like `/tmp`.
    assert!(validate_output_dir(Path::new("/tmp"), false).is_err());
  }

  #[test]
  fn validate_rejects_non_dot_without_force() {
    assert!(validate_output_dir(Path::new("project/generated"), false).is_err());
  }

  #[test]
  fn validate_accepts_dot_prefixed_relative() {
    assert!(validate_output_dir(Path::new("project/.thebe"), false).is_ok());
  }

  #[test]
  fn validate_accepts_plain_relative() {
    assert!(validate_output_dir(Path::new(".thebe"), false).is_ok());
  }

  #[test]
  fn validate_accepts_non_dot_with_force() {
    assert!(validate_output_dir(Path::new("project/generated"), true).is_ok());
  }

  #[test]
  fn validate_accepts_deep_absolute_dot() {
    assert!(validate_output_dir(Path::new("/Users/x/project/.thebe"), false).is_ok());
  }

  #[test]
  fn validate_rejects_deep_absolute_no_dot() {
    assert!(validate_output_dir(Path::new("/Users/x/project/out"), false).is_err());
  }

  #[test]
  fn validate_accepts_deep_absolute_force() {
    assert!(validate_output_dir(Path::new("/Users/x/project/out"), true).is_ok());
  }
}
