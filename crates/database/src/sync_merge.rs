//! 同步合并结果类型（条件原语返回类型）
//!
//! `try_merge_remote_X` 只比较本地 `version`/`is_dirty` 与远程记录，返回本枚举，
//! **不写库、不 warn**；具体写库或上抛冲突由 services 层按 `MergeOutcome` 裁决。
//! 这是 database 瘦身（A2）的契约：database 保留"比较"与"原子写"原语，
//! 冲突决策归属 services（见 `services::sync::conflict`）。

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome<T> {
    /// 远程应覆盖本地（insert/replace）
    Applied,
    /// 版本一致且本地无修改，仅清脏标记
    UpToDate,
    /// 存在冲突，无法自动合并；携带远程记录供上层裁决
    Conflict(T),
}
