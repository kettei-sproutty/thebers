use thebe_ast::visitor::walk_ast;
use thebe_ast::visitor::walk_element;
use thebe_ast::visitor::walk_html_node;
use thebe_ast::visitor::Visitor;
use thebe_ast::Attribute;
use thebe_ast::Directive;
use thebe_ast::Element;
use thebe_ast::HtmlNode;
use thebe_ast::IfBranch;
use thebe_ast::ScriptBlock;
use thebe_ast::StyleBlock;
use thebe_ast::TemplateFragment;
use thebe_ast::TemplateNode;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Counts every `visit_element` call (both Element and Component).
struct ElementCounter {
  count: usize,
}

impl Visitor for ElementCounter {
  fn visit_element(&mut self, el: &Element) {
    self.count += 1;
    walk_element(self, el);
  }
}

/// Collects all tag names encountered via `visit_element`.
struct TagCollector {
  tags: Vec<String>,
}

impl Visitor for TagCollector {
  fn visit_element(&mut self, el: &Element) {
    self.tags.push(el.tag.clone());
    walk_element(self, el);
  }
}

/// Collects all expression strings from both HTML `{{ expr }}` and
/// attribute interpolations.
struct ExprCollector {
  exprs: Vec<String>,
}

impl Visitor for ExprCollector {
  fn visit_template_node(&mut self, node: &TemplateNode) {
    if let TemplateNode::Expr { expr, .. } = node {
      self.exprs.push(expr.clone());
    }
  }

  fn visit_html_node(&mut self, node: &HtmlNode) {
    if let HtmlNode::Expr { expr, .. } = node {
      self.exprs.push(expr.clone());
    }
    walk_html_node(self, node);
  }
}

/// Counts all `HtmlNode` variants visited.
struct NodeCounter {
  elements: usize,
  components: usize,
  texts: usize,
  exprs: usize,
  ifs: usize,
  eachs: usize,
  slots: usize,
}

impl NodeCounter {
  fn new() -> Self {
    Self {
      elements: 0,
      components: 0,
      texts: 0,
      exprs: 0,
      ifs: 0,
      eachs: 0,
      slots: 0,
    }
  }
}

