/// Preprocess markdown text to convert LaTeX math expressions (`$...$`, `$$...$$`)
/// to Unicode approximations for rendering in GPUI's plain-text views.
///
/// ## Examples
///
/// ```ignore
/// assert_eq!(preprocess_math("$F_1$ score"), "F₁ score");
/// assert_eq!(preprocess_math("$\\alpha + \\beta$"), "α + β");
/// assert_eq!(preprocess_math("$$x^2$$"), "\nx²\n");
/// ```
pub fn preprocess_math(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // ── Skip code blocks (``` … ```) ──
        if bytes[i] == b'`' && i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            let block_start = i;
            i += 3;
            while i < len {
                if bytes[i] == b'`' && i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
                    i += 3;
                    break;
                }
                i += 1;
            }
            out.push_str(&text[block_start..i]);
            continue;
        }

        // ── $$ … $$ (block math) ──
        if bytes[i] == b'$' && i + 1 < len && bytes[i + 1] == b'$' {
            if let Some(end) = find_closing_delim(bytes, i + 2, b"$$") {
                out.push('\n');
                render_math(text[i + 2..end].trim(), &mut out);
                out.push('\n');
                i = end + 2;
                continue;
            }
            out.push_str("$$");
            i += 2;
            continue;
        }

        // ── $ … $ (inline math) ──
        if bytes[i] == b'$' {
            if let Some(end) = find_closing_delim(bytes, i + 1, b"$") {
                render_math(text[i + 1..end].trim(), &mut out);
                i = end + 1;
                continue;
            }
            out.push('$');
            i += 1;
            continue;
        }

        // ── \( … \) (inline math, LaTeX notation) ──
        if bytes[i] == b'\\' && i + 1 < len && bytes[i + 1] == b'(' {
            if let Some(end) = find_closing_escaped_delim(bytes, i + 2, b')') {
                render_math(text[i + 2..end].trim(), &mut out);
                i = end + 2;
                continue;
            }
            out.push_str("\\(");
            i += 2;
            continue;
        }

        // ── \[ … \] (display math, LaTeX notation) ──
        if bytes[i] == b'\\' && i + 1 < len && bytes[i + 1] == b'[' {
            if let Some(end) = find_closing_escaped_delim(bytes, i + 2, b']') {
                out.push('\n');
                render_math(text[i + 2..end].trim(), &mut out);
                out.push('\n');
                i = end + 2;
                continue;
            }
            out.push_str("\\[");
            i += 2;
            continue;
        }

        // ── Plain text (ASCII or multi-byte UTF-8) ──
        if bytes[i].is_ascii() {
            out.push(bytes[i] as char);
            i += 1;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    out
}

/// Find closing `\)` or `\]` (backslash-prefixed delimiter).
fn find_closing_escaped_delim(bytes: &[u8], start: usize, expected: u8) -> Option<usize> {
    let mut i = start;
    let mut brace_depth = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if i + 1 < bytes.len() && brace_depth == 0 && bytes[i + 1] == expected {
                return Some(i);
            }
            i += 2;
            continue;
        }
        if bytes[i] == b'{' {
            brace_depth += 1;
        } else if bytes[i] == b'}' && brace_depth > 0 {
            brace_depth -= 1;
        }
        i += 1;
    }
    None
}

