# Thebe - The flexible rust framework

Thebe is a (WIP) Rust framework for building component-based applications using `.trs` single-file components (inspired by Svelte's SFC approach, but for Rust).

## Crates

| Crate | Description |
|-------|-------------|
| [thebe-ast](./crates/thebe-ast) | Parser and AST for `.trs` single-file components |

## `.trs` File Format

A `.trs` file can contain:

```html
<script setup>
  // Build-time / server-side Rust logic
  let count = 0;
</script>

<script lang="js">
  // Client-side reactivity
  var x = 1;
</script>

<style scoped>
  .active { color: red; }
</style>

<div>
  <h1>{{ title }}</h1>
  <Button on:click|preventDefault="increment">
    Count: {{ count }}
  </Button>

  {#if show}
    <Modal />
  {:else}
    <slot name="fallback" />
  {/if}

  {#each items as item, i}
    <ListItem bind:value="item" />
  {/each}
</div>
```

## License

MIT