impl Visitor for NodeCounter {
  fn visit_html_node(&mut self, node: &HtmlNode) {
    match node {
      HtmlNode::Element(_) => self.elements += 1,
      HtmlNode::Component(_) => self.components += 1,
      HtmlNode::Text(_) => self.texts += 1,
      HtmlNode::Expr { .. } => self.exprs += 1,
      HtmlNode::If { .. } => self.ifs += 1,
      HtmlNode::Each { .. } => self.eachs += 1,
      HtmlNode::Slot { .. } => self.slots += 1,
      HtmlNode::RawHtml { .. } => self.exprs += 1,
    }
    walk_html_node(self, node);
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn count_elements_in_flat_html() {
  let nodes = thebe_ast::parse_html("<div><p>hi</p></div>", 0).unwrap();
  let mut counter = ElementCounter { count: 0 };
  for node in &nodes {
    counter.visit_html_node(node);
  }
  // div + p = 2
  assert_eq!(counter.count, 2);
}

#[test]
fn collect_tags_nested() {
  let nodes = thebe_ast::parse_html("<ul><li>a</li><li>b</li></ul>", 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.tags, vec!["ul", "li", "li"]);
}

#[test]
fn component_visited_as_element() {
  let nodes = thebe_ast::parse_html("<div><MyButton /></div>", 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  // visit_element is called for both Element and Component
  assert_eq!(collector.tags, vec!["div", "MyButton"]);
}

#[test]
fn count_all_node_types() {
  let src = r"<div>text<MyComponent />{{ expr }}{#if x}<slot />{/if}{#each items as i}<p />{/each}</div>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut counter = NodeCounter::new();
  for node in &nodes {
    counter.visit_html_node(node);
  }
  assert_eq!(counter.elements, 2); // div + p
  assert_eq!(counter.components, 1); // MyComponent
  assert_eq!(counter.texts, 1); // "text"
  assert_eq!(counter.exprs, 1); // {{ expr }}
  assert_eq!(counter.ifs, 1); // {#if x}
  assert_eq!(counter.eachs, 1); // {#each}
  assert_eq!(counter.slots, 1); // <slot />
}

#[test]
fn collect_html_expressions() {
  let src = r"<div>{{ a }}<p>{{ b }}</p></div>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = ExprCollector { exprs: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.exprs, vec!["a", "b"]);
}

#[test]
fn collect_attribute_interpolations() {
  let src = r#"<img src="{{ url }}" alt="{{ desc }}" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = ExprCollector { exprs: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.exprs, vec!["url", "desc"]);
}

#[test]
fn visitor_recurses_into_if_branches() {
  let src = r"{#if a}<p />{:else}<span />{/if}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.tags, vec!["p", "span"]);
}

#[test]
fn visitor_recurses_into_each_children() {
  let src = r"{#each items as item}<li>{{ item }}</li>{/each}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.tags, vec!["li"]);
}

#[test]
fn visitor_recurses_into_slot_fallback() {
  let src = "<slot><p>fallback</p></slot>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(collector.tags, vec!["p"]);
}

#[test]
fn walk_ast_visits_all_top_level_parts() {
  struct PartCounter {
    scripts: usize,
    styles: usize,
    fragments: usize,
  }

  impl Visitor for PartCounter {
    fn visit_script(&mut self, _script: &ScriptBlock) {
      self.scripts += 1;
    }
    fn visit_style(&mut self, _style: &StyleBlock) {
      self.styles += 1;
    }
    fn visit_template_fragment(&mut self, _frag: &TemplateFragment) {
      self.fragments += 1;
    }
  }

  let src = r#"<script setup>let x = 1;</script>
<script lang="js">var y = 2;</script>
<style>body {}</style>
<div>hello</div>"#;
  let ast = thebe_ast::parse(src).unwrap();
  let mut counter = PartCounter {
    scripts: 0,
    styles: 0,
    fragments: 0,
  };
  walk_ast(&mut counter, &ast);

  assert_eq!(counter.scripts, 2); // script_setup + script
  assert_eq!(counter.styles, 1);
  assert!(counter.fragments > 0);
}

#[test]
fn default_visitor_does_nothing() {
  // A visitor with no overrides should compile and traverse without panicking.
  struct NoOp;
  impl Visitor for NoOp {}

  let nodes = thebe_ast::parse_html("<div><p>text</p></div>", 0).unwrap();
  let mut v = NoOp;
  for node in &nodes {
    v.visit_html_node(node);
  }
}

#[test]
fn deeply_nested_traversal() {
  let src = r"<div><section><article><Card><p>deep</p></Card></article></section></div>";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut collector = TagCollector { tags: Vec::new() };
  for node in &nodes {
    collector.visit_html_node(node);
  }
  assert_eq!(
    collector.tags,
    vec!["div", "section", "article", "Card", "p"]
  );
}

#[test]
fn directive_visited() {
  struct DirCounter {
    count: usize,
  }

  impl Visitor for DirCounter {
    fn visit_directive(&mut self, _dir: &Directive) {
      self.count += 1;
    }
  }

  let src = r#"<button on:click="go" bind:value="x">ok</button>"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut counter = DirCounter { count: 0 };
  for node in &nodes {
    counter.visit_html_node(node);
  }
  assert_eq!(counter.count, 2);
}

#[test]
fn attribute_visited() {
  struct AttrCounter {
    count: usize,
  }

  impl Visitor for AttrCounter {
    fn visit_attribute(&mut self, _attr: &Attribute) {
      self.count += 1;
    }
  }

  let src = r#"<img src="a.png" alt="pic" />"#;
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut counter = AttrCounter { count: 0 };
  for node in &nodes {
    counter.visit_html_node(node);
  }
  assert_eq!(counter.count, 2);
}

#[test]
fn if_branch_visited() {
  struct BranchCounter {
    count: usize,
  }

  impl Visitor for BranchCounter {
    fn visit_if_branch(&mut self, _branch: &IfBranch) {
      self.count += 1;
    }
  }

  let src = r"{#if a}x{:else if b}y{:else}z{/if}";
  let nodes = thebe_ast::parse_html(src, 0).unwrap();
  let mut counter = BranchCounter { count: 0 };
  for node in &nodes {
    counter.visit_html_node(node);
  }
  assert_eq!(counter.count, 3); // if + else if + else
}
