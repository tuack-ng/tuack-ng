use colored::ColoredString;
use indexmap::IndexMap;

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
    pub messages: Vec<LoadMessage>,
    pub sub: IndexMap<String, LoadMessages>,
}

impl LoadMessages {
    /// 创建空的消息集合
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            sub: IndexMap::new(),
        }
    }

    /// 创建包含单条消息的集合
    pub fn from_message(message: LoadMessage) -> Self {
        let mut messages = Self::new();
        messages.messages.push(message);
        messages
    }

    /// 创建包含多条消息的集合
    pub fn from_messages(messages: Vec<LoadMessage>) -> Self {
        Self {
            messages,
            sub: IndexMap::new(),
        }
    }

    /// 添加消息到当前层级
    pub fn push(&mut self, message: LoadMessage) {
        self.messages.push(message);
    }

    /// 添加子层级
    pub fn sub<K: Into<String>>(&mut self, key: K) -> &mut LoadMessages {
        self.sub.entry(key.into()).or_insert_with(LoadMessages::new)
    }

    /// 检查是否为空（无消息且无子层级）
    pub fn is_empty(&self) -> bool {
        // TODO: 不要 DFS
        self.messages.is_empty() && self.sub.values().all(|v| v.is_empty())
    }
}

#[derive(Clone, Debug, Default)]
pub struct LoadContext {
    pub root: LoadMessages,
    pub current_path: Vec<String>,
    pub migrated: bool,
}

impl LoadContext {
    fn current(&mut self) -> &mut LoadMessages {
        let mut node = &mut self.root;
        for key in &self.current_path {
            node = node.sub(key);
        }
        node
    }

    pub fn new() -> Self {
        Self {
            root: LoadMessages::new(),
            current_path: Vec::new(),
            migrated: false,
        }
    }

    pub fn enter(&mut self, sub: String) -> () {
        self.current_path.push(sub);
    }

    pub fn ret(&mut self) -> () {
        self.current_path.pop();
    }

    pub fn emit_note(&mut self, message: impl Into<ColoredString>) -> () {
        self.current().push(LoadMessage::note(message));
    }

    pub fn emit_warn(&mut self, message: impl Into<ColoredString>) -> () {
        self.current().push(LoadMessage::warn(message));
    }

    pub fn emit_error(&mut self, message: impl Into<ColoredString>) -> () {
        self.current().push(LoadMessage::error(message));
    }
}
