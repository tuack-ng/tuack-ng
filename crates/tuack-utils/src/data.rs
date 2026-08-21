use std::io;
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs::File;

use crate::prelude::*;
use tuack_config::{DmkConfig, ExpandedDataItem, ExpandedSampleItem, ProblemConfig};
use tuack_lib::data::{AsyncReader, Data, DmkData};
use tuack_lib::utils::testlib::Arg;

/// 构造正式数据的 `FsTestData` 列表（从 `data/` 读取）。
pub fn problem_test_data(problem: &ProblemConfig) -> Vec<FsTestData<'_>> {
    problem
        .runtime
        .data
        .iter()
        .map(|item| FsTestData::from_data(problem.path.join("data"), item))
        .collect()
}

/// 构造样例数据的 `FsTestData` 列表（从 `sample/` 读取）。
pub fn problem_sample_data(problem: &ProblemConfig) -> Vec<FsTestData<'_>> {
    problem
        .runtime
        .samples
        .iter()
        .map(|item| FsTestData::from_sample(problem.path.join("sample"), item))
        .collect()
}

/// 被包装的数据来源引用
#[derive(Clone)]
enum TestItemRef<'a> {
    Data(&'a ExpandedDataItem),
    Sample(&'a ExpandedSampleItem),
}

/// 从文件系统读取的测试数据，持有基础目录与对配置项的引用。
#[derive(Clone)]
pub struct FsTestData<'a> {
    base_dir: PathBuf,
    item: TestItemRef<'a>,
}

impl<'a> FsTestData<'a> {
    pub fn from_data(base_dir: PathBuf, item: &'a ExpandedDataItem) -> Self {
        Self {
            base_dir,
            item: TestItemRef::Data(item),
        }
    }

    pub fn from_sample(base_dir: PathBuf, item: &'a ExpandedSampleItem) -> Self {
        Self {
            base_dir,
            item: TestItemRef::Sample(item),
        }
    }

    fn input_name(&self) -> &str {
        match &self.item {
            TestItemRef::Data(item) => &item.input,
            TestItemRef::Sample(item) => &item.input,
        }
    }

    fn answer_name(&self) -> &str {
        match &self.item {
            TestItemRef::Data(item) => &item.output,
            TestItemRef::Sample(item) => &item.output,
        }
    }

    /// 该测试点的生成行为。
    pub fn dmk(&self) -> DmkConfig {
        match &self.item {
            TestItemRef::Data(item) => item.dmk,
            TestItemRef::Sample(item) => item.dmk,
        }
    }

    /// 该测试点是否生成输入。
    pub fn gen_input(&self) -> bool {
        matches!(self.dmk(), DmkConfig::Input | DmkConfig::On)
    }

    /// 该测试点是否生成输出。
    pub fn gen_output(&self) -> bool {
        matches!(self.dmk(), DmkConfig::Output | DmkConfig::On)
    }

    /// 输入文件的完整路径。
    pub fn input_path(&self) -> PathBuf {
        self.base_dir.join(self.input_name())
    }

    /// 输出文件的完整路径。
    pub fn output_path(&self) -> PathBuf {
        self.base_dir.join(self.answer_name())
    }

    /// 该测试点的满分 (样例约定每点 1 分)。
    pub fn full_score(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.score,
            TestItemRef::Sample(_) => 1,
        }
    }

    /// 该测试点所属子任务 (样例约定为 0)。
    pub fn subtask(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.subtask,
            TestItemRef::Sample(_) => 0,
        }
    }

    /// 该测试点编号。
    pub fn id(&self) -> u32 {
        match &self.item {
            TestItemRef::Data(item) => item.id,
            TestItemRef::Sample(item) => item.id,
        }
    }
}

#[async_trait]
impl Data for FsTestData<'_> {
    async fn input(&self) -> io::Result<Box<dyn AsyncReader>> {
        let f = File::open(self.base_dir.join(self.input_name())).await?;
        Ok(Box::new(f))
    }

    async fn answer(&self) -> io::Result<Box<dyn AsyncReader>> {
        let f = File::open(self.base_dir.join(self.answer_name())).await?;
        Ok(Box::new(f))
    }
}

#[async_trait]
impl DmkData for FsTestData<'_> {
    fn args(&self) -> &IndexMap<String, Arg> {
        match &self.item {
            TestItemRef::Data(item) => &item.args,
            TestItemRef::Sample(item) => &item.args,
        }
    }

    async fn write_input(&self, mut input: Box<dyn AsyncReader>) -> Result<()> {
        let mut f = File::create(self.input_path()).await?;
        tokio::io::copy(&mut *input, &mut f).await?;
        Ok(())
    }

    async fn write_output(&self, mut output: Box<dyn AsyncReader>) -> Result<()> {
        let mut f = File::create(self.output_path()).await?;
        tokio::io::copy(&mut *output, &mut f).await?;
        Ok(())
    }
}
