//! 自写扩展：fenced-div、link-attribute 与 latex。

pub mod fenced_div;
pub mod footnote;
pub mod latex;
pub mod link_attribute;

use rushdown::parser::ParserExtension;

/// 默认启用的扩展集合。
///
/// 不使用 `gfm()` 全家桶，而是分别注册需要的 GFM 扩展：
/// 任务列表（`gfm_task_list_item`）不在支持范围内，故不注册。
pub fn default_extensions() -> impl ParserExtension {
    rushdown::parser::gfm_table()
        .and(rushdown::parser::gfm_linkify(
            rushdown::parser::GfmOptions::default().linkify,
        ))
        .and(rushdown::parser::gfm_strikethrough())
        .and(fenced_div::fenced_div_parser_extension())
        .and(link_attribute::link_attribute_parser_extension())
        .and(latex::latex_parser_extension())
        .and(latex::latex_block_parser_extension())
        .and(footnote::footnote_parser_extension())
}
