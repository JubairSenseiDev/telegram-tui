use unicode_width::UnicodeWidthChar;

/// Display columns a string occupies in a terminal cell grid.
pub fn width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Truncate to `max` display columns, appending an ellipsis when cut.
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if width(s) <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// Pad on the right to `cols` display columns, truncating when too wide.
pub fn pad(s: &str, cols: usize) -> String {
    let t = truncate(s, cols);
    let mut out = t;
    for _ in width(&out)..cols {
        out.push(' ');
    }
    out
}

/// Collapse control characters so a message never breaks the layout.
pub fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wrap into lines of at most `cols` display columns, breaking on spaces.
pub fn wrap(s: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for raw in s.split('\n') {
        let mut line = String::new();
        let mut used = 0usize;
        for word in raw.split(' ') {
            let w = width(word);
            if used > 0 && used + 1 + w > cols {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            if w > cols {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    used = 0;
                }
                for c in word.chars() {
                    let cw = c.width().unwrap_or(0);
                    if used + cw > cols {
                        lines.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    line.push(c);
                    used += cw;
                }
                continue;
            }
            if used > 0 {
                line.push(' ');
                used += 1;
            }
            line.push_str(word);
            used += w;
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Byte offset of a character index, for cursor math on multi-byte input.
pub fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_columns_not_chars() {
        assert_eq!(width("abc"), 3);
        assert_eq!(width("日本"), 4);
        assert_eq!(width(""), 0);
    }

    #[test]
    fn truncate_respects_display_width() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 8), "abc");
        assert_eq!(truncate("anything", 0), "");
        assert!(width(&truncate("日本語テスト", 6)) <= 6);
    }

    #[test]
    fn pad_fills_to_exact_columns() {
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(width(&pad("日本", 6)), 6);
        assert_eq!(width(&pad("abcdefgh", 4)), 4);
    }

    #[test]
    fn one_line_strips_newlines_and_runs() {
        assert_eq!(one_line("a\nb"), "a b");
        assert_eq!(one_line("a  \t b\r\n"), "a b");
    }

    #[test]
    fn wrap_breaks_on_words_and_splits_long_ones() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("a\nb", 10), vec!["a", "b"]);
        assert!(wrap("", 4).len() == 1);
    }

    #[test]
    fn byte_at_handles_multibyte() {
        assert_eq!(byte_at("abc", 1), 1);
        assert_eq!(byte_at("日本", 1), 3);
        assert_eq!(byte_at("ab", 9), 2);
    }
}
