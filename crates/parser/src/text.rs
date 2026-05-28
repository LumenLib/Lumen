//! 文本处理工具
//!
//! 提供通用的文本清理和格式化功能

use log::debug;

/// 清理文本内容，移除换行符、控制字符和多余空格
///
/// 这个函数主要用于清理从外部API获取的文本内容，确保它们适合在UI中显示
///
/// # 参数
/// - `text`: 需要清理的原始文本
///
/// # 返回值
/// 清理后的文本，已移除换行符和控制字符，并压缩了多余空格
#[must_use]
pub fn clean_text_content(text: &str) -> String {
    // 替换各种换行符为空格
    let cleaned = text
        .replace(['\n', '\r', '\t'], " ")
        .chars()
        // 过滤控制字符（ASCII码小于32的字符，除了空格、制表符、换行符等）
        .filter(|c| !c.is_control())
        .collect::<String>();

    // 压缩多个连续空格为单个空格
    let mut result = String::new();
    let mut prev_char = ' ';

    for c in cleaned.chars() {
        if c == ' ' && prev_char == ' ' {
            // 跳过连续的空格
            continue;
        }
        result.push(c);
        prev_char = c;
    }

    // 去除首尾空格并返回
    result.trim().to_string()
}

/// 移除字符串中的 HTML 标签
///
/// 简单的状态机实现，移除所有 <...> 内容
#[must_use]
pub fn strip_html_tags(input: &str) -> String {
    debug!("HTML 去标签: 输入 {} 字符", input.len());
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;

    // 预处理：将常见的 HTML 实体转义
    // 注意：这里只处理最基本的，完整处理需要专门的库
    let decoded = input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'");

    for c in decoded.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            // 标签结束时添加一个空格，防止 "End</p><p>Start" 变成 "EndStart"
            result.push(' ');
        } else if !in_tag {
            result.push(c);
        }
    }

    // 清理可能产生的多余空格
    clean_text_content(&result)
}

/// 清理用于UI显示的文本，移除所有换行符
///
/// 这个函数专门用于准备要在GPUI文本系统中显示的文本
///
/// # 参数
/// - `text`: 需要清理的文本
///
/// # 返回值
/// 清理后的文本，确保不包含换行符
#[must_use]
pub fn clean_for_ui_display(text: &str) -> String {
    text.replace('\n', " ").replace('\r', "").trim().to_string()
}

#[must_use]
pub fn clean_title(text: &str) -> String {
    let cleaned = clean_text_content(text);
    let result = to_title_case(&cleaned);
    debug!(
        "标题清理: '{}' -> '{}'",
        &text[..text.len().min(60)],
        &result[..result.len().min(60)]
    );
    result
}

/// 转换为标题格式 (Title Case)
///
/// 遵循 Zotero 风格的标题转换规则：
/// 1. 始终应用 Title Case，除非是被保护的特殊词。
/// 2. 只有当整个标题被判定为“呐喊式全大写”(Screaming Caps)时，才强制重置所有词的大小写（此时 Gpu 会变成 Gpu）。
/// 3. 如果标题是正常的（混合或全小写），则保护已有的混合大小写词（iPhone）和全大写缩写（GPU）。
/// 4. 支持连字符复合词的处理（State-of-the-Art）。
fn to_title_case(text: &str) -> String {
    let total_chars = text.chars().filter(|c| c.is_alphabetic()).count();
    if total_chars == 0 {
        return text.to_string();
    }

    // 1. 检测是否是全大写标题 (Screaming Caps)
    let upper_chars = text.chars().filter(|c| c.is_uppercase()).count();
    // 阈值 > 60% 且单词总数 > 0
    let is_screaming = (upper_chars as f32 / total_chars as f32) > 0.6;

    // 定义“小词” (Minor words) - 参考 Zotero / CMoS
    let minors = [
        "a", "an", "the", "and", "but", "or", "nor", "for", "yet", "so", "at", "by", "in", "of",
        "on", "to", "up", "as", "with", "via", "en", "vs", "v", "top",
        "bot", // 一些常见的
    ];

    let words: Vec<&str> = text.split_whitespace().collect();
    let count = words.len();
    let mut result = Vec::new();

    for (i, word) in words.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == count - 1;

        // 如果是全大写标题，首先将单词转为全小写作为基准，以便后续重新大写
        // 如果不是全大写标题，则保留原词（可能是 iPhone 或 GPU）
        let word_base = if is_screaming {
            word.to_lowercase()
        } else {
            word.to_string()
        };

        let new_word = process_word_casing(&word_base, &minors, is_first, is_last, !is_screaming);
        result.push(new_word);
    }

    result.join(" ")
}

fn process_word_casing(
    word: &str,
    minors: &[&str],
    is_first: bool,
    is_last: bool,
    protect_special: bool,
) -> String {
    // 检查是否有连字符
    if word.contains('-') {
        let parts: Vec<&str> = word.split('-').collect();
        let mut new_parts = Vec::new();
        for (j, part) in parts.iter().enumerate() {
            // 连字符后的部分通常也视为大词，除非是极短的介词（State-of-the-Art）
            // 这里简单起见，除了纯介词外都大写
            // Zotero 规则：总是大写连字符后的部分，除非它是小词
            let part_is_first = is_first && j == 0;
            let part_is_last = is_last && j == parts.len() - 1;
            new_parts.push(process_single_word_casing(
                part,
                minors,
                part_is_first,
                part_is_last,
                protect_special,
            ));
        }
        return new_parts.join("-");
    }

    process_single_word_casing(word, minors, is_first, is_last, protect_special)
}

