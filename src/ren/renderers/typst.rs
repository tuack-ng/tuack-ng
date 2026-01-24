use crate::config::ProblemConfig;
use crate::ren::RenderQueue;
use crate::ren::renderers::base::Checker;
use crate::ren::renderers::base::Compiler;
use log::debug;
use log::error;
use log::info;
use log::warn;
use markdown_ppp::ast::Document;
use markdown_ppp::typst_printer::config::Config;
use markdown_ppp::typst_printer::render_typst;
use serde_json;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{
    ContestConfig, ContestDayConfig, DataJson, DateInfo, Problem, SupportLanguage, TemplateManifest,
};
pub struct TypstChecker {
    pub template_dir: PathBuf,
}

impl Checker for TypstChecker {
    fn new(template_dir: PathBuf) -> Self {
        TypstChecker { template_dir }
    }

    fn check_compiler(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("检查Typst编译环境");
        let typst_check = Command::new("typst").arg("--version").output();

        match typst_check {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    debug!("Typst 版本: {}", version.trim());
                } else {
                    error!("Typst 命令执行失败，请检查是否已安装");
                    return Err("Typst 命令执行失败，请检查是否已安装".into());
                }
            }
            Err(_) => {
                return Err("未找到 typst 命令，请确保已安装并添加到PATH".into());
            }
        }

        let template_required_files = ["main.typ", "utils.typ"];
        for file in template_required_files {
            if !self.template_dir.join(file).exists() {
                error!("模板缺少必要文件: {}", file);
                return Err(format!("模板缺少必要文件: {}", file).into());
            }
            info!("文件存在: {}", file);
        }
        Ok(())
    }
}

pub struct TypstCompiler {
    pub contest_config: ContestConfig,
    pub day_config: ContestDayConfig,
    pub tmp_dir: PathBuf,
    pub renderqueue: Vec<RenderQueue>,
    pub manifest: TemplateManifest,
}

