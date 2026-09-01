# tuack-ng-parser

English | [中文](./README.zh_CN.md)

Next-generation Markdown parser for [Tuack-NG](https://github.com/tuack-ng/tuack-ng), built on [rushdown](https://github.com/yuin/rushdown).

A neutral, self-owned AST with byte-precise spans on inline elements, table merge support, and three-way rendering (Markdown / Typst / HTML).

## Features

- **Precise spans**: leaf inline elements (Text/Html/Latex) cover their source exactly; every block element provides a precise start position
- **Table merging**: `<` → colspan, `^` → rowspan (ported from markdown-ppp `process_spans`)
- **Extended syntax**: fenced-div containers (`:::`), LaTeX math (`$..$` / `$$..$$`), footnotes (`[^label]`), image/link attributes (`{width=..}`)
- **Three-way rendering**: Markdown (idempotent roundtrip), Typst (handles merging), HTML (planned)
- **Visitor**: read-only AST traversal (`visit_block`/`visit_inline` with spans)
- **Transform**: in-place AST rewriting (`map_blocks` / `transform_image_urls` / `transform_link_urls`)

## Quick Start

```rust
use tuack_ng_parser::parse;
use tuack_ng_parser::printers::{render_markdown, render_typst};

let doc = parse("# Title\n\nBody *emphasis* and $x^2$.\n");

let md = render_markdown(&doc);
let typ = render_typst(&doc);
```

## Supported Syntax

| Category    | Syntax                             | Description                                                         |
| ----------- | ---------------------------------- | ------------------------------------------------------------------- |
| Paragraph   | `a\nb`                             | Soft break → SoftBreak; `a  \nb` / `a\\\nb` hard break → LineBreak  |
| Headings    | `# H`, `H\n===`                    | ATX 1–6, Setext 1/2                                                 |
| Lists       | `- a` / `1. a`                     | Unordered (`-`/`*`/`+`), ordered (always rendered from 1), nested   |
| Blockquotes | `> quote`                          | Nested supported                                                    |
| Code        | `` `code` ``, fenced block         | Inline, fenced/indented blocks                                      |
| Tables      | `\| a \| b \|`                     | Merging `<`/`^`, alignment                                          |
| Emphasis    | `*em*` `**strong**` `~~del~~`      | Strikethrough (GFM)                                                 |
| Links       | `[text](url)` `[ref]` `<https://>` | Inline / reference-style / autolink                                 |
| Images      | `![alt](url){width=..}`            | With attributes                                                     |
| LaTeX       | `$x$` `$$...$$`                    | Inline / block (own line only)                                      |
| Footnotes   | `[^label]` `[^label]: content`     | Reference + definition                                              |
| HTML        | `<div>` `<span>`                   | Block / inline raw                                                  |

### Containers

`:::kind` fenced-div blocks — nestable and parameterized. `kind` is taken from the
`class` attribute (`:::note` / `:::{.note}`); remaining attributes become params.

| Syntax                                                    | Description                                            |
| --------------------------------------------------------- | ------------------------------------------------------ |
| `:::note`                                                 | Plain container; `kind` from class                     |
| `:::figure{caption="caption"}`                            | Figure container; caption → Typst `#figure(caption:)`  |
| `:::align{right}` / `:::align{center}` / `:::align{left}` | Alignment container; → Typst `#align(...)`             |
| `:::a{key=val}`                                           | Key-value params                                       |
| `:::{aa, bb, b=c, c=d}`                                   | Mixed list: bare flags + key-value pairs               |
| Nested                                                    | Containers can be nested arbitrarily                   |

## Development

```sh
cargo build
cargo test
```

## Acknowledgements

- [rushdown](https://github.com/yuin/rushdown) — parsing foundation
- [markdown-ppp](https://github.com/johnlepikhin/markdown-ppp) — reference implementation for architecture and semantics

## License

MIT
