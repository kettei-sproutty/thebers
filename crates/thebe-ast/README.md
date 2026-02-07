# thebe-ast

Parser and AST for `.trs` single-file components.

## Overview

`thebe-ast` parses `.trs` files into a typed AST that downstream compiler passes can consume. It handles the full syntax surface of Thebers templates, including HTML trees, interpolation, directives, control-flow, components, and slots.

## Features

### Parsing

- **Script blocks** — `<script setup>` (always Rust) and `<script lang="...">` (client-side)
- **Style blocks** — `<style>` and `<style scoped>`, with optional `lang` attribute
- **Template fragments** — everything outside script/style blocks, preserved in source order

### Template HTML Tree

- **Elements** — full HTML tag parsing with attributes, children, self-closing and void elements
- **Components** — uppercase tags (e.g. `<MyButton />`) as distinct `HtmlNode::Component` variants
- **Interpolation** — `{{ expr }}` in text and attribute values
- **Slots** — `<slot />` and `<slot name="header">fallback</slot>`
- **Control-flow** — `{#if}` / `{:else if}` / `{:else}` / `{/if}` and `{#each items as item, i}` / `{/each}`

### Directives

- `on:event` — event bindings with modifier support (`on:click|preventDefault|stopPropagation`)
- `bind:prop` — two-way bindings
- `class:name` — conditional CSS classes
- `style:prop` — inline style properties
- `use:action` — action / directive hooks

### Infrastructure

- **Visitor / walker** — `Visitor` trait with `walk_*` free functions for tree traversal
- **Diagnostics** — rich error messages via [ariadne](https://crates.io/crates/ariadne) with source snippets and colored output
- **Spans** — byte-offset spans on every node for precise error reporting

## Future Implementations

| Feature | Description | Priority |
|---------|-------------|----------|
| Spread attributes | `{...props}` on elements/components | Medium |
| `{@html expr}` | Raw HTML injection | Medium |
| `{@debug}` | Debug output helper | Low |
| `{#key}` block | Force re-render on value change | Medium |
| `{#await}` block | Async data loading (`{:then}` / `{:catch}`) | Medium |
| Shorthand attributes | `{name}` → `name="{{ name }}"` | Low |
| HTML comments | `<!-- comment -->` preservation | Low |
| `transition:` / `in:` / `out:` | Animation directive kinds | Low |
| `animate:` directive | FLIP animations in `{#each}` | Low |
| `let:` directive | Expose slot data upward | Low |

## Usage

```rust
use thebe_ast::{parse, parse_html, HtmlNode};

// Parse a full .trs file
let ast = parse(r#"
  <script setup>let x = 1;</script>
  <div>{{ x }}</div>
"#).unwrap();

// Parse standalone HTML template
let nodes = parse_html("<div><p>hello</p></div>", 0).unwrap();
```

## Visitor Example

```rust
use thebe_ast::visitor::{Visitor, walk_element};
use thebe_ast::Element;

struct TagCollector { tags: Vec<String> }

impl Visitor for TagCollector {
  fn visit_element(&mut self, el: &Element) {
    self.tags.push(el.tag.clone());
    walk_element(self, el); // recurse into children
  }
}
```
