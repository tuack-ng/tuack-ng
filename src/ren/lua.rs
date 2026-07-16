use crate::prelude::*;

use super::tools;

use mlua::{AnyUserData, Function, Lua, LuaSerdeExt, Table, UserData, Value};

/*
-- 配置
tng.config.contest
tng.config.day
tng.config.problem
tng.config.sample_cases
tng.config.data_cases

-- 与 jinja 一致的
tng.tools.int_lg
tng.tools.comma
tng.tools.hn
tng.tools.cases

-- md wrapper
tng.tools.italic
tng.tools.bold
tng.tools.strikethrough
tng.tools.inline_code
tng.tools.link(text, url)
tng.tools.autolink
tng.tools.inline_latex

-- 构建 table 对象
tng.table

*/

// 提取为独立函数
fn map_table(lua: &Lua, tbl: Table, func: Function) -> mlua::Result<Table> {
    let result = lua.create_table()?;

    for (index, item) in tbl.sequence_values::<Value>().enumerate() {
        let item = item?;
        let mapped_value: Value = func.call(item)?;
        result.set(index + 1, mapped_value)?;
    }

    Ok(result)
}

fn build_config(
    lua: &mut Lua,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
) -> mlua::Result<mlua::Table> {
    lua.create_table_from(vec![
        ("contest", lua.to_value(contest)?),
        ("day", lua.to_value(day)?),
        ("problem", lua.to_value(problem)?),
        ("sample_cases", {
            let samples = lua
                .to_value(&problem.samples)?
                .as_table()
                .unwrap()
                .to_owned();
            samples.set(
                "map",
                lua.create_function(|lua, (tbl, func): (Table, Function)| {
                    map_table(lua, tbl, func)
                })?,
            )?;

            Value::Table(samples)
        }),
        ("data_cases", {
            let data_cases = lua
                .to_value(&problem.orig_data)?
                .as_table()
                .unwrap()
                .to_owned();
            data_cases.set(
                "map",
                lua.create_function(|lua, (tbl, func): (Table, Function)| {
                    map_table(lua, tbl, func)
                })?,
            )?;

            Value::Table(data_cases)
        }),
    ])
}

fn build_tools(lua: &mut Lua) -> mlua::Result<mlua::Table> {
    lua.create_table_from(vec![
        (
            "int_lg",
            lua.create_function(|_, num: f64| Ok(tools::int_lg(num)))?,
        ),
        (
            "comma",
            lua.create_function(|_, num: i64| Ok(tools::comma(num)))?,
        ),
        (
            "hn",
            lua.create_function(|_, (num, style): (f64, Option<String>)| {
                Ok(tools::hn(num, style.as_deref()))
            })?,
        ),
        (
            "cases",
            lua.create_function(|_, value: mlua::Value| {
                let items: Vec<i32> = match value {
                    mlua::Value::Integer(i) => vec![i as i32],
                    mlua::Value::Number(n) => vec![n as i32],
                    mlua::Value::Table(t) => {
                        t.sequence_values::<i32>().collect::<mlua::Result<_>>()?
                    }
                    other => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "expected number or table, got {}",
                            other.type_name()
                        )));
                    }
                };
                Ok(tools::cases(&items))
            })?,
        ),
        // MarkDown 辅助
        (
            "italic",
            lua.create_function(|_, text: String| Ok(format!("*{}*", text)))?,
        ),
        (
            "bold",
            lua.create_function(|_, text: String| Ok(format!("**{}**", text)))?,
        ),
        (
            "strikethrough",
            lua.create_function(|_, text: String| Ok(format!("~~{}~~", text)))?,
        ),
        (
            "inline_code",
            lua.create_function(|_, text: String| Ok(format!("`{}`", text)))?,
        ),
        (
            "link",
            lua.create_function(|_, (text, url): (String, String)| {
                Ok(format!("[{}]({})", text, url))
            })?,
        ),
        (
            "autolink",
            lua.create_function(|_, url: String| Ok(format!("<{}>", url)))?,
        ),
        (
            "inline_latex",
            lua.create_function(|_, formula: String| Ok(format!("${}$", formula)))?,
        ),
    ])
}

