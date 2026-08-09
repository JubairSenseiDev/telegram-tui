use crate::text;

/// A single-line text field with a character-indexed cursor.
#[derive(Default, Clone)]
pub struct Input {
    pub text: String,
    pub cursor: usize,
}

impl Input {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn set(&mut self, value: &str) {
        self.text = value.to_string();
        self.cursor = self.text.chars().count();
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Take the trimmed contents, leaving the field empty.
    pub fn take(&mut self) -> String {
        let out = self.text.trim().to_string();
        self.clear();
        out
    }

    pub fn insert(&mut self, c: char) {
        let at = text::byte_at(&self.text, self.cursor);
        self.text.insert(at, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let at = text::byte_at(&self.text, self.cursor - 1);
        self.text.remove(at);
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.text.chars().count() {
            return;
        }
        let at = text::byte_at(&self.text, self.cursor);
        self.text.remove(at);
    }

    /// Delete back to the start of the previous word.
    pub fn delete_word(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let keep: String = chars[..i].iter().chain(chars[self.cursor..].iter()).collect();
        self.text = keep;
        self.cursor = i;
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    /// The visible slice and the cursor column within it, for a box `cols` wide.
    pub fn view(&self, cols: usize) -> (String, usize) {
        if cols == 0 {
            return (String::new(), 0);
        }
        let chars: Vec<char> = self.text.chars().collect();
        let before: String = chars[..self.cursor.min(chars.len())].iter().collect();
        let cursor_col = text::width(&before);
        if cursor_col < cols {
            let mut shown = String::new();
            let mut used = 0;
            for c in &chars {
                let w = unicode_width_of(*c);
                if used + w > cols {
                    break;
                }
                shown.push(*c);
                used += w;
            }
            return (shown, cursor_col);
        }
        // Cursor has run off the right edge: scroll so it sits at the last column.
        let mut start = self.cursor.min(chars.len());
        let mut used = 0;
        while start > 0 {
            let w = unicode_width_of(chars[start - 1]);
            if used + w > cols.saturating_sub(1) {
                break;
            }
            used += w;
            start -= 1;
        }
        let shown: String = chars[start..self.cursor.min(chars.len())].iter().collect();
        let col = text::width(&shown);
        (shown, col)
    }
}

fn unicode_width_of(c: char) -> usize {
    use unicode_width::UnicodeWidthChar;
    c.width().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(s: &str) -> Input {
        let mut i = Input::default();
        i.set(s);
        i
    }

    #[test]
    fn insert_and_backspace_track_cursor() {
        let mut i = Input::default();
        i.insert('a');
        i.insert('b');
        assert_eq!((i.text.as_str(), i.cursor), ("ab", 2));
        i.backspace();
        assert_eq!((i.text.as_str(), i.cursor), ("a", 1));
        i.backspace();
        i.backspace();
        assert_eq!((i.text.as_str(), i.cursor), ("", 0));
    }

    #[test]
    fn insert_at_cursor_not_at_end() {
        let mut i = input("ac");
        i.left();
        i.insert('b');
        assert_eq!(i.text, "abc");
        assert_eq!(i.cursor, 2);
    }

    #[test]
    fn multibyte_editing_does_not_panic() {
        let mut i = input("日本語");
        i.left();
        i.backspace();
        assert_eq!(i.text, "日語");
        i.home();
        i.delete();
        assert_eq!(i.text, "語");
    }

    #[test]
    fn delete_word_removes_previous_word() {
        let mut i = input("hello world  ");
        i.delete_word();
        assert_eq!(i.text, "hello ");
        i.delete_word();
        assert_eq!(i.text, "");
    }

    #[test]
    fn cursor_movement_is_clamped() {
        let mut i = input("ab");
        i.right();
        i.right();
        assert_eq!(i.cursor, 2);
        i.left();
        i.left();
        i.left();
        assert_eq!(i.cursor, 0);
    }

    #[test]
    fn take_trims_and_clears() {
        let mut i = input("  hi  ");
        assert_eq!(i.take(), "hi");
        assert!(i.is_empty());
        assert_eq!(i.cursor, 0);
    }

    #[test]
    fn view_scrolls_when_cursor_passes_the_edge() {
        let mut i = input("abcdefghij");
        let (shown, col) = i.view(4);
        assert!(col < 4, "cursor column {col} must stay inside the box");
        assert!(crate::text::width(&shown) <= 4);

        i.home();
        let (shown, col) = i.view(4);
        assert_eq!(col, 0);
        assert_eq!(shown, "abcd");

        assert_eq!(i.view(0), (String::new(), 0));
    }
}
