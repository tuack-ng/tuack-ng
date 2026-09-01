#heading(level: 1, [#"h1"])

#heading(level: 2, [#"h2"])

#heading(level: 3, [#"h3"])

#heading(level: 4, [#"h4"])

#heading(level: 5, [#"h5"])

#heading(level: 6, [#"h6"])

#par[#"paragraph 1"]

#par[#"paragraph 2"]

#par[#"paragraph"#linebreak()#"break"]

#par[#emph[#"emphasis"]#" "#emph[#"emphasis"]#" "#strong[#"strong"]#" "#strong[#"strong"]#" "#strike[#"delete"]]

#raw(block: true, lang: "python", "print(\"python code block\")\nprint(0.1 + 0.2)")

#par[#"This is "#raw("inline code")]

#quote(block: true)[#par[#"quote"]
#quote(block: true)[#par[#"quote in quote"]]]

#list(
  [#"item A"],
  [#"item B"],
  [#"item C"],
)

#enum(
  [#"item 1"],
  [#"item 2"],
  [#"item 3"],
)

#list(
  [#"item 1"],
  [#"item 2"],
)

#par[#link("https://noi.cn/")[#"NOI website"]]

#par[#"Inline "#box(image("img/3.png", alt: "img"))#" image"]

#figure(caption: [居中图片。在这里添加一些图片描述。])[#par[#box(image("img/2.jpg", alt: "1.jpg"))]]

#thematic-break

#figure[#par[#"caption 参数是可选的。"]
#par[#"文本也可以放进去。"]]

#par[#"小"#box(image("img/2.jpg", alt: "small", height: 4em))#box(image("img/2.jpg", alt: "small", width: 4em))#"图片"]

#par[#"支持的单位有 "#raw("pt")#", "#raw("mm")#", "#raw("cm")#", "#raw("in")#", "#raw("em")#" 和按页面比例的 "#raw("%")#"。"]

#par[#"简单链接："#link("https://luogu.com.cn")]

#thematic-break

#figure(table(
  columns: (4),
  align: (left + horizon, center + horizon, right + horizon, center + horizon),
  [#"我是左对齐"],  [#"我是居中对齐"],  [#"我是右侧对齐"],  [#"没有对齐默认居中"],
  [#"内容"],  [#"内容"],  [#"内容"],  [#"内容"],
))

#par[#"单元格合并"]

#figure(table(
  columns: (4),
  align: (center + horizon, center + horizon, center + horizon, center + horizon),
  [#"如下"],  [#"进行"],  [#"单元格"],  [#"合并"],
  [#"1"],  table.cell(rowspan: 3)[#mi(block: false, "\\le 10")],  table.cell(rowspan: 8)[#mi(block: false, "\\le 10")],  [#"无"],
  [#"2"],  [#"无"],
  [#"3"],  [#"无"],
  [#"4"],  table.cell(rowspan: 6)[#mi(block: false, "\\le 3\\times 10^5")],  [#"无"],
  [#"5"],  [#"无"],
  [#"6"],  [#"无"],
  [#"7"],  [#"无"],
  [#"8"],  [#"无"],
  [#"9"],  table.cell(colspan: 2)[#"跨列合并 1"],
  [#"10"],  table.cell(colspan: 2, rowspan: 2)[#"大格子"],  [#"无"],
  [#"11"],  [#"无"],
))

#par[#"inline latex "#mi(block: false, "a^2 + b^2 = c^2")]

#mi(block: true, "\\sum_{i=1}^n i = \\frac{n(n+1)}{2}\n")
