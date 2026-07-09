#![allow(unused)]
use crate::prelude::*;
use colored::ColoredString;
use debug_tree::{TreeBuilder, TreeConfig, TreeSymbols};

#[derive(Clone, Debug, Copy)]
pub enum LoadMessageLevel {
    Warn,
    Error,
    Note,
}

#[derive(Clone, Debug)]
pub struct LoadMessage {
    pub level: LoadMessageLevel,
    pub message: ColoredString,
}

impl LoadMessage {
    // 创建警告消息
    pub fn warn(message: impl Into<ColoredString>) -> Self {
        Self {
            level: LoadMessageLevel::Warn,
            message: message.into(),
        }
    }

    // 创建错误消息
    pub fn error(message: impl Into<ColoredString>) -> Self {
        Self {
            level: LoadMessageLevel::Error,
            message: message.into(),
        }
    }

    // 创建提示消息
    pub fn note(message: impl Into<ColoredString>) -> Self {
        Self {
            level: LoadMessageLevel::Note,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LoadMessages {
    pub name: String,
    pub messages: Vec<LoadMessage>,
    pub sub: Vec<LoadMessages>,
}

impl LoadMessages {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            messages: Vec::new(),
            sub: Vec::new(),
        }
    }

    pub fn push(&mut self, message: LoadMessage) {
        self.messages.push(message);
    }

    pub fn count(&self) -> usize {
        self.messages.len() + self.sub.iter().map(|v| v.count()).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    pub fn render_tree(&self, tree: &mut TreeBuilder) {
        for leaf in &self.messages {
            let level_string = if colored::control::SHOULD_COLORIZE.should_colorize() {
                match leaf.level {
                    LoadMessageLevel::Warn => "*".yellow(),
                    LoadMessageLevel::Error => "*".red(),
                    LoadMessageLevel::Note => "*".blue(),
                }
                .bold()
            } else {
                match leaf.level {
                    LoadMessageLevel::Warn => "[W]",
                    LoadMessageLevel::Error => "[E]",
                    LoadMessageLevel::Note => "[N]",
                }
                .into()
            };

            tree.add_leaf(&format!("{level_string} {}", leaf.message));
        }

        for branch in &self.sub {
            if branch.is_empty() {
                continue;
            }
            let _branch = tree.add_branch(&branch.name);
            branch.render_tree(tree);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LoadContext {
    pub root: LoadMessages,
    current_path: Vec<usize>,
    pub migrated: bool,
    force_migrate: bool,
    pub migrated_notices: HashMap<i32, &'static str>,
}

impl LoadContext {
    fn current(&mut self) -> &mut LoadMessages {
        let mut node = &mut self.root;
        for &idx in &self.current_path {
            node = &mut node.sub[idx];
        }
        node
    }

    pub fn new() -> Self {
        Self {
            root: LoadMessages::new(),
            current_path: Vec::new(),
            migrated: false,
            force_migrate: false,
            migrated_notices: HashMap::new(),
        }
    }

    pub fn new_force_migrate() -> Self {
        Self {
            force_migrate: true,
            ..Self::new()
        }
    }

    pub fn force_migrate(&self) -> bool {
        self.force_migrate
    }

    pub fn enter(&mut self) {
        let idx = {
            let node = self.current();
            node.sub.push(LoadMessages::new());
            node.sub.len() - 1
        };
        self.current_path.push(idx);
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.current().name = name.into();
    }

    pub fn ret(&mut self) {
        self.current_path.pop();
    }

    pub fn emit_note(&mut self, message: impl Into<ColoredString>) {
        self.current().push(LoadMessage::note(message));
    }

    pub fn emit_warn(&mut self, message: impl Into<ColoredString>) {
        self.current().push(LoadMessage::warn(message));
    }

    pub fn emit_error(&mut self, message: impl Into<ColoredString>) {
        self.current().push(LoadMessage::error(message));
    }

    pub fn render_tree(&self) -> String {
        let mut tree = TreeBuilder::new();
        tree.set_config_override(
            TreeConfig::new()
                .symbols(TreeSymbols::new().leaf("─ "))
                .indent(2),
        );
        self.root.render_tree(&mut tree);
        tree.string()
    }
}
