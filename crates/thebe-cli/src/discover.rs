//! Route file discovery.
//!
//! Walks a directory tree to find `.trs` files and classifies them into
//! page routes, layouts, error pages, and components. Also supports
//! dynamic route segments (e.g. `[id].trs` → `/:id`).

use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Classification of a discovered `.trs` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteKind {
  /// A normal page route.
  Page,
  /// A layout that wraps sibling and child routes (`_layout.trs`).
  Layout,
  /// An error page (`_error.trs`, used as 404/500 fallback).
  Error,
  /// A reusable component (lives under `components/`).
  Component,
}

/// A discovered `.trs` file with computed metadata.
#[derive(Debug, Clone)]
pub struct RouteEntry {
  /// Absolute path to the `.trs` source file.
  pub source_path: PathBuf,
  /// Path relative to the base directory (e.g. `index.trs`).
  pub relative_path: PathBuf,
  /// URL route path (e.g. `/`, `/about`, `/blog/{id}`).
  pub url_path: String,
  /// Rust module path segments (e.g. `["blog", "_id"]`).
  pub module_segments: Vec<String>,
  /// What kind of file this is.
  pub kind: RouteKind,
  /// Names of dynamic path parameters, in order (e.g. `["id"]`).
  pub params: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Recursively discover all `.trs` files under `routes_dir` and
/// optionally a `components_dir`.
///
/// Results are sorted by relative path for deterministic output.
pub fn discover_routes(
  routes_dir: &Path,
  components_dir: Option<&Path>,
) -> Result<Vec<RouteEntry>> {
  let mut entries = Vec::new();
  walk_dir(routes_dir, routes_dir, &mut entries, false)?;

  if let Some(comp_dir) = components_dir
    && comp_dir.exists()
  {
    walk_dir(comp_dir, comp_dir, &mut entries, true)?;
  }

  entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
  Ok(entries)
}

// ---------------------------------------------------------------------------
// Walking
// ---------------------------------------------------------------------------

fn walk_dir(
  base: &Path,
  dir: &Path,
  entries: &mut Vec<RouteEntry>,
  is_components: bool,
) -> Result<()> {
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.is_dir() {
      walk_dir(base, &path, entries, is_components)?;
    } else if path.extension().is_some_and(|ext| ext == "trs") {
      let relative = path.strip_prefix(base)?.to_path_buf();
      let kind = classify(&relative, is_components);
      let url_path = compute_url_path(&relative);
      let params = extract_params(&relative);
      let module_segments = compute_module_segments(&relative);

      entries.push(RouteEntry {
        source_path: path,
        relative_path: relative,
        url_path,
        module_segments,
        kind,
        params,
      });
    }
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a `.trs` file based on its stem.
fn classify(relative: &Path, is_components: bool) -> RouteKind {
  if is_components {
    return RouteKind::Component;
  }
  let stem = relative
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("");
  match stem {
    "_layout" => RouteKind::Layout,
    "_error" => RouteKind::Error,
    _ => RouteKind::Page,
  }
}

// ---------------------------------------------------------------------------
// URL path computation
// ---------------------------------------------------------------------------

/// Convert a relative `.trs` file path to a URL route path.
///
/// - `index.trs` → `/`
/// - `about.trs` → `/about`
/// - `blog/index.trs` → `/blog`
/// - `blog/[id].trs` → `/blog/{id}`
/// - `_layout.trs` → `/` (layouts get their parent path)
/// - `_error.trs` → `/` (error pages get their parent path)
pub fn compute_url_path(relative: &Path) -> String {
  let stem = relative
    .file_stem()
    .expect("file has no stem")
    .to_str()
    .expect("non-utf8 path");
  let parent = relative.parent().unwrap_or(Path::new(""));

  let mut segments: Vec<String> = parent
    .components()
    .map(|c| {
      let s = c.as_os_str().to_str().expect("non-utf8 path");
      convert_segment(s)
    })
    .collect();

  if stem != "index" && stem != "_layout" && stem != "_error" {
    segments.push(convert_segment(stem));
  }

  if segments.is_empty() {
    "/".to_string()
  } else {
    format!("/{}", segments.join("/"))
  }
}

/// Convert a path segment, replacing `[param]` with `{param}` (Axum 0.8 syntax).
fn convert_segment(segment: &str) -> String {
  if segment.starts_with('[') && segment.ends_with(']') {
    format!("{{{}}}", &segment[1..segment.len() - 1])
  } else {
    segment.to_string()
  }
}

// ---------------------------------------------------------------------------
// Parameter extraction
// ---------------------------------------------------------------------------

/// Extract dynamic parameter names from the path.
///
/// - `blog/[id].trs` → `["id"]`
/// - `[org]/[repo].trs` → `["org", "repo"]`
/// - `about.trs` → `[]`
pub fn extract_params(relative: &Path) -> Vec<String> {
  let stem = relative
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("");
  let parent = relative.parent().unwrap_or(Path::new(""));

  let mut params = Vec::new();

  for component in parent.components() {
    let s = component.as_os_str().to_str().unwrap_or("");
    if s.starts_with('[') && s.ends_with(']') {
      params.push(s[1..s.len() - 1].to_string());
    }
  }

  if stem.starts_with('[') && stem.ends_with(']') {
    params.push(stem[1..stem.len() - 1].to_string());
  }

  params
}

// ---------------------------------------------------------------------------
// Module segments
// ---------------------------------------------------------------------------

/// Convert a relative `.trs` file path to Rust module path segments.
///
/// Dynamic segments like `[id]` become `_id` (valid Rust identifiers).
///
/// - `index.trs` → `["index"]`
/// - `blog/[id].trs` → `["blog", "_id"]`
pub fn compute_module_segments(relative: &Path) -> Vec<String> {
  let stem = relative
    .file_stem()
    .expect("file has no stem")
    .to_str()
    .expect("non-utf8 path");
  let parent = relative.parent().unwrap_or(Path::new(""));

  let mut segments: Vec<String> = parent
    .components()
    .map(|c| {
      let s = c.as_os_str().to_str().expect("non-utf8 path");
      sanitize_module_name(s)
    })
    .collect();

  segments.push(sanitize_module_name(stem));
  segments
}

/// Make a path segment a valid Rust identifier.
///
/// `[id]` → `_id`, otherwise passthrough.
fn sanitize_module_name(segment: &str) -> String {
  if segment.starts_with('[') && segment.ends_with(']') {
    format!("_{}", &segment[1..segment.len() - 1])
  } else {
    segment.to_string()
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  // ── URL paths ──────────────────────────────────────────────────────

  #[test]
  fn index_maps_to_root() {
    assert_eq!(compute_url_path(Path::new("index.trs")), "/");
  }

  #[test]
  fn about_maps_to_about() {
    assert_eq!(compute_url_path(Path::new("about.trs")), "/about");
  }

  #[test]
  fn nested_index_maps_to_parent() {
    assert_eq!(compute_url_path(Path::new("blog/index.trs")), "/blog");
  }

  #[test]
  fn nested_leaf_maps_to_full_path() {
    assert_eq!(
      compute_url_path(Path::new("blog/post.trs")),
      "/blog/post"
    );
  }

  #[test]
  fn deeply_nested() {
    assert_eq!(
      compute_url_path(Path::new("docs/api/v2/index.trs")),
      "/docs/api/v2"
    );
  }

  #[test]
  fn dynamic_segment_in_filename() {
    assert_eq!(
      compute_url_path(Path::new("blog/[id].trs")),
      "/blog/{id}"
    );
  }

  #[test]
  fn dynamic_segment_in_directory() {
    assert_eq!(
      compute_url_path(Path::new("[org]/[repo].trs")),
      "/{org}/{repo}"
    );
  }

  #[test]
  fn layout_maps_to_parent_path() {
    assert_eq!(compute_url_path(Path::new("_layout.trs")), "/");
    assert_eq!(
      compute_url_path(Path::new("blog/_layout.trs")),
      "/blog"
    );
  }

  #[test]
  fn error_maps_to_parent_path() {
    assert_eq!(compute_url_path(Path::new("_error.trs")), "/");
    assert_eq!(
      compute_url_path(Path::new("blog/_error.trs")),
      "/blog"
    );
  }

  // ── Params ─────────────────────────────────────────────────────────

  #[test]
  fn no_params_for_static() {
    assert!(extract_params(Path::new("about.trs")).is_empty());
  }

  #[test]
  fn single_param() {
    assert_eq!(
      extract_params(Path::new("blog/[id].trs")),
      vec!["id"]
    );
  }

  #[test]
  fn multiple_params() {
    assert_eq!(
      extract_params(Path::new("[org]/[repo].trs")),
      vec!["org", "repo"]
    );
  }

  // ── Module segments ────────────────────────────────────────────────

  #[test]
  fn module_segments_flat() {
    assert_eq!(
      compute_module_segments(Path::new("index.trs")),
      vec!["index"]
    );
  }

  #[test]
  fn module_segments_nested() {
    assert_eq!(
      compute_module_segments(Path::new("blog/post.trs")),
      vec!["blog", "post"]
    );
  }

  #[test]
  fn module_segments_deeply_nested() {
    assert_eq!(
      compute_module_segments(Path::new("docs/api/v2/reference.trs")),
      vec!["docs", "api", "v2", "reference"]
    );
  }

  #[test]
  fn module_segments_dynamic() {
    assert_eq!(
      compute_module_segments(Path::new("blog/[id].trs")),
      vec!["blog", "_id"]
    );
  }

  #[test]
  fn module_segments_layout() {
    assert_eq!(
      compute_module_segments(Path::new("_layout.trs")),
      vec!["_layout"]
    );
  }

  #[test]
  fn module_segments_error() {
    assert_eq!(
      compute_module_segments(Path::new("_error.trs")),
      vec!["_error"]
    );
  }

  // ── Classification ─────────────────────────────────────────────────

  #[test]
  fn classify_page() {
    assert_eq!(classify(Path::new("index.trs"), false), RouteKind::Page);
  }

  #[test]
  fn classify_layout() {
    assert_eq!(
      classify(Path::new("_layout.trs"), false),
      RouteKind::Layout
    );
  }

  #[test]
  fn classify_error() {
    assert_eq!(
      classify(Path::new("_error.trs"), false),
      RouteKind::Error
    );
  }

  #[test]
  fn classify_component() {
    assert_eq!(
      classify(Path::new("Counter.trs"), true),
      RouteKind::Component
    );
  }

  #[test]
  fn classify_dynamic_page() {
    assert_eq!(
      classify(Path::new("blog/[id].trs"), false),
      RouteKind::Page
    );
  }
}