impl Compiler for TypstCompiler {
    fn new(
        contest_config: ContestConfig,
        day_config: ContestDayConfig,
        tmp_dir: PathBuf,
        renderqueue: Vec<RenderQueue>,
        manifest: TemplateManifest,
    ) -> Self {
        TypstCompiler {
            contest_config,
            day_config,
            tmp_dir,
            renderqueue,
            manifest,
        }
    }
    fn compile(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.generate_conf(&self.day_config, &self.tmp_dir)?;
        let mut render_idx: usize = 0;
        for item in &self.renderqueue {
            match item {
                RenderQueue::Problem(ast, config) => {
                    self.convert_ast(config, &self.tmp_dir, ast, render_idx)?;
                    render_idx += 1;
                }
                RenderQueue::Precaution(ast) => {
                    self.convert_ast_precaution(&self.tmp_dir, ast)?;
                }
            }
        }
        fs::create_dir(self.tmp_dir.join("output"))?;
        let output_filename = format!("output/{}.pdf", self.day_config.name);
        let output = Command::new("typst")
            .arg("compile")
            .arg("--font-path=fonts")
            .arg("main.typ")
            .arg(&output_filename)
            .current_dir(&self.tmp_dir)
            .output()?;
        if output.status.success() {
            info!("Typst 编译成功: {}", output_filename);
            Ok(self.tmp_dir.join(output_filename))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Typst 编译失败: {}", stderr);
            Err("Typst 编译失败".into())
        }
    }
}
impl TypstCompiler {
    fn generate_conf(
        &self,
        day_config: &ContestDayConfig,
        tmp_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 构建问题列表
        let mut problems = Vec::new();

        for (_name, problem_config) in &day_config.subconfig {
            let mut submit_filenames = Vec::new();

            // 遍历 day_config.compile 中的语言配置来生成对应的提交文件名
            for lang_key in day_config.compile.keys() {
                submit_filenames.push(format!("{}.{}", problem_config.name, lang_key));
            }

            let point_equal = if problem_config.data.is_empty() {
                // 如果没有测试数据，默认为"是"
                "是".to_string()
            } else {
                // 获取第一个测试点的分数
                let first_score = problem_config.data[0].score;
                // 检查所有测试点的分数是否都等于第一个测试点的分数
                let all_equal = problem_config
                    .data
                    .iter()
                    .all(|data_item| data_item.score == first_score);

                if all_equal {
                    "是".to_string()
                } else {
                    "否".to_string()
                }
            };

            let problem = Problem {
                name: problem_config.name.clone(),
                title: problem_config.title.clone(),
                dir: problem_config.name.clone(), // 假设目录名就是问题名
                exec: problem_config.name.clone(), // 默认值，你可能需要从配置文件读取
                input: problem_config.name.clone() + ".in",
                output: problem_config.name.clone() + ".ans", // 改为使用 .ans 后缀
                problem_type: match problem_config.problem_type.as_str() {
                    "program" => "传统型",
                    "output" => "提交答案型",
                    "interactive" => "交互型",
                    _ => {
                        warn!(
                            "未知的题目类型 {} , 使用默认值: 传统型",
                            problem_config.problem_type
                        );
                        "传统型"
                    }
                }
                .to_string(),
                time_limit: format!("{:.1} 秒", problem_config.time_limit),
                memory_limit: problem_config.memory_limit.clone(),
                testcase: problem_config.data.len().to_string(),
                point_equal,
                submit_filename: submit_filenames,
            };
            problems.push(problem);
        }

        // 构建支持的语言列表
        let context = crate::context::get_context();
        let mut support_languages = Vec::new();

        for (lang_key, compile_options) in &day_config.compile {
            // 从context中查找对应的语言配置来获取语言名称
            let language_name = if let Some(lang_config) = context.languages.get(lang_key) {
                lang_config.language.clone()
            } else {
                // 如果context中没有对应的语言配置，使用键名作为语言名称
                error!("在语言配置中未找到 {}", lang_key);
                return Err(format!("在语言配置中未找到 {}", lang_key).into());
            };

            let language = SupportLanguage {
                name: language_name,
                compile_options: compile_options.clone(),
            };
            support_languages.push(language);
        }

        // 创建日期信息
        let date = DateInfo {
            start: day_config.start_time,
            end: day_config.end_time,
        };

        // 从ContestConfig和ContestDayConfig中获取覆盖值
        let use_pretest = day_config
            .use_pretest
            .or(self.contest_config.use_pretest)
            .unwrap_or(self.manifest.use_pretest);
        let noi_style = day_config
            .noi_style
            .or(self.contest_config.noi_style)
            .unwrap_or(self.manifest.noi_style);
        let file_io = day_config
            .file_io
            .or(self.contest_config.file_io)
            .unwrap_or(self.manifest.file_io);
        let data_json = DataJson {
            title: self.contest_config.title.clone(),
            subtitle: self.contest_config.short_title.clone(),
            dayname: day_config.title.clone(),
            date,
            use_pretest,
            noi_style,
            file_io,
            support_languages,
            problems,
        };

        let data_json_str = serde_json::to_string_pretty(&data_json)?;
        fs::write(tmp_dir.join("data.json"), data_json_str)?;
        info!("生成 data.json");

        Ok(())
    }
    pub fn convert_ast(
        &self,
        problem: &ProblemConfig,
        tmp_dir: &Path,
        ast: &Document,
        index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("生成Typst: {}", problem.name);
        let typst_output = render_typst(ast, Config::default().with_width(1000000));
        let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);

        let typst_filename = format!("problem-{}.typ", index);
        fs::write(tmp_dir.join(&typst_filename), typst_output)?;
        info!("生成: {}", typst_filename);
        Ok(())
    }
    pub fn convert_ast_precaution(
        &self,
        tmp_dir: &Path,
        ast: &Document,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("生成注意事项Typst...");
        let typst_output = render_typst(ast, Config::default().with_width(1000000));
        let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);
        fs::write(tmp_dir.join("precaution.typ"), typst_output)?;
        info!("生成: precaution.typ");
        Ok(())
    }
}
