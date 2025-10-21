use std::ops::Range;
use lsp_types::Position;
use ropey::{LineType, Rope, RopeSlice};
use sum_tree::Bias;
use tree_sitter::Point;

/// An iterator over the lines of a `Rope`.
pub struct RopeLines<'a> {
    rope: &'a Rope,
    row: usize,
    end_row: usize,
}

impl<'a> RopeLines<'a> {
    /// Create a new `RopeLines` iterator.
    pub fn new(rope: &'a Rope) -> Self {
        let end_row = rope.lines_len();
        Self {
            row: 0,
            end_row,
            rope,
        }
    }
}
impl<'a> Iterator for RopeLines<'a> {
    type Item = RopeSlice<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.end_row {
            return None;
        }

        let line = self.rope.slice_line(self.row);
        self.row += 1;
        Some(line)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.row = self.row.saturating_add(n);
        self.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end_row - self.row;
        (len, Some(len))
    }
}

impl std::iter::ExactSizeIterator for RopeLines<'_> {}
impl std::iter::FusedIterator for RopeLines<'_> {}

/// An extension trait for [`Rope`] to provide additional utility methods.
pub trait RopeExt {
    /// Start offset of the line at the given row (0-based) index.
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    ///
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.line_start_offset(0), 0);
    /// assert_eq!(rope.line_start_offset(1), 6);
    /// ```
    fn line_start_offset(&self, row: usize) -> usize;

    /// Line the end offset (including `\n`) of the line at the given row (0-based) index.
    ///
    /// Return the end of the rope if the row is out of bounds.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.line_end_offset(0), 5); // "Hello\n"
    /// assert_eq!(rope.line_end_offset(1), 12); // "World\r\n"
    /// ```
    fn line_end_offset(&self, row: usize) -> usize;

    /// Return a line slice at the given row (0-based) index. including `\r` if present, but not `\n`.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.slice_line(0).to_string(), "Hello");
    /// assert_eq!(rope.slice_line(1).to_string(), "World\r");
    /// assert_eq!(rope.slice_line(2).to_string(), "This is a test 中文");
    /// assert_eq!(rope.slice_line(6).to_string(), ""); // out of bounds
    /// ```
    fn slice_line(&self, row: usize) -> RopeSlice<'_>;

    /// Return a slice of rows in the given range (0-based, end exclusive).
    ///
    /// If the range is out of bounds, it will be clamped to the valid range.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.slice_lines(0..2).to_string(), "Hello\nWorld\r");
    /// assert_eq!(rope.slice_lines(1..3).to_string(), "World\r\nThis is a test 中文");
    /// assert_eq!(rope.slice_lines(2..5).to_string(), "This is a test 中文\nRope");
    /// assert_eq!(rope.slice_lines(3..10).to_string(), "Rope");
    /// assert_eq!(rope.slice_lines(5..10).to_string(), ""); // out of bounds
    /// ```
    fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_>;

    /// Return an iterator over all lines in the rope.
    ///
    /// Each line slice includes `\r` if present, but not `\n`.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// let lines: Vec<_> = rope.iter_lines().map(|r| r.to_string()).collect();
    /// assert_eq!(lines, vec!["Hello", "World\r", "This is a test 中文", "Rope"]);
    /// ```
    fn iter_lines(&self) -> RopeLines<'_>;

    /// Return the number of lines in the rope.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.lines_len(), 4);
    /// ```
    fn lines_len(&self) -> usize;

    /// Return the length of the row (0-based) in characters, including `\r` if present, but not `\n`.
    ///
    /// If the row is out of bounds, return 0.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// assert_eq!(rope.line_len(0), 5); // "Hello"
    /// assert_eq!(rope.line_len(1), 6); // "World\r"
    /// assert_eq!(rope.line_len(2), 21); // "This is a test 中文"
    /// assert_eq!(rope.line_len(4), 0); // out of bounds
    /// ```
    fn line_len(&self, row: usize) -> usize;

    /// Replace the text in the given byte range with new text.
    ///
    /// # Panics
    ///
    /// - If the range is not on char boundary.
    /// - If the range is out of bounds.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let mut rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    /// rope.replace(6..11, "Universe");
    /// assert_eq!(rope.to_string(), "Hello\nUniverse\r\nThis is a test 中文\nRope");
    /// ```
    fn replace(&mut self, range: Range<usize>, new_text: &str);

    /// Get char at the given offset (byte).
    ///
    /// - If the offset is in the middle of a multi-byte character will panic.
    /// - If the offset is out of bounds, return None.
    fn char_at(&self, offset: usize) -> Option<char>;

    /// Get the byte offset from the given line, column [`Position`] (0-based).
    ///
    /// The column is in characters.
    fn position_to_offset(&self, line_col: &Position) -> usize;

    /// Get the line, column [`Position`] (0-based) from the given byte offset.
    ///
    /// The column is in characters.
    fn offset_to_position(&self, offset: usize) -> Position;

    /// Get point (row, column) from the given byte offset.
    ///
    /// The column is in bytes.
    fn offset_to_point(&self, offset: usize) -> Point;

    /// Get byte offset from the given point (row, column).
    ///
    /// The column is 0-based in bytes.
    fn point_to_offset(&self, point: Point) -> usize;

    /// Get the word byte range at the given byte offset (0-based).
    fn word_range(&self, offset: usize) -> Option<Range<usize>>;

    /// Get word at the given byte offset (0-based).
    fn word_at(&self, offset: usize) -> String;

    /// Convert offset in UTF-16 to byte offset (0-based).
    ///
    /// Runs in O(log N) time.
    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize;

    /// Convert byte offset (0-based) to offset in UTF-16.
    ///
    /// Runs in O(log N) time.
    fn offset_to_offset_utf16(&self, offset: usize) -> usize;

    /// Get a clipped offset (avoid in a char boundary).
    ///
    /// - If Bias::Left and inside the char boundary, return the ix - 1;
    /// - If Bias::Right and in inside char boundary, return the ix + 1;
    /// - Otherwise return the ix.
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// use sum_tree::Bias;
    ///
    /// let rope = Rope::from("Hello 中文🎉 test\nRope");
    /// assert_eq!(rope.clip_offset(5, Bias::Left), 5);
    /// // Inside multi-byte character '中' (3 bytes)
    /// assert_eq!(rope.clip_offset(7, Bias::Left), 6);
    /// assert_eq!(rope.clip_offset(7, Bias::Right), 9);
    /// ```
    fn clip_offset(&self, offset: usize, bias: Bias) -> usize;

    /// Convert offset in characters to byte offset (0-based).
    ///
    /// Run in O(n) time.
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("a 中文🎉 test\nRope");
    /// assert_eq!(rope.char_index_to_offset(0), 0);
    /// assert_eq!(rope.char_index_to_offset(1), 1);
    /// assert_eq!(rope.char_index_to_offset(3), "a 中".len());
    /// assert_eq!(rope.char_index_to_offset(5), "a 中文🎉".len());
    /// ```
    fn char_index_to_offset(&self, char_index: usize) -> usize;

    /// Convert byte offset (0-based) to offset in characters.
    ///
    /// Run in O(n) time.
    ///
    /// # Example
    ///
    /// ```
    /// use gpui_component::{Rope, RopeExt};
    /// let rope = Rope::from("a 中文🎉 test\nRope");
    /// assert_eq!(rope.offset_to_char_index(0), 0);
    /// assert_eq!(rope.offset_to_char_index(1), 1);
    /// assert_eq!(rope.offset_to_char_index(3), 3);
    /// assert_eq!(rope.offset_to_char_index(4), 3);
    /// ```
    fn offset_to_char_index(&self, offset: usize) -> usize;
}

