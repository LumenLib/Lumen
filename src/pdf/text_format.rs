/// 文本格式化：段落重组 → 空白归一化 → 字符过滤
pub fn clean_translation_text(text: &str) -> String {
    let s = reconstruct_paragraphs(text);
    let s = normalize_whitespace(&s);
    filter_chars(&s)
}

/// 段落重组：单换行 → 空格，连续换行 → 段落分隔
fn reconstruct_paragraphs(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            let mut count = 1;
            while chars.peek() == Some(&'\n') || chars.peek() == Some(&'\r') {
                if chars.peek() == Some(&'\r') {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                } else {
                    chars.next();
                }
                count += 1;
            }
            if count == 1 {
                result.push(' ');
            } else {
                result.push_str("\n\n");
            }
        } else if c == '\n' {
            let mut count = 1;
            while chars.peek() == Some(&'\n') {
                chars.next();
                count += 1;
            }
            if count == 1 {
                result.push(' ');
            } else {
                result.push_str("\n\n");
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{2F800}'..='\u{2FA1F}'
    )
}

/// 空白归一化：trim + 合并连续空格 + 中文间去空格
fn normalize_whitespace(s: &str) -> String {
    let s = s.trim();
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        if c == '\n' {
            result.push(c);
            prev_space = false;
        } else if c.is_whitespace() {
            if prev_space {
                continue;
            }
            let prev = if i > 0 { chars[i - 1] } else { '\0' };
            let next = chars.get(i + 1).copied().unwrap_or('\0');
            if is_cjk(prev) && is_cjk(next) {
                prev_space = true;
                continue;
            }
            prev_space = true;
            result.push(' ');
        } else {
            prev_space = false;
            result.push(c);
        }
    }
    result
}

/// 字符过滤：移除控制字符，保留 \n 和 \t
fn filter_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect()
}
