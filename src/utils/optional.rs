use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct Optional<T> {
    pub value: Option<T>,
    pub initialized: bool,
}

#[allow(unused)]
impl<T> Optional<T> {
    pub fn new() -> Self {
        Self {
            value: None,
            initialized: false,
        }
    }

    pub fn uninitialized() -> Self {
        Self::new()
    }

    pub fn initialized(value: T) -> Self {
        Self {
            value: Some(value),
            initialized: true,
        }
    }

    pub fn set(&mut self, val: T) {
        self.value = Some(val);
        self.initialized = true;
    }

    pub fn set_default(&mut self, val: T) {
        if !self.initialized {
            self.value = Some(val);
            self.initialized = true;
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn should_skip(&self) -> bool {
        !self.initialized
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.value.as_mut()
    }

    pub fn take(&mut self) -> Option<T> {
        let value = self.value.take();
        if value.is_some() {
            self.initialized = true; // 即使取出，也标记为已初始化
        }
        value
    }
}

impl<T> Default for Optional<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Serialize> Serialize for Optional<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 关键逻辑：只有当 initialized 为 true 时才序列化
        if self.initialized && self.value.is_some() {
            match &self.value {
                Some(val) => serializer.serialize_some(val),
                _ => unreachable!(),
            }
        } else {
            // 不应该被调用，因为外层应该用 skip_serializing_if 跳过
            unimplemented!()
        }
    }
}

// 实现 Deserialize
impl<'de, T> Deserialize<'de> for Optional<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::deserialize(deserializer)?;
        let initialized = value.is_some(); // 先检查是否 Some

        Ok(Optional { value, initialized })
    }
}