impl RopeExt for Rope {
    fn slice_line(&self, row: usize) -> RopeSlice<'_> {
        let total_lines = self.lines_len();
        if row >= total_lines {
            return self.slice(0..0);
        }

        let line = self.line(row, LineType::LF);
        if line.len() > 0 {
            let line_end = line.len() - 1;
            if line.is_char_boundary(line_end) && line.char(line_end) == '\n' {
                return line.slice(..line_end);
            }
        }

        line
    }

    fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_> {
        let start = self.line_start_offset(rows_range.start);
        let end = self.line_end_offset(rows_range.end.saturating_sub(1));
        self.slice(start..end)
    }

    fn iter_lines(&self) -> RopeLines<'_> {
        RopeLines::new(&self)
    }

    fn line_len(&self, row: usize) -> usize {
        self.slice_line(row).len()
    }

    fn line_start_offset(&self, row: usize) -> usize {
        self.point_to_offset(Point::new(row, 0))
    }

    fn offset_to_point(&self, offset: usize) -> Point {
        let offset = self.clip_offset(offset, Bias::Left);
        let row = self.byte_to_line_idx(offset, LineType::LF);
        let line_start = self.line_to_byte_idx(row, LineType::LF);
        let column = offset.saturating_sub(line_start);
        Point::new(row, column)
    }

    fn point_to_offset(&self, point: Point) -> usize {
        if point.row >= self.lines_len() {
            return self.len();
        }

        let line_start = self.line_to_byte_idx(point.row, LineType::LF);
        line_start + point.column
    }

    fn position_to_offset(&self, pos: &Position) -> usize {
        let line = self.slice_line(pos.line as usize);
        self.line_start_offset(pos.line as usize)
            + line
                .chars()
                .take(pos.character as usize)
                .map(|c| c.len_utf8())
                .sum::<usize>()
    }

    fn offset_to_position(&self, offset: usize) -> Position {
        let point = self.offset_to_point(offset);
        let line = self.slice_line(point.row);
        let offset = line.utf16_to_byte_idx(line.byte_to_utf16_idx(point.column));
        let character = line.slice(..offset).chars().count();
        Position::new(point.row as u32, character as u32)
    }

    fn line_end_offset(&self, row: usize) -> usize {
        if row > self.lines_len() {
            return self.len();
        }

        self.line_start_offset(row) + self.line_len(row)
    }

    fn lines_len(&self) -> usize {
        self.len_lines(LineType::LF)
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        if offset > self.len() {
            return None;
        }

        self.get_char(offset).ok()
    }

    fn word_range(&self, offset: usize) -> Option<Range<usize>> {
        if offset >= self.len() {
            return None;
        }

        let mut left = String::new();
        let offset = self.clip_offset(offset, Bias::Left);
        for c in self.chars_at(offset).reversed() {
            if c.is_alphanumeric() || c == '_' {
                left.insert(0, c);
            } else {
                break;
            }
        }
        let start = offset.saturating_sub(left.len());

        let right = self
            .chars_at(offset)
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();

        let end = offset + right.len();

        if start == end { None } else { Some(start..end) }
    }

    fn word_at(&self, offset: usize) -> String {
        if let Some(range) = self.word_range(offset) {
            self.slice(range).to_string()
        } else {
            String::new()
        }
    }

    #[inline]
    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize {
        if offset_utf16 > self.len_utf16() {
            return self.len();
        }

        self.utf16_to_byte_idx(offset_utf16)
    }

    #[inline]
    fn offset_to_offset_utf16(&self, offset: usize) -> usize {
        if offset > self.len() {
            return self.len_utf16();
        }

        self.byte_to_utf16_idx(offset)
    }

    fn replace(&mut self, range: Range<usize>, new_text: &str) {
        let range =
            self.clip_offset(range.start, Bias::Left)..self.clip_offset(range.end, Bias::Right);
        self.remove(range.clone());
        self.insert(range.start, new_text);
    }

    fn clip_offset(&self, offset: usize, bias: Bias) -> usize {
        if offset > self.len() {
            return self.len();
        }

        if self.is_char_boundary(offset) {
            return offset;
        }

        if bias == Bias::Left {
            self.floor_char_boundary(offset)
        } else {
            self.ceil_char_boundary(offset)
        }
    }

    fn char_index_to_offset(&self, char_offset: usize) -> usize {
        self.chars().take(char_offset).map(|c| c.len_utf8()).sum()
    }

    fn offset_to_char_index(&self, offset: usize) -> usize {
        let offset = self.clip_offset(offset, Bias::Right);
        self.slice(..offset).chars().count()
    }
}
