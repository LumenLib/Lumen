use database::ccf_data::CCF_RANK_MAP;
use log::{debug, info, warn};
use std::collections::HashSet;

#[derive(Clone)]
pub struct CCFService;

impl Default for CCFService {
    fn default() -> Self {
        Self::new()
    }
}

impl CCFService {
    #[must_use]
    pub fn new() -> Self {
        info!("CCF管理: 初始化 CCF 管理器 (静态数据)");
        Self
    }

    /// 获取排名
    /// 使用 Sørensen–Dice 系数进行智能匹配，自动处理年份、前后缀和噪音。
    #[must_use]
    pub fn get_rank(&self, name: &str) -> Option<String> {
        let name = name.trim();
        debug!("CCF 匹配: 输入 '{name}'");
        if name.is_empty() {
            warn!("CCF 匹配: 输入为空");
            return None;
        }

        // 1. 快速路径：完全匹配 (归一化后)
        let normalized = Self::normalize(name);
        if let Some(rank) = CCF_RANK_MAP.get(&normalized) {
            debug!("CCF 精确匹配: '{name}' -> {rank}");
            return Some(rank.to_string());
        }

        // 2. 智能匹配：Sørensen–Dice Coefficient
        let input_tokens = Self::tokenize(name);
        if input_tokens.is_empty() {
            debug!("CCF 匹配: 归一化后分词为空 (input='{name}', normalized='{normalized}')");
            return None;
        }

        let mut best_score = 0.0;
        let mut best_rank: Option<&str> = None;

        // 阈值设为 0.6
        // 例如：A(5词) 和 B(5词) 有 3 词匹配 -> 2*3/10 = 0.6 (勉强匹配)
        // A(5词) 和 B(5词) 有 4 词匹配 -> 2*4/10 = 0.8 (良好匹配)
        const MATCH_THRESHOLD: f32 = 0.6;

        for (key, rank) in CCF_RANK_MAP.entries() {
            // 这里为了性能，实际项目中可以将 Key 的 Token 预处理缓存。
            // 但考虑到 CCF 库只有几百条，现代 CPU 毫秒级就能跑完，直接实时算也没问题。
            let key_tokens = Self::tokenize(key);

            if key_tokens.is_empty() {
                warn!("CCF 数据: Key '{key}' 分词结果为空，跳过");
                continue;
            }

            let score = Self::sorensen_dice(&input_tokens, &key_tokens);

            if score > best_score {
                best_score = score;
                best_rank = Some(rank);
            }
        }

        if best_score >= MATCH_THRESHOLD
            && let Some(rank) = best_rank
        {
            debug!("CCF Dice匹配: '{name}' -> Rank {rank} (Score: {best_score:.2})");
            return Some(rank.to_string());
        } else if best_score > 0.0 {
            debug!(
                "CCF Dice未达标: '{name}' -> best_score={best_score:.2} (threshold={MATCH_THRESHOLD})"
            );
        }

        // 3. 兜底策略：尝试提取第一个单词作为缩写 (仅当 Input 很长时才尝试，防止误判)
        // 例如 "CVPR 2024"
        let first_word = name.split_whitespace().next().unwrap_or("");
        if first_word.len() > 1 {
            // 忽略单个字母
            let norm_first = Self::normalize(first_word);
            if let Some(rank) = CCF_RANK_MAP.get(&norm_first) {
                debug!("CCF 缩写匹配: '{name}' -> '{first_word}' -> {rank}");
                return Some(rank.to_string());
            }
            debug!("CCF 缩写匹配失败: '{name}' -> first_word='{first_word}'");
        }

        debug!("CCF 未匹配: '{name}' (best_score={best_score:.2}, 策略已全部尝试)");
        None
    }

    /// Sørensen–Dice 系数计算
    /// Formula: 2 * |A ∩ B| / (|A| + |B|)
    fn sorensen_dice(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
        let intersect_count = a.intersection(b).count();
        let total_len = a.len() + b.len();

        if total_len == 0 {
            0.0
        } else {
            (2.0 * intersect_count as f32) / total_len as f32
        }
    }

    /// 分词与清洗
    /// 1. 转小写
    /// 2. 去除非字母数字
    /// 3. 去除停用词
    /// 4. 去除年份
    fn tokenize(text: &str) -> HashSet<String> {
        let stop_words = [
            "of",
            "the",
            "and",
            "in",
            "on",
            "for",
            "at",
            "to",
            "by",
            "with",
            "a",
            "an",
            "proc",
            "proceedings",
            "vol",
            "no",
            "pp",
            "pages",
        ];

        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric()) // 按非字母数字分割 (处理 "IEEE/CVF" -> "ieee", "cvf")
            .filter(|s| !s.is_empty())
            .filter(|s| !stop_words.contains(s))
            .filter(|s| !Self::is_year(s)) // 去除年份
            .map(std::string::ToString::to_string)
            .collect()
    }

    /// 判断是否是年份 (简单判断：4位数字，19xx或20xx)
    fn is_year(s: &str) -> bool {
        if s.len() != 4 {
            return false;
        }
        if let Ok(year) = s.parse::<i32>() {
            return (1900..=2100).contains(&year);
        }
        false
    }

    /// 归一化字符串: 分隔符 `-` `/` `&` → 空格，转小写，去除非字母数字，折叠空白
    fn normalize(s: &str) -> String {
        s.to_lowercase()
            .replace(['-', '/', '&'], " ")
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}