fn find_closing_delim(bytes: &[u8], start: usize, delim: &[u8]) -> Option<usize> {
    let mut i = start;
    let mut brace_depth = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'{' {
            brace_depth += 1;
        } else if bytes[i] == b'}' && brace_depth > 0 {
            brace_depth -= 1;
        } else if brace_depth == 0
            && bytes[i] == delim[0]
            && (delim.len() == 1 || (i + 1 < bytes.len() && bytes[i + 1] == delim[1]))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut i: usize, len: usize) -> usize {
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn read_braced(bytes: &[u8], i: &mut usize, len: usize) -> String {
    let start = *i;
    if *i >= len || bytes[*i] != b'{' {
        return String::new();
    }
    *i += 1;
    let mut depth = 1;
    while *i < len && depth > 0 {
        if bytes[*i] == b'\\' {
            *i += 2;
        } else if bytes[*i] == b'{' {
            depth += 1;
            *i += 1;
        } else if bytes[*i] == b'}' {
            depth -= 1;
            if depth > 0 {
                *i += 1;
            }
        } else {
            *i += 1;
        }
    }
    let content = &bytes[start..*i];
    let s = std::str::from_utf8(content).unwrap_or("");
    *i += 1;
    // Recursively render the content
    let mut buf = String::new();
    render_math(s, &mut buf);
    buf
}

fn read_braced_literal(bytes: &[u8], i: &mut usize, len: usize) -> String {
    if *i >= len || bytes[*i] != b'{' {
        return String::new();
    }
    *i += 1;
    let mut depth = 1;
    let mut out = String::new();
    while *i < len && depth > 0 {
        if bytes[*i] == b'\\' {
            *i += 1;
            if *i < len {
                let next_b = bytes[*i];
                match next_b {
                    b'_' => out.push('_'),
                    b'&' => out.push('&'),
                    b'%' => out.push('%'),
                    b'{' => out.push('{'),
                    b'}' => out.push('}'),
                    b' ' => out.push(' '),
                    _ => {
                        out.push('\\');
                        if next_b.is_ascii_alphabetic() {
                            let start = *i;
                            while *i < len && bytes[*i].is_ascii_alphabetic() {
                                *i += 1;
                            }
                            let cmd = std::str::from_utf8(&bytes[start..*i]).unwrap_or("");
                            out.push_str(cmd);
                            continue;
                        } else {
                            out.push(next_b as char);
                        }
                    }
                }
                *i += 1;
            }
        } else if bytes[*i] == b'{' {
            depth += 1;
            out.push('{');
            *i += 1;
        } else if bytes[*i] == b'}' {
            depth -= 1;
            if depth > 0 {
                out.push('}');
                *i += 1;
            }
        } else {
            if bytes[*i].is_ascii() {
                out.push(bytes[*i] as char);
                *i += 1;
            } else {
                if let Ok(s) = std::str::from_utf8(&bytes[*i..]) {
                    if let Some(ch) = s.chars().next() {
                        out.push(ch);
                        *i += ch.len_utf8();
                        continue;
                    }
                }
                out.push(bytes[*i] as char);
                *i += 1;
            }
        }
    }
    *i += 1;
    out
}

fn read_group_or_char(bytes: &[u8], i: &mut usize, len: usize) -> String {
    *i = skip_ws(bytes, *i, len);
    if *i >= len {
        return String::new();
    }
    if bytes[*i] == b'{' {
        read_braced(bytes, i, len)
    } else if bytes[*i] == b'\\' {
        let mut buf = String::new();
        *i += 1;
        if *i < len && bytes[*i].is_ascii_alphabetic() {
            let start = *i;
            while *i < len && bytes[*i].is_ascii_alphabetic() {
                *i += 1;
            }
            let cmd = &bytes[start..*i];
            let s = std::str::from_utf8(cmd).unwrap_or("");
            handle_cmd_internal(s, bytes, i, &bytes[start..*i], &mut buf);
            buf
        } else if *i < len {
            handle_non_alpha_cmd(bytes[*i], &mut buf);
            *i += 1;
            buf
        } else {
            buf
        }
    } else {
        let ch = bytes[*i] as char;
        *i += 1;
        ch.to_string()
    }
}

fn to_superscript(c: char) -> Option<char> {
    match c {
        '0' => Some('\u{2070}'),
        '1' => Some('\u{00B9}'),
        '2' => Some('\u{00B2}'),
        '3' => Some('\u{00B3}'),
        '4' => Some('\u{2074}'),
        '5' => Some('\u{2075}'),
        '6' => Some('\u{2076}'),
        '7' => Some('\u{2077}'),
        '8' => Some('\u{2078}'),
        '9' => Some('\u{2079}'),
        '+' => Some('\u{207A}'),
        '-' => Some('\u{207B}'),
        '=' => Some('\u{207C}'),
        '(' => Some('\u{207D}'),
        ')' => Some('\u{207E}'),
        'a' => Some('\u{1D43}'),
        'A' => Some('\u{1D2C}'),
        'b' => Some('\u{1D47}'),
        'B' => Some('\u{1D2E}'),
        'c' | 'C' => Some('\u{1D9C}'),
        'd' => Some('\u{1D48}'),
        'D' => Some('\u{1D30}'),
        'e' => Some('\u{1D49}'),
        'E' => Some('\u{1D49}'), // Fallback to lowercase superscript e since no capital superscript E exists in standard Unicode
        'f' | 'F' => Some('\u{1DA0}'),
        'g' => Some('\u{1D4D}'),
        'G' => Some('\u{1D35}'),
        'h' => Some('\u{02B0}'),
        'H' => Some('\u{1D36}'),
        'i' => Some('\u{2071}'),
        'I' => Some('\u{1D37}'),
        'j' => Some('\u{02B2}'),
        'J' => Some('\u{1D38}'),
        'k' => Some('\u{1D4F}'),
        'K' => Some('\u{1D39}'),
        'l' => Some('\u{02E1}'),
        'L' => Some('\u{1D3A}'),
        'm' => Some('\u{1D50}'),
        'M' => Some('\u{1D3C}'),
        'n' => Some('\u{207F}'),
        'N' => Some('\u{1D3E}'),
        'o' => Some('\u{1D52}'),
        'O' => Some('\u{1D3F}'),
        'p' => Some('\u{1D56}'),
        'P' => Some('\u{1D40}'),
        'r' => Some('\u{02B3}'),
        'R' => Some('\u{1D5F}'),
        's' | 'S' => Some('\u{02E2}'),
        't' => Some('\u{1D57}'),
        'T' => Some('\u{1D41}'),
        'u' => Some('\u{1D58}'),
        'U' => Some('\u{1D42}'),
        'v' => Some('\u{1D5B}'),
        'V' => Some('\u{2C7D}'),
        'w' => Some('\u{02B7}'),
        'W' => Some('\u{1D42}'), // Fallback to superscript U or lowercase w
        'x' | 'X' => Some('\u{02E3}'),
        'y' | 'Y' => Some('\u{02B8}'),
        'z' | 'Z' => Some('\u{1DBB}'),
        _ => None,
    }
}

fn to_subscript(c: char) -> Option<char> {
    match c {
        '0' => Some('\u{2080}'),
        '1' => Some('\u{2081}'),
        '2' => Some('\u{2082}'),
        '3' => Some('\u{2083}'),
        '4' => Some('\u{2084}'),
        '5' => Some('\u{2085}'),
        '6' => Some('\u{2086}'),
        '7' => Some('\u{2087}'),
        '8' => Some('\u{2088}'),
        '9' => Some('\u{2089}'),
        '+' => Some('\u{208A}'),
        '-' => Some('\u{208B}'),
        '=' => Some('\u{208C}'),
        '(' => Some('\u{208D}'),
        ')' => Some('\u{208E}'),
        'a' | 'A' => Some('\u{2090}'),
        'e' | 'E' => Some('\u{2091}'),
        'h' | 'H' => Some('\u{2095}'),
        'i' | 'I' => Some('\u{1D62}'),
        'j' | 'J' => Some('\u{2C7C}'),
        'k' | 'K' => Some('\u{2096}'),
        'l' | 'L' => Some('\u{2097}'),
        'm' | 'M' => Some('\u{2098}'),
        'n' | 'N' => Some('\u{2099}'),
        'o' | 'O' => Some('\u{2092}'),
        'p' | 'P' => Some('\u{209A}'),
        'r' | 'R' => Some('\u{1D63}'),
        's' | 'S' => Some('\u{209B}'),
        't' | 'T' => Some('\u{209C}'),
        'u' | 'U' => Some('\u{1D64}'),
        'v' | 'V' => Some('\u{1D65}'),
        'x' | 'X' => Some('\u{2093}'),
        'β' => Some('\u{1D66}'),
        'γ' => Some('\u{1D67}'),
        'ρ' => Some('\u{1D68}'),
        'ϕ' | 'φ' => Some('\u{1D69}'),
        'χ' => Some('\u{1D6A}'),
        _ => None,
    }
}

fn render_math(math: &str, out: &mut String) {
    let bytes = math.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let c = bytes[i] as char;

        if c.is_ascii_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            continue;
        }

        if c == '\\' {
            i += 1;
            if i >= len {
                out.push('\\');
                break;
            }
            if bytes[i] == b'\\' {
                i += 1;
                continue;
            }
            if !bytes[i].is_ascii_alphabetic() {
                handle_non_alpha_cmd(bytes[i], out);
                i += 1;
                continue;
            }
            let cmd_start = i;
            while i < len && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let cmd = &math[cmd_start..i];
            handle_cmd_internal(cmd, bytes, &mut i, bytes, out);
            continue;
        }

        if c == '^' {
            i += 1;
            i = skip_ws(bytes, i, len);
            if i >= len {
                out.push('^');
                continue;
            }
            if bytes[i] == b'{' {
                i += 1;
                let mut depth = 1;
                let content_start = i;
                while i < len && depth > 0 {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == b'{' {
                        depth += 1;
                        i += 1;
                    } else if bytes[i] == b'}' {
                        depth -= 1;
                        if depth > 0 {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                let content = &bytes[content_start..i];
                let s = std::str::from_utf8(content).unwrap_or("");
                let mut rendered = String::new();
                render_math(s, &mut rendered);
                for ch in rendered.chars() {
                    if let Some(sup) = to_superscript(ch) {
                        out.push(sup);
                    } else if ch == '_' {
                        out.push('_');
                    } else if !ch.is_ascii_alphanumeric() {
                        out.push(ch);
                    } else {
                        out.push('^');
                        out.push(ch);
                    }
                }
                i += 1;
            } else {
                let ch = bytes[i] as char;
                i += 1;
                if let Some(sup) = to_superscript(ch) {
                    out.push(sup);
                } else if !ch.is_ascii_alphanumeric() {
                    out.push(ch);
                } else {
                    out.push('^');
                    out.push(ch);
                }
            }
            continue;
        }

        if c == '_' {
            i += 1;
            i = skip_ws(bytes, i, len);
            if i >= len {
                out.push('_');
                continue;
            }
            if bytes[i] == b'{' {
                i += 1;
                let mut depth = 1;
                let content_start = i;
                while i < len && depth > 0 {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == b'{' {
                        depth += 1;
                        i += 1;
                    } else if bytes[i] == b'}' {
                        depth -= 1;
                        if depth > 0 {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                let content = &bytes[content_start..i];
                let s = std::str::from_utf8(content).unwrap_or("");
                let mut rendered = String::new();
                render_math(s, &mut rendered);
                for ch in rendered.chars() {
                    if let Some(sub) = to_subscript(ch) {
                        out.push(sub);
                    } else if ch == '_' {
                        out.push('_');
                    } else if !ch.is_ascii_alphanumeric() {
                        out.push(ch);
                    } else {
                        out.push('_');
                        out.push(ch);
                    }
                }
                i += 1;
            } else {
                let ch = bytes[i] as char;
                i += 1;
                if let Some(sub) = to_subscript(ch) {
                    out.push(sub);
                } else if !ch.is_ascii_alphanumeric() {
                    out.push(ch);
                } else {
                    out.push('_');
                    out.push(ch);
                }
            }
            continue;
        }

        if c == '{' || c == '}' {
            // Grouping braces are transparent in output
            i += 1;
            continue;
        }

        if c == '~' {
            out.push(' ');
            i += 1;
            continue;
        }

        if c == '&' {
            out.push(' ');
            i += 1;
            continue;
        }

        if c == '#' || c == '%' {
            i += 1;
            continue;
        }

        out.push(c);
        i += 1;
    }
}

fn handle_non_alpha_cmd(b: u8, out: &mut String) {
    match b {
        b'(' | b')' | b'[' | b']' => out.push(b as char),
        b'.' => out.push('.'),
        b',' | b';' | b':' | b'&' => out.push(' '),
        b'!' => {}
        b'#' => out.push('#'),
        b'%' => {}
        b'_' => out.push('_'),
        b'{' => out.push('{'),
        b'}' => out.push('}'),
        b'~' => out.push(' '),
        b' ' => out.push(' '),
        b'$' => out.push('$'),
        _ => {
            out.push('\\');
            out.push(b as char);
        }
    }
}

fn handle_cmd_internal(
    cmd: &str,
    bytes: &[u8],
    i: &mut usize,
    _math_bytes: &[u8],
    out: &mut String,
) {
    match cmd {
        // Greek lowercase
        "alpha" => out.push('α'),
        "beta" => out.push('β'),
        "gamma" => out.push('γ'),
        "delta" => out.push('δ'),
        "epsilon" => out.push('ε'),
        "varepsilon" => out.push('ε'),
        "zeta" => out.push('ζ'),
        "eta" => out.push('η'),
        "theta" => out.push('θ'),
        "vartheta" => out.push('ϑ'),
        "iota" => out.push('ι'),
        "kappa" => out.push('κ'),
        "lambda" => out.push('λ'),
        "mu" => out.push('μ'),
        "nu" => out.push('ν'),
        "xi" => out.push('ξ'),
        "omicron" => out.push('ο'),
        "pi" => out.push('π'),
        "varpi" => out.push('ϖ'),
        "rho" => out.push('ρ'),
        "varrho" => out.push('ϱ'),
        "sigma" => out.push('σ'),
        "varsigma" => out.push('ς'),
        "tau" => out.push('τ'),
        "upsilon" => out.push('υ'),
        "phi" => out.push('φ'),
        "varphi" => out.push('ϕ'),
        "chi" => out.push('χ'),
        "psi" => out.push('ψ'),
        "omega" => out.push('ω'),

        // Greek uppercase
        "Alpha" => out.push('Α'),
        "Beta" => out.push('Β'),
        "Gamma" => out.push('Γ'),
        "Delta" => out.push('Δ'),
        "Epsilon" => out.push('Ε'),
        "Zeta" => out.push('Ζ'),
        "Eta" => out.push('Η'),
        "Theta" => out.push('Θ'),
        "Iota" => out.push('Ι'),
        "Kappa" => out.push('Κ'),
        "Lambda" => out.push('Λ'),
        "Mu" => out.push('Μ'),
        "Nu" => out.push('Ν'),
        "Xi" => out.push('Ξ'),
        "Omicron" => out.push('Ο'),
        "Pi" => out.push('Π'),
        "Rho" => out.push('Ρ'),
        "Sigma" => out.push('Σ'),
        "Tau" => out.push('Τ'),
        "Upsilon" => out.push('Υ'),
        "Phi" => out.push('Φ'),
        "Chi" => out.push('Χ'),
        "Psi" => out.push('Ψ'),
        "Omega" => out.push('Ω'),

        // Large operators
        "sum" => out.push('∑'),
        "prod" => out.push('∏'),
        "coprod" => out.push('∐'),
        "int" => out.push('∫'),
        "iint" => out.push('∬'),
        "iiint" => out.push('∭'),
        "oint" => out.push('∮'),
        "bigcap" => out.push('⋂'),
        "bigcup" => out.push('⋃'),
        "bigsqcup" => out.push('⨆'),
        "bigvee" => out.push('⋁'),
        "bigwedge" => out.push('⋀'),
        "bigodot" => out.push('⨀'),
        "bigoplus" => out.push('⨁'),
        "bigotimes" => out.push('⨂'),

        // Binary operations
        "times" => out.push('×'),
        "div" => out.push('÷'),
        "pm" => out.push('±'),
        "mp" => out.push('∓'),
        "cdot" => out.push('·'),
        "circ" => out.push('∘'),
        "bullet" => out.push('•'),
        "ast" => out.push('∗'),
        "star" => out.push('⋆'),
        "dagger" => out.push('†'),
        "ddagger" => out.push('‡'),
        "wedge" => out.push('∧'),
        "vee" => out.push('∨'),
        "cap" => out.push('∩'),
        "cup" => out.push('∪'),
        "uplus" => out.push('⊎'),
        "sqcap" => out.push('⊓'),
        "sqcup" => out.push('⊔'),

        // Relations
        "leq" | "le" => out.push('≤'),
        "geq" | "ge" => out.push('≥'),
        "neq" | "ne" => out.push('≠'),
        "approx" => out.push('≈'),
        "equiv" => out.push('≡'),
        "sim" => out.push('∼'),
        "simeq" => out.push('≃'),
        "cong" => out.push('≅'),
        "propto" => out.push('∝'),
        "models" => out.push('⊨'),
        "mid" => out.push('∣'),
        "parallel" => out.push('∥'),
        "doteq" => out.push('≐'),
        "subset" => out.push('⊂'),
        "supset" => out.push('⊃'),
        "subseteq" => out.push('⊆'),
        "supseteq" => out.push('⊇'),
        "sqsubset" => out.push('⊏'),
        "sqsupset" => out.push('⊐'),
        "sqsubseteq" => out.push('⊑'),
        "sqsupseteq" => out.push('⊒'),
        "in" => out.push('∈'),
        "ni" | "owns" => out.push('∋'),
        "notin" => out.push('∉'),

        // Arrows
        "to" | "rightarrow" => out.push('→'),
        "gets" | "leftarrow" => out.push('←'),
        "leftrightarrow" => out.push('↔'),
        "Rightarrow" => out.push('⇒'),
        "Leftarrow" => out.push('⇐'),
        "Leftrightarrow" => out.push('⇔'),
        "mapsto" => out.push('↦'),
        "longrightarrow" => out.push('⟶'),
        "longleftarrow" => out.push('⟵'),
        "Longrightarrow" => out.push('⟹'),
        "Longleftarrow" => out.push('⟸'),
        "uparrow" => out.push('↑'),
        "downarrow" => out.push('↓'),
        "updownarrow" => out.push('↕'),
        "nearrow" => out.push('↗'),
        "searrow" => out.push('↘'),
        "nwarrow" => out.push('↖'),
        "swarrow" => out.push('↙'),

        // Dots
        "dots" | "ldots" => out.push('…'),
        "cdots" => out.push('⋯'),
        "vdots" => out.push('⋮'),
        "ddots" => out.push('⋱'),

        // Miscellaneous symbols
        "infty" => out.push('∞'),
        "partial" => out.push('∂'),
        "nabla" => out.push('∇'),
        "forall" => out.push('∀'),
        "exists" => out.push('∃'),
        "nexists" => out.push('∄'),
        "emptyset" | "varnothing" => out.push('∅'),
        "hbar" => out.push('ℏ'),
        "ell" => out.push('ℓ'),
        "imath" => out.push('ı'),
        "jmath" => out.push('ȷ'),
        "Re" => out.push('ℜ'),
        "Im" => out.push('ℑ'),
        "aleph" => out.push('ℵ'),
        "wp" => out.push('℘'),
        "triangle" => out.push('△'),
        "angle" => out.push('∠'),
        "perp" => out.push('⟂'),
        "top" => out.push('⊤'),
        "bot" => out.push('⊥'),
        "clubsuit" => out.push('♣'),
        "diamondsuit" => out.push('♦'),
        "heartsuit" => out.push('♥'),
        "spadesuit" => out.push('♠'),
        "Box" => out.push('□'),
        "Diamond" => out.push('◇'),

        // Functions (rendered as upright text)
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "sinh" | "cosh" | "tanh" | "coth"
        | "arcsin" | "arccos" | "arctan" | "arccot" | "arcsec" | "arccsc" | "log" | "ln" | "lg"
        | "exp" | "det" | "dim" | "hom" | "ker" | "min" | "max" | "sup" | "inf" | "lim" | "arg"
        | "deg" | "Pr" | "mod" => {
            out.push_str(cmd);
        }
        "limsup" => out.push_str("lim sup"),
        "liminf" => out.push_str("lim inf"),
        "pmod" => {
            out.push_str(" (mod ");
            let content = read_group_or_char(bytes, i, bytes.len());
            out.push_str(&content);
            out.push(')');
        }
        "bmod" => out.push_str(" mod "),

        // \operatorname{...}
        "operatorname" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() && bytes[*i] == b'{' {
                let content = read_braced(bytes, i, bytes.len());
                out.push_str(&content);
            }
        }

        // Fractions
        "frac" | "dfrac" | "tfrac" => {
            let num = read_group_or_char(bytes, i, bytes.len());
            let den = read_group_or_char(bytes, i, bytes.len());
            out.push_str(&num);
            out.push('⁄');
            out.push_str(&den);
        }

        // Square root
        "sqrt" => {
            *i = skip_ws(bytes, *i, bytes.len());
            // Optional [nth] root index
            if *i < bytes.len() && bytes[*i] == b'[' {
                *i += 1;
                let root_start = *i;
                let mut depth = 1;
                while *i < bytes.len() && depth > 0 {
                    if bytes[*i] == b'[' {
                        depth += 1;
                    } else if bytes[*i] == b']' {
                        depth -= 1;
                    }
                    *i += 1;
                }
                let root = &bytes[root_start..*i - 1];
                let root_str = std::str::from_utf8(root).unwrap_or("");
                for ch in root_str.chars() {
                    if let Some(sup) = to_superscript(ch) {
                        out.push(sup);
                    } else {
                        out.push(ch);
                    }
                }
            }
            out.push('√');
            let content = read_group_or_char(bytes, i, bytes.len());
            if !content.is_empty() {
                out.push('(');
                out.push_str(&content);
                out.push(')');
            }
        }

        // Binomial
        "binom" => {
            let num = read_group_or_char(bytes, i, bytes.len());
            let den = read_group_or_char(bytes, i, bytes.len());
            out.push('(');
            out.push_str(&num);
            out.push('/');
            out.push_str(&den);
            out.push(')');
        }

        // Text
        "text" | "normaltext" | "mbox" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() && bytes[*i] == b'{' {
                let content = read_braced_literal(bytes, i, bytes.len());
                out.push_str(&content);
            }
        }
        "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt" | "textbf" | "textit" | "textrm"
        | "textsf" | "texttt" | "textsc" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() && bytes[*i] == b'{' {
                let content = read_braced(bytes, i, bytes.len());
                out.push_str(&content);
            }
        }

        // Font commands
        "mathbb" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() && bytes[*i] == b'{' {
                let content = read_braced(bytes, i, bytes.len());
                for ch in content.chars() {
                    match ch {
                        'R' => out.push('ℝ'),
                        'N' => out.push('ℕ'),
                        'Z' => out.push('ℤ'),
                        'Q' => out.push('ℚ'),
                        'C' => out.push('ℂ'),
                        'H' => out.push('ℍ'),
                        'P' => out.push('ℙ'),
                        'B' => out.push('𝔹'),
                        'D' => out.push('𝔻'),
                        'F' => out.push('𝔽'),
                        _ => out.push(ch),
                    }
                }
            }
        }
        "mathcal" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() && bytes[*i] == b'{' {
                let content = read_braced(bytes, i, bytes.len());
                for ch in content.chars() {
                    match ch {
                        'A' => out.push('𝒜'),
                        'B' => out.push('ℬ'),
                        'C' => out.push('𝒞'),
                        'D' => out.push('𝒟'),
                        'E' => out.push('ℰ'),
                        'F' => out.push('ℱ'),
                        'G' => out.push('𝒢'),
                        'H' => out.push('ℋ'),
                        'I' => out.push('ℐ'),
                        'J' => out.push('𝒥'),
                        'K' => out.push('𝒦'),
                        'L' => out.push('ℒ'),
                        'M' => out.push('ℳ'),
                        'N' => out.push('𝒩'),
                        'O' => out.push('𝒪'),
                        'P' => out.push('𝒫'),
                        'Q' => out.push('𝒬'),
                        'R' => out.push('ℛ'),
                        'S' => out.push('𝒮'),
                        'T' => out.push('𝒯'),
                        'U' => out.push('𝒰'),
                        'V' => out.push('𝒱'),
                        'W' => out.push('𝒲'),
                        'X' => out.push('𝒳'),
                        'Y' => out.push('𝒴'),
                        'Z' => out.push('𝒵'),
                        _ => out.push(ch),
                    }
                }
            }
        }

        // Left/right/big delimiters
        "left" | "right" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() {
                let d = bytes[*i];
                *i += 1;
                if d == b'.' || d == b'|' {
                    if d == b'|' {
                        out.push('|');
                    }
                } else if d == b'\\' {
                    read_delim_command(bytes, i, out);
                } else {
                    out.push(d as char);
                }
            }
        }
        "big" | "Big" | "bigg" | "Bigg" | "bigl" | "Bigl" | "biggl" | "Biggl" | "bigr" | "Bigr"
        | "biggr" | "Biggr" | "bigm" | "Bigm" | "biggm" | "Biggm" => {
            *i = skip_ws(bytes, *i, bytes.len());
            if *i < bytes.len() {
                let d = bytes[*i];
                *i += 1;
                if d == b'.' || d == b'|' {
                    if d == b'|' {
                        out.push('|');
                    }
                } else if d == b'\\' {
                    read_delim_command(bytes, i, out);
                } else {
                    out.push(d as char);
                }
            }
        }

        // Accents
        "hat" => render_accent(bytes, i, '\u{0302}', out),
        "tilde" => render_accent(bytes, i, '\u{0303}', out),
        "bar" => render_accent(bytes, i, '\u{0304}', out),
        "overline" => render_accent(bytes, i, '\u{0305}', out),
        "vec" => render_accent(bytes, i, '\u{20D7}', out),
        "dot" => render_accent(bytes, i, '\u{0307}', out),
        "ddot" => render_accent(bytes, i, '\u{0308}', out),
        "check" => render_accent(bytes, i, '\u{030C}', out),
        "acute" => render_accent(bytes, i, '\u{0301}', out),
        "grave" => render_accent(bytes, i, '\u{0300}', out),
        "breve" => render_accent(bytes, i, '\u{0306}', out),

        // Underscore (used in \__foo__)
        "_" => out.push('_'),

        // Commands that are no-ops in our renderer
        "displaystyle" | "scriptstyle" | "textstyle" | "tiny" | "small" | "normalsize"
        | "large" | "Large" | "LARGE" | "huge" | "Huge" | "rm" | "it" | "em" | "bf" | "sf"
        | "tt" | "sc" | "enspace" | "quad" | "qquad" | "thinspace" | "negthinspace"
        | "negmedspace" | "negthickspace" | "medskip" | "bigskip" | "smallskip" | "label"
        | "tag" | "notag" | "nonumber" | "cr" | "hfill" | "vfill" | "hfil" | "vfil"
        | "noindent" | "indent" | "raggedright" | "raggedleft" | "centering" => {}

        // Unknown command — emit as raw LaTeX
        _ => {
            out.push('\\');
            out.push_str(cmd);
        }
    }
}

