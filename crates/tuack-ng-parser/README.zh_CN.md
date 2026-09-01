# tuack-ng-parser

[English](./README.md) | 中文

[Tuack-NG](https://github.com/tuack-ng/tuack-ng) 的下一代 Markdown 解析器，基于 [rushdown](https://github.com/yuin/rushdown)。

自建中立 AST、精准到 inline 元素的 span、支持表格合并、三路渲染（Markdown / Typst / HTML）。

## 特性

- **精准 span**：叶子行内元素（Text/Html/Latex）精确覆盖源码内容；所有块元素提供精确的 start 定位
- **表格合并**：`<` → colspan、`^` → rowspan（移植自 markdown-ppp `process_spans`）
- **扩展语法**：fenced-div 容器（`:::`）、LaTeX 公式（`$..$` / `$$..$$`）、脚注（`[^label]`）、图片/链接属性（`{width=..}`）
- **三路渲染**：Markdown（幂等往返）、Typst（处理合并）、HTML（待实现）
- **Visitor**：只读遍历 AST（`visit_block`/`visit_inline` 携带 span）
- **Transform**：就地改写 AST（`map_blocks` / `transform_image_urls` / `transform_link_urls`）

## 快速开始

```rust
use tuack_ng_parser::parse;
use tuack_ng_parser::printers::{render_markdown, render_typst};

let doc = parse("# 标题\n\n正文 *强调* 和 $x^2$。\n");

let md = render_markdown(&doc);
let typ = render_typst(&doc);
```

## 支持的语法

| 类别        | 语法                               | 说明                                                         |
| ----------- | ---------------------------------- | ------------------------------------------------------------ |
| 段落 / 换行 | `a\nb`                             | 软换行 → SoftBreak；`a  \nb` / `a\\\nb` 硬换行 → LineBreak   |
| 标题        | `# H`、`H\n===`                    | ATX 1–6 级、Setext 1/2 级                                    |
| 列表        | `- a` / `1. a`                     | 无序（`-`/`*`/`+`）、有序（渲染恒从 1 开始）、嵌套           |
| 引用        | `> quote`                          | 支持嵌套                                                     |
| 代码        | `` `code` ``、行间块               | 行内代码、围栏/缩进代码块                                    |
| 表格        | `\| a \| b \|`                     | 合并 `<`/`^`、对齐                                           |
| 强调        | `*em*` `**strong**` `~~del~~`      | 删除线（GFM）                                                |
| 链接        | `[text](url)` `[ref]` `<https://>` | 内联 / 引用式 / autolink                                     |
| 图片        | `![alt](url){width=..}`            | 支持属性                                                     |
| LaTeX       | `$x$` `$$...$$`                    | 行内 / 块级（独占行）                                        |
| 脚注        | `[^label]` `[^label]: 内容`        | 引用 + 定义                                                  |
| HTML        | `<div>` `<span>`                   | 块级 / 行内 raw                                              |

### 容器

`:::kind` fenced-div 块，支持嵌套与参数。`kind` 取自 `class` 属性（`:::note` / `:::{.note}`），其余属性作为参数。

| 语法                                                      | 说明                                           |
| --------------------------------------------------------- | ---------------------------------------------- |
| `:::note`                                                 | 普通容器，kind 取自 class                      |
| `:::figure{caption="标题"}`                               | 图片容器；caption → Typst `#figure(caption:)`  |
| `:::align{right}` / `:::align{center}` / `:::align{left}` | 对齐容器；→ Typst `#align(...)`                |
| `:::a{key=val}`                                           | 键值对参数                                     |
| `:::{aa, bb, b=c, c=d}`                                   | 混合列表：裸属性（Flag）+ 键值对（KeyValue）   |
| 嵌套                                                      | 容器可任意嵌套                                 |

## 开发

```sh
cargo build
cargo test
```

## 致谢

- [rushdown](https://github.com/yuin/rushdown) — 解析器基础
- [markdown-ppp](https://github.com/johnlepikhin/markdown-ppp) — 架构与语义对齐的参考实现

## License

MIT
