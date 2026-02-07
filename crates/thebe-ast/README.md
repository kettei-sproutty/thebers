# Thebe AST Decisions
===================

This document records the agreed AST shape and parsing rules for `.trs` files.

AST Shape (MVP)
--------------
- `script_setup`: Optional, at most one `<script setup>...</script>` block.
- `script`: Optional, `<script>...</script>` block (reserved for later behavior changes).
- `style`: Optional, `<style>...</style>` block (behavior TBD).
- `template`: Zero or more fragments, preserving order and raw text for all non-script/style content.

Parsing Rules
-------------
- Non-contiguous template is allowed and preserved as ordered fragments.
- Only one `<script setup>` block is permitted; duplicates are an error.
- Malformed or mismatched tags are an error.
- Empty files are an error.
- Block contents are captured as raw strings (whitespace preserved).
