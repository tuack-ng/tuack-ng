use std::collections::BTreeMap;

use tuack_config::{ProblemConfig, ScorePolicy as ConfigScorePolicy, SubtaskItem};
use tuack_lib::test::TestCaseResult;
use tuack_utils::data::FsTestData;

/// 组的得分
#[derive(Debug, Clone)]
pub struct GroupScore {
    pub earned: u32,
    pub full: u32,
}

/// 判分报告
#[derive(Debug, Clone)]
pub struct ScoreReport {
    /// 每组 (subtask) 的得分
    pub groups: BTreeMap<u32, GroupScore>,
    /// 总得分
    pub total: u32,
    /// 满分
    pub full_score: u32,
}

/// 判分策略，解释一组执行结果，并返回判分结果。
pub trait ScorePolicy {
    fn score(
        &self,
        config: &ProblemConfig,
        data_items: &[FsTestData],
        results: &[TestCaseResult],
    ) -> ScoreReport;
}

/// 正式数据判分：按配置计算
pub struct DataPolicy;

impl ScorePolicy for DataPolicy {
    fn score(
        &self,
        config: &ProblemConfig,
        data_items: &[FsTestData],
        results: &[TestCaseResult],
    ) -> ScoreReport {
        let case_scores = pair_scores(data_items, results);
        finish_score(&config.runtime.subtasks, &case_scores)
    }
}

/// 样例判分：加和计算
pub struct SamplePolicy;

impl ScorePolicy for SamplePolicy {
    fn score(
        &self,
        _config: &ProblemConfig,
        data_items: &[FsTestData],
        results: &[TestCaseResult],
    ) -> ScoreReport {
        let n = results.len();
        let groups = BTreeMap::from([(
            0u32,
            SubtaskItem {
                items: (0..n).collect(),
                max_score: n as u32,
                policy: ConfigScorePolicy::Sum,
            },
        )]);
        let case_scores = pair_scores(data_items, results);
        finish_score(&groups, &case_scores)
    }
}

/// 按位置配对 `data_items` 与 `results`,产出 `(subtask, earned)` 元组。
///
/// 单点得分 = 归一化比例 * 满分，四舍五入。
fn pair_scores(data_items: &[FsTestData], results: &[TestCaseResult]) -> Vec<(u32, u32)> {
    data_items
        .iter()
        .zip(results)
        .map(|(item, r)| {
            let earned = (r.score * item.full_score() as f64).round() as u32;
            (item.subtask(), earned)
        })
        .collect()
}

/// 按计分模型聚合各组的得分，产出报告。
/// `case_scores` 为 `(subtask, earned)` 元组。
fn finish_score(groups: &BTreeMap<u32, SubtaskItem>, case_scores: &[(u32, u32)]) -> ScoreReport {
    let mut by_group: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (subtask, earned) in case_scores {
        by_group.entry(*subtask).or_default().push(*earned);
    }

    let mut score_groups = BTreeMap::new();
    let mut total = 0;
    let mut full_score = 0;
    for (id, group) in groups {
        let scores = by_group.get(id).cloned().unwrap_or_default();
        let earned = match group.policy {
            ConfigScorePolicy::Sum => scores.iter().sum(),
            ConfigScorePolicy::Max => *scores.iter().max().unwrap_or(&0),
            ConfigScorePolicy::Min => *scores.iter().min().unwrap_or(&0),
        };
        total += earned;
        full_score += group.max_score;
        score_groups.insert(
            *id,
            GroupScore {
                earned,
                full: group.max_score,
            },
        );
    }

    ScoreReport {
        groups: score_groups,
        total,
        full_score,
    }
}