fn process_single_word_casing(
    word: &str,
    minors: &[&str],
    is_first: bool,
    is_last: bool,
    protect_special: bool,
) -> String {
    // 保护特殊格式 (混合大小写 或 全大写)
    if protect_special && (is_mixed_case(word) || is_all_caps(word)) {
        return word.to_string();
    }

    let lower_word = word.to_lowercase();
    // 剥离标点符号进行检查 (例如 "(Hello)" -> "hello")
    let clean_lower = lower_word.trim_matches(|c: char| !c.is_alphabetic());

    // 0. 检查通用缩略词字典 (必须在小词检查之前，防止如 "IT" 被误判)
    // 即使原本是 introduction to gpu (被强制转小写了)，这里也能根据 gpu -> GPU 恢复
    if let Some(acronym) = check_known_acronyms(clean_lower) {
        // 恢复大写，但要保留原来的标点符号
        // 这是一个简单的替换：将 lower_word 中的 clean_lower 部分替换为 acronym
        // 例如 "(gpu)" -> "(GPU)"
        return lower_word.replace(clean_lower, acronym);
    }

    let is_minor = minors.contains(&clean_lower);

    // 规则：首尾词永远大写；中间的小词小写；中间的大词大写
    if !is_first && !is_last && is_minor {
        lower_word
    } else {
        capitalize_first_letter(&lower_word)
    }
}

/// 检查是否是已知的缩略词
fn check_known_acronyms(lower_word: &str) -> Option<&'static str> {
    // 常用技术与学术缩略词表
    const KNOWN_ACRONYMS: &[&str] = &[
        "GPU", "CPU", "TPU", "NPU", "FPGA", "ASIC", "AI", "ML", "DL", "RL", "LLM", "NLP", "CV",
        "RNN", "CNN", "LSTM", "GAN", "VAE", "API", "SDK", "IDE", "GUI", "CLI", "REST", "JSON",
        "XML", "YAML", "HTML", "CSS", "SQL", "HTTP", "HTTPS", "TCP", "IP", "UDP", "DNS", "SSH",
        "FTP", "OS", "IO", "PC", "VM", "RAM", "ROM", "ID", "DOI", "URL", "URI", "ISBN", "ISSN",
        "IEEE", "ACM", "ISO", "USA", "UK", "EU", "MIT", "PDF", "PNG", "JPG", "SVG", "GIF", "IoT",
        "SaaS", "PaaS", "IaaS", // 混合大小写特例
    ];

    KNOWN_ACRONYMS
        .iter()
        .find(|&&acronym| acronym.to_lowercase() == lower_word)
        .copied()
        .map(|v| v as _)
}

fn is_mixed_case(word: &str) -> bool {
    let has_upper = word.chars().any(char::is_uppercase);
    let has_lower = word.chars().any(char::is_lowercase);
    has_upper && has_lower
}

fn is_all_caps(word: &str) -> bool {
    let alpha_chars: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha_chars.is_empty() {
        return false;
    }
    alpha_chars.iter().all(|c| c.is_uppercase())
}

fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => {
            // 只大写第一个字母
            // 注意：如果第一个字符是标点（如 "(hello"），我们可能需要找到第一个字母？
            // Zotero 通常只大写第一个字符。
            // 改进：遍历直到找到第一个字母
            if f.is_alphabetic() {
                f.to_uppercase().collect::<String>() + chars.as_str()
            } else {
                // 如果开头是标点，尝试递归寻找字母？
                // 简单起见，保持原样，直接大写第一个字符（即便它是符号，to_upper不变）
                // 但如果是 "("，后面跟着 "h"，应该是 "(Hello"
                // 这里做一个简单增强：
                let mut res = String::new();
                res.push(f);
                let mut capitalized = false;
                for c in chars {
                    if !capitalized && c.is_alphabetic() {
                        res.push(c.to_uppercase().next().unwrap());
                        capitalized = true;
                    } else {
                        res.push(c);
                    }
                }
                res
            }
        }
    }
}

/// 清理摘要文本
///
/// 移除换行符、控制字符，并合理处理空格
#[must_use]
pub fn clean_abstract(text: &str) -> String {
    clean_text_content(text)
}

/// 清理作者姓名
///
/// 移除换行符但保留其他字符，适合作者姓名字段
#[must_use]
pub fn clean_author_name(text: &str) -> String {
    text.replace('\n', " ").replace('\r', "").trim().to_string()
}

/// 清理期刊/会议名称
///
/// 移除换行符并压缩空格
#[must_use]
pub fn clean_publication_name(text: &str) -> String {
    clean_text_content(text)
}

/// 从可选文本中清理内容
///
/// 如果文本为Some，则清理它；如果为None或清理后为空字符串，返回None
pub fn clean_optional_text(text: Option<&str>) -> Option<String> {
    text.map(clean_text_content).filter(|s| !s.is_empty())
}

/// 清理页码范围
///
/// 移除换行符，并将各种特殊横杠（如 en-dash, em-dash, minus sign, double hyphen）统一为标准连字符 (-)
#[must_use]
pub fn clean_page_range(text: &str) -> String {
    let cleaned = clean_text_content(text);
    // 1. 先替换 LaTeX 风格的双连字符
    let standard = cleaned.replace("--", "-");
    // 2. 再替换各种 Unicode 特殊横杠
    standard.replace(['–', '—', '−'], "-")
}

/// 从可选文本中清理页码范围
pub fn clean_optional_page_range(text: Option<&str>) -> Option<String> {
    text.map(clean_page_range).filter(|s| !s.is_empty())
}