fn read_delim_command(bytes: &[u8], i: &mut usize, out: &mut String) {
    let start = *i;
    while *i < bytes.len() && bytes[*i].is_ascii_alphabetic() {
        *i += 1;
    }
    let cmd = std::str::from_utf8(&bytes[start..*i]).unwrap_or("");
    match cmd {
        "vert" | "|" => out.push('|'),
        "Vert" | "\\|" => out.push('‖'),
        "lbrace" | "{" => out.push('{'),
        "rbrace" | "}" => out.push('}'),
        "langle" => out.push('⟨'),
        "rangle" => out.push('⟩'),
        "lfloor" => out.push('⌊'),
        "rfloor" => out.push('⌋'),
        "lceil" => out.push('⌈'),
        "rceil" => out.push('⌉'),
        "lgroup" => out.push('('),
        "rgroup" => out.push(')'),
        "arrowvert" => out.push('|'),
        "Arrowvert" => out.push('‖'),
        "bracevert" => out.push('|'),
        _ => {
            out.push('\\');
            out.push_str(cmd);
        }
    }
}

fn render_accent(bytes: &[u8], i: &mut usize, accent: char, out: &mut String) {
    *i = skip_ws(bytes, *i, bytes.len());
    let content = read_group_or_char(bytes, i, bytes.len());
    if content.len() == 1 {
        let ch = content.chars().next().unwrap();
        // Skip if combining char can't be meaningfully applied
        if ch.is_ascii_alphabetic() {
            out.push(ch);
            out.push(accent);
            return;
        }
    }
    out.push_str(&content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_inline_math() {
        assert_eq!(preprocess_math("$F_1$ score"), "F₁ score");
    }

    #[test]
    fn test_greek_letters() {
        assert_eq!(preprocess_math("$\\alpha + \\beta$"), "α + β");
    }

    #[test]
    fn test_superscript() {
        assert_eq!(preprocess_math("$x^2$"), "x²");
        assert_eq!(preprocess_math("$x^{n+1}$"), "xⁿ⁺¹");
    }

    #[test]
    fn test_subscript() {
        assert_eq!(preprocess_math("$a_{ij}$"), "aᵢⱼ");
    }

    #[test]
    fn test_frac() {
        assert_eq!(preprocess_math("$\\frac{a}{b}$"), "a⁄b");
    }

    #[test]
    fn test_sqrt() {
        assert_eq!(preprocess_math("$\\sqrt{x}$"), "√(x)");
    }

    #[test]
    fn test_operators() {
        assert_eq!(preprocess_math("$\\sum_{i=1}^n$"), "∑ᵢ₌₁ⁿ");
    }

    #[test]
    fn test_block_math() {
        let result = preprocess_math("$$x = y$$");
        assert!(result.contains("x = y"));
        assert!(result.starts_with('\n'));
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_no_math_passthrough() {
        let text = "Hello, world!";
        assert_eq!(preprocess_math(text), text);
    }

    #[test]
    fn test_utf8_chinese_passthrough() {
        let text = "这是一个测试：核心方法。";
        assert_eq!(preprocess_math(text), text);
    }

    #[test]
    fn test_utf8_mixed_with_math() {
        let result = preprocess_math("损失函数 $\\mathcal{L}$ 是核心。");
        assert!(result.contains('ℒ'));
        assert!(result.contains("是核心"));
        assert!(!result.contains("$"));
    }

    #[test]
    fn test_paren_delim_inline() {
        let result = preprocess_math("\\(\\alpha + \\beta\\)");
        assert_eq!(result, "α + β");
    }

    #[test]
    fn test_bracket_delim_display() {
        let result = preprocess_math("\\[x^2\\]");
        assert!(result.contains("x²"));
        assert!(result.starts_with('\n'));
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_skip_code_block() {
        let input = "text ```python\nprint(\"$5\")\n``` end";
        assert_eq!(preprocess_math(input), input);
    }

    #[test]
    fn test_math_with_unicode_in_text() {
        // Chinese + math should not corrupt Chinese chars
        let result = preprocess_math("模型 $F_1$ 分数");
        assert_eq!(result, "模型 F₁ 分数");
    }

    #[test]
    fn test_mathbb() {
        assert_eq!(preprocess_math("$\\mathbb{R}$"), "ℝ");
        assert_eq!(preprocess_math("$\\mathbb{R}^d$"), "ℝᵈ");
    }

    #[test]
    fn test_mathcal() {
        assert_eq!(preprocess_math("$\\mathcal{L}$"), "ℒ");
        assert_eq!(preprocess_math("$\\mathcal{A}_M$"), "𝒜ₘ");
        assert_eq!(preprocess_math("$\\mathcal{A}_M^{(s)}$"), "𝒜ₘ⁽ˢ⁾");
    }

    #[test]
    fn test_text_braced_literal() {
        assert_eq!(preprocess_math("$\\text{null_base}$"), "null_base");
        assert_eq!(preprocess_math("$\\text{null\\_base}$"), "null_base");
        assert_eq!(
            preprocess_math("$\\mathbf{e}_{M}^{\\text{null\\_base}}$"),
            "eₘⁿᵘˡˡ_ᵇᵃˢᵉ"
        );
    }

    #[test]
    fn test_functions() {
        assert_eq!(preprocess_math("$\\sin(x)$"), "sin(x)");
        assert_eq!(preprocess_math("$\\log x$"), "log x");
    }

    #[test]
    fn test_user_report() {
        let input1 = r#"\( \mathcal{A}_M(\mathbf{z}_{M'}^{(s)}; \mathbf{e}_{M}^{\text{null\_base}}) = \text{LayerNorm} \left( \text{MLP}_M^{\text{ctx}} \left[ \mathbf{e}_{M}^{\text{null\_base}}; \text{StopGrad}(\mathbf{z}_{M'}^{(s)}) \right] + \mathbf{e}_{M}^{\text{null\_base}} \right) \)"#;
        let output1 = preprocess_math(input1);
        assert_eq!(
            output1,
            "𝒜ₘ(zₘ'⁽ˢ⁾; eₘⁿᵘˡˡ_ᵇᵃˢᵉ) = LayerNorm ( MLPₘᶜᵗˣ [ eₘⁿᵘˡˡ_ᵇᵃˢᵉ; StopGrad(zₘ'⁽ˢ⁾) ] + eₘⁿᵘˡˡ_ᵇᵃˢᵉ )"
        );

        let input3 = r#"\( \mathbf{e}_{M}^{\text{null_base}} \)"#;
        let output3 = preprocess_math(input3);
        assert_eq!(output3, "eₘⁿᵘˡˡ_ᵇᵃˢᵉ");

        let input4 = r#"\( \{\hat{\mathbf{z}}_M^{(s)}\}_{M \in \mathcal{S}} \)"#;
        let output4 = preprocess_math(input4);
        assert_eq!(output4, "{ẑₘ⁽ˢ⁾}ₘ ∈ 𝒮");
    }
}
