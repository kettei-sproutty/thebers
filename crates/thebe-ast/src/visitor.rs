//! AST visitor / walker infrastructure.
//!
//! The [`Visitor`] trait provides a hook for every node kind in the AST.
//! Each `visit_*` method has a default implementation that calls the
//! corresponding `walk_*` free function, which recurses into children.
//!
//! To use the visitor, implement the trait and override only the methods
//! you care about.  Call the matching `walk_*` function at the end of
//! your override if you still want the default recursion.
//!
//! # Example
//!
//! ```
//! use thebe_ast::visitor::{Visitor, walk_element};
//! use thebe_ast::Element;
//!
//! struct TagCollector { tags: Vec<String> }
//!
//! impl Visitor for TagCollector {
//!   fn visit_element(&mut self, el: &Element) {
//!     self.tags.push(el.tag.clone());
//!     walk_element(self, el); // recurse into children
//!   }
//! }
//! ```

use crate::Attribute;
use crate::Directive;
use crate::Element;
use crate::HtmlNode;
use crate::IfBranch;
use crate::ScriptBlock;
use crate::StyleBlock;
use crate::TemplateFragment;
use crate::TemplateNode;
use crate::ThebeAst;

/// A read-only visitor for the Thebers AST.
///
/// Every method has a default implementation that either recurses into
/// child nodes (via the corresponding `walk_*` function) or does nothing
/// for leaf nodes.  Override only the methods you need.
#[allow(unused_variables)]
pub trait Visitor {
  /// Visit the top-level AST. Default: [`walk_ast`].
  fn visit_ast(&mut self, ast: &ThebeAst) {
    walk_ast(self, ast);
  }

  /// Visit a `<script>` or `<script setup>` block. Leaf — no children.
  fn visit_script(&mut self, script: &ScriptBlock) {}

  /// Visit a `<style>` block. Leaf — no children.
  fn visit_style(&mut self, style: &StyleBlock) {}

  /// Visit a parsed template fragment. Default: [`walk_template_fragment`].
  fn visit_template_fragment(&mut self, frag: &TemplateFragment) {
    walk_template_fragment(self, frag);
  }

  /// Visit a single template node (text or `{{ expr }}`). Leaf.
  fn visit_template_node(&mut self, node: &TemplateNode) {}

  /// Visit an HTML node (element, text, expr, if, each, component, slot).
  /// Default: [`walk_html_node`].
  fn visit_html_node(&mut self, node: &HtmlNode) {
    walk_html_node(self, node);
  }

  /// Visit an element (called for both `Element` and `Component` variants).
  /// Default: [`walk_element`].
  fn visit_element(&mut self, element: &Element) {
    walk_element(self, element);
  }

  /// Visit a single branch of an `{#if}` / `{:else if}` / `{:else}` chain.
  /// Default: [`walk_if_branch`].
  fn visit_if_branch(&mut self, branch: &IfBranch) {
    walk_if_branch(self, branch);
  }

  /// Visit an attribute on an element. Default: [`walk_attribute`].
  fn visit_attribute(&mut self, attr: &Attribute) {
    walk_attribute(self, attr);
  }

  /// Visit a directive on an element. Leaf — no children.
  fn visit_directive(&mut self, directive: &Directive) {}
}

// ---------------------------------------------------------------------------
// Walk functions — handle recursion into child nodes
// ---------------------------------------------------------------------------

/// Walk the top-level AST: scripts, styles, then template fragments.
pub fn walk_ast<V: Visitor + ?Sized>(v: &mut V, ast: &ThebeAst) {
  if let Some(script) = &ast.script_setup {
    v.visit_script(script);
  }
  if let Some(script) = &ast.script {
    v.visit_script(script);
  }
  for style in &ast.styles {
    v.visit_style(style);
  }
  for frag in &ast.template {
    v.visit_template_fragment(frag);
  }
}

/// Walk a template fragment: visit each [`TemplateNode`].
pub fn walk_template_fragment<V: Visitor + ?Sized>(v: &mut V, frag: &TemplateFragment) {
  for node in &frag.nodes {
    v.visit_template_node(node);
  }
}

/// Walk an [`HtmlNode`], dispatching to the appropriate visitor method.
pub fn walk_html_node<V: Visitor + ?Sized>(v: &mut V, node: &HtmlNode) {
  match node {
    HtmlNode::Element(el) | HtmlNode::Component(el) => v.visit_element(el),
    HtmlNode::Text(_) | HtmlNode::Expr { .. } | HtmlNode::RawHtml { .. }
    | HtmlNode::Const { .. } | HtmlNode::Debug { .. } => {}
    HtmlNode::If { branches, .. } => {
      for branch in branches {
        v.visit_if_branch(branch);
      }
    }
    HtmlNode::Each { children, .. } | HtmlNode::Slot { children, .. }
    | HtmlNode::Head { children, .. } => {
      for child in children {
        v.visit_html_node(child);
      }
    }
  }
}

/// Walk an [`Element`]: attributes, directives, then children.
pub fn walk_element<V: Visitor + ?Sized>(v: &mut V, el: &Element) {
  for attr in &el.attributes {
    v.visit_attribute(attr);
  }
  for dir in &el.directives {
    v.visit_directive(dir);
  }
  for child in &el.children {
    v.visit_html_node(child);
  }
}

/// Walk an [`IfBranch`]: visit each child node.
pub fn walk_if_branch<V: Visitor + ?Sized>(v: &mut V, branch: &IfBranch) {
  for child in &branch.children {
    v.visit_html_node(child);
  }
}

/// Walk an [`Attribute`]: visit each value segment (text or expression).
pub fn walk_attribute<V: Visitor + ?Sized>(v: &mut V, attr: &Attribute) {
  for node in &attr.value {
    v.visit_template_node(node);
  }
}