#[derive(Clone)]
struct TngTable {
    pub headers: Vec<String>,
    pub align: Vec<AlignRule>,
    pub item: Vec<Vec<String>>,
}

impl UserData for TngTable {}

impl TngTable {
    fn to_markdown(&self) -> String {
        let mut out = String::new();

        out.push('|');
        for h in &self.headers {
            out.push_str(&format!(" {} |", h));
        }
        out.push('\n');

        out.push('|');
        for a in &self.align {
            out.push(' ');
            match a {
                AlignRule::Default => out.push_str("---"),
                AlignRule::Center => out.push_str(":---:"),
                AlignRule::Left => out.push_str(":---"),
                AlignRule::Right => out.push_str("---:"),
            }
            out.push_str(" |");
        }
        out.push('\n');

        for r in 0..self.item.len() {
            out.push('|');
            for c in 0..self.item[r].len() {
                out.push(' ');
                out.push_str(&self.item[r][c]);
                out.push_str(" |");
            }
            out.push('\n');
        }

        out
    }
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum MergeCol {
    Single(usize),
    Multiple(Vec<usize>),
}

#[derive(Clone, Deserialize)]
struct MergeRule {
    pub col: MergeCol,
    pub merge_row: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AlignRule {
    Default,
    Center,
    Left,
    Right,
}

fn apply_merge_rules(data: &[Vec<String>], rules: &[MergeRule]) -> Vec<Vec<String>> {
    if data.is_empty() || rules.is_empty() {
        return data.to_vec();
    }

    let rows = data.len();
    let col_len = data[0].len();
    let mut result = data.to_vec();

    // 收集需要合并的列索引（1-indexed → 0-indexed）
    let mut merge_cols = Vec::new();
    for rule in rules {
        if rule.merge_row {
            match &rule.col {
                MergeCol::Single(col) => {
                    if *col >= 1 && *col <= col_len {
                        merge_cols.push(*col - 1);
                    }
                }
                MergeCol::Multiple(cols) => {
                    for &col in cols {
                        if col >= 1 && col <= col_len {
                            merge_cols.push(col - 1);
                        }
                    }
                }
            }
        }
    }

    merge_cols.sort();
    merge_cols.dedup();

    if merge_cols.is_empty() {
        return result;
    }

    // 对每一列单独处理合并
    for &col in &merge_cols {
        for i in (1..rows).rev() {
            if data[i][col] == data[i - 1][col] {
                result[i][col] = "^".to_string();
            }
        }
    }

    result
}

fn create_tng_table(lua: &Lua, tbl: Table) -> Result<TngTable> {
    let headers: Vec<String> = tbl.get("headers")?;
    let align: Vec<AlignRule> = lua.from_value(tbl.get("align")?)?;
    let data: Vec<Vec<String>> = tbl.get("data")?;
    let merge_rules: Option<Vec<MergeRule>> = lua.from_value(tbl.get("merge_rules")?)?;

    let len = headers.len();

    if align.len() != len {
        bail!("`align` 列数与标题不符");
    }

    for (idx, col) in data.iter().enumerate() {
        if col.len() != len {
            bail!("表格第 {} 行列数与标题不符", idx + 1);
        }
    }

    let item = apply_merge_rules(&data, &merge_rules.unwrap_or_default());

    Ok(TngTable {
        headers,
        align,
        item,
    })
}

fn build_namespace(
    lua: &mut Lua,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
) -> Result<()> {
    let tng_config = build_config(lua, problem, day, contest)?;
    let tng_tools = build_tools(lua)?;

    lua.globals().set(
        "tng",
        lua.create_table_from(vec![
            // tng.config
            ("config", Value::Table(tng_config)),
            // tng.tools
            ("tools", Value::Table(tng_tools)),
            // tng.table
            (
                "table",
                Value::Function(
                    lua.create_function(|lua, tbl: Table| Ok(create_tng_table(lua, tbl)?))?,
                ),
            ),
        ])?,
    )?;

    Ok(())
}
pub fn render_template(
    path: &Path,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
) -> Result<String> {
    let mut lua = Lua::new();

    let source = fs::read(path)?;

    build_namespace(&mut lua, problem, day, contest)?;

    let table: TngTable = lua.load(source).eval::<AnyUserData>()?.take()?;

    Ok(table.to_markdown())
}
