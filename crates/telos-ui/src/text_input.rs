//! Unified text editing buffer, UTF-8 cursor navigation, word boundaries, selection, and clipboard actions.

use std::ops::Range;

/// Action command for unified text inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputAction {
    /// Move cursor left.
    MoveLeft {
        /// If true, jump word by word (Ctrl+Left).
        word: bool,
        /// If true, extend or shrink the selection (Shift+Left).
        extend: bool,
    },
    /// Move cursor right.
    MoveRight {
        /// If true, jump word by word (Ctrl+Right).
        word: bool,
        /// If true, extend or shrink the selection (Shift+Right).
        extend: bool,
    },
    /// Move cursor to beginning of input (Home).
    MoveStart {
        /// If true, extend selection (Shift+Home).
        extend: bool,
    },
    /// Move cursor to end of input (End).
    MoveEnd {
        /// If true, extend selection (Shift+End).
        extend: bool,
    },
    /// Delete character or selection before cursor (Backspace).
    Backspace {
        /// If true, delete preceding word (Ctrl+Backspace).
        word: bool,
    },
    /// Delete character or selection after cursor (Delete).
    Delete {
        /// If true, delete following word (Ctrl+Delete).
        word: bool,
    },
    /// Select all text (Ctrl+A).
    SelectAll,
    /// Copy selected text (Ctrl+C).
    Copy,
    /// Paste text from clipboard, replacing any selection (Ctrl+V).
    Paste,
}

/// Result of executing a `TextInputAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputResult {
    /// Text, cursor, or selection was modified.
    Changed,
    /// A copy action was executed; contains the selected text to put onto the system clipboard.
    Copied(String),
    /// No change occurred.
    None,
}

/// Standalone text editing buffer holding string contents, cursor position, and optional selection anchor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextInputBuffer {
    /// Text content.
    pub text: String,
    /// Cursor position as a valid UTF-8 byte offset in `text`.
    pub cursor_pos: usize,
    /// Selection anchor index where selection started when holding Shift.
    pub selection_anchor: Option<usize>,
}

impl TextInputBuffer {
    /// Creates a new empty `TextInputBuffer`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `TextInputBuffer` initialized with text.
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        let t = text.into();
        let len = t.len();
        Self {
            text: t,
            cursor_pos: len,
            selection_anchor: None,
        }
    }

    /// Sets the text, resetting cursor to end and clearing selection.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_pos = self.text.len();
        self.selection_anchor = None;
    }

    /// Clears text, cursor, and selection.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = 0;
        self.selection_anchor = None;
    }

    /// Returns the active selection range in byte indices if anchor is present and non-empty.
    #[must_use]
    pub fn selection_range(&self) -> Option<Range<usize>> {
        selection_range(&self.text, self.cursor_pos, self.selection_anchor)
    }

    /// Returns the currently selected slice of text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        self.selection_range().map(|r| &self.text[r])
    }

    /// Inserts a typed character, replacing any active selection.
    pub fn insert_char(&mut self, ch: char) {
        insert_char(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            ch,
        );
    }

    /// Inserts a string (e.g. from clipboard paste), replacing any active selection.
    pub fn insert_str(&mut self, s: &str) {
        insert_str(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            s,
        );
    }

    /// Deletes the currently selected range, returning true if a selection was removed.
    pub fn delete_selection(&mut self) -> bool {
        delete_selection(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
        )
    }

    /// Deletes character before cursor or deletes selection.
    pub fn backspace(&mut self, word: bool) -> bool {
        backspace(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
        )
    }

    /// Deletes character after cursor or deletes selection.
    pub fn delete(&mut self, word: bool) -> bool {
        delete(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
        )
    }

    /// Moves cursor left by single character or word.
    pub fn move_left(&mut self, word: bool, extend: bool) {
        move_left(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
            extend,
        );
    }

    /// Moves cursor right by single character or word.
    pub fn move_right(&mut self, word: bool, extend: bool) {
        move_right(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
            extend,
        );
    }

    /// Moves cursor to start (0).
    pub fn move_start(&mut self, extend: bool) {
        move_start(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend,
        );
    }

    /// Moves cursor to end (`text.len()`).
    pub fn move_end(&mut self, extend: bool) {
        move_end(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend,
        );
    }

    /// Selects all text in the buffer.
    pub fn select_all(&mut self) {
        select_all(&self.text, &mut self.cursor_pos, &mut self.selection_anchor);
    }

    /// Clears any active selection without moving the cursor.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Executes a unified `TextInputAction` command.
    pub fn handle_action(
        &mut self,
        action: TextInputAction,
        clipboard_text: Option<&str>,
    ) -> TextInputResult {
        handle_action(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            action,
            clipboard_text,
        )
    }
}

/// Finds the preceding UTF-8 character boundary before `from`.
#[must_use]
pub fn prev_char_boundary(text: &str, from: usize) -> usize {
    let mut pos = from.min(text.len());
    if pos == 0 {
        return 0;
    }
    pos -= 1;
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Finds the next UTF-8 character boundary after `from`.
#[must_use]
pub fn next_char_boundary(text: &str, from: usize) -> usize {
    let mut pos = from.min(text.len());
    if pos >= text.len() {
        return text.len();
    }
    pos += 1;
    while pos < text.len() && !text.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

/// Computes preceding word boundary for Ctrl+Left navigation.
#[must_use]
pub fn prev_word_boundary(text: &str, from: usize) -> usize {
    let from = from.min(text.len());
    if from == 0 {
        return 0;
    }
    let slice = &text[..from];
    let mut chars = slice.char_indices().rev().peekable();

    // Skip trailing whitespace before cursor
    while let Some(&(_, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }

    if let Some(&(_, first_ch)) = chars.peek() {
        let is_alphanumeric = first_ch.is_alphanumeric() || first_ch == '_';
        let mut boundary = 0;
        while let Some(&(idx, ch)) = chars.peek() {
            if is_alphanumeric {
                if ch.is_alphanumeric() || ch == '_' {
                    boundary = idx;
                    chars.next();
                } else {
                    break;
                }
            } else if !ch.is_whitespace() && !ch.is_alphanumeric() && ch != '_' {
                boundary = idx;
                chars.next();
            } else {
                break;
            }
        }
        boundary
    } else {
        0
    }
}

/// Computes following word boundary for Ctrl+Right navigation.
#[must_use]
pub fn next_word_boundary(text: &str, from: usize) -> usize {
    let from = from.min(text.len());
    if from >= text.len() {
        return text.len();
    }
    let slice = &text[from..];
    let mut chars = slice.char_indices().peekable();

    // Skip whitespace at cursor
    while let Some(&(_, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }

    if let Some(&(_, first_ch)) = chars.peek() {
        let is_alphanumeric = first_ch.is_alphanumeric() || first_ch == '_';
        let mut end_idx = from;
        while let Some(&(rel_idx, ch)) = chars.peek() {
            if is_alphanumeric {
                if ch.is_alphanumeric() || ch == '_' {
                    end_idx = from + rel_idx + ch.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            } else if !ch.is_whitespace() && !ch.is_alphanumeric() && ch != '_' {
                end_idx = from + rel_idx + ch.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        end_idx.min(text.len())
    } else {
        text.len()
    }
}

/// Computes active selection range in byte indices if anchor is set and non-empty.
#[must_use]
pub fn selection_range(
    text: &str,
    cursor_pos: usize,
    selection_anchor: Option<usize>,
) -> Option<Range<usize>> {
    let anchor = selection_anchor?;
    let c = cursor_pos.min(text.len());
    let a = anchor.min(text.len());
    if a == c {
        None
    } else {
        let start = a.min(c);
        let end = a.max(c);
        Some(start..end)
    }
}

/// Deletes the currently selected range, resetting cursor and anchor.
pub fn delete_selection(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
) -> bool {
    if let Some(range) = selection_range(text, *cursor_pos, *selection_anchor) {
        text.drain(range.clone());
        *cursor_pos = range.start;
        *selection_anchor = None;
        true
    } else {
        false
    }
}

/// Inserts a typed character at cursor, replacing any active selection.
pub fn insert_char(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    ch: char,
) {
    if ch.is_control() {
        return;
    }
    delete_selection(text, cursor_pos, selection_anchor);
    *cursor_pos = (*cursor_pos).min(text.len());
    while *cursor_pos > 0 && !text.is_char_boundary(*cursor_pos) {
        *cursor_pos = prev_char_boundary(text, *cursor_pos);
    }
    text.insert(*cursor_pos, ch);
    *cursor_pos += ch.len_utf8();
    *selection_anchor = None;
}

/// Inserts string at cursor, replacing any active selection.
pub fn insert_str(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    s: &str,
) {
    let clean: String = s.chars().filter(|c| !c.is_control()).collect();
    if clean.is_empty() {
        return;
    }
    delete_selection(text, cursor_pos, selection_anchor);
    *cursor_pos = (*cursor_pos).min(text.len());
    while *cursor_pos > 0 && !text.is_char_boundary(*cursor_pos) {
        *cursor_pos = prev_char_boundary(text, *cursor_pos);
    }
    text.insert_str(*cursor_pos, &clean);
    *cursor_pos += clean.len();
    *selection_anchor = None;
}

/// Deletes preceding character or word, or active selection.
pub fn backspace(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    word: bool,
) -> bool {
    if delete_selection(text, cursor_pos, selection_anchor) {
        return true;
    }
    let cur = (*cursor_pos).min(text.len());
    if cur == 0 {
        return false;
    }
    let target = if word {
        prev_word_boundary(text, cur)
    } else {
        prev_char_boundary(text, cur)
    };
    text.drain(target..cur);
    *cursor_pos = target;
    *selection_anchor = None;
    true
}

/// Deletes following character or word, or active selection.
pub fn delete(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    word: bool,
) -> bool {
    if delete_selection(text, cursor_pos, selection_anchor) {
        return true;
    }
    let cur = (*cursor_pos).min(text.len());
    if cur >= text.len() {
        return false;
    }
    let target = if word {
        next_word_boundary(text, cur)
    } else {
        next_char_boundary(text, cur)
    };
    text.drain(cur..target);
    *selection_anchor = None;
    true
}

/// Moves cursor left with optional word jump and selection extension.
pub fn move_left(
    text: &str,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    word: bool,
    extend: bool,
) {
    let cur = (*cursor_pos).min(text.len());
    if extend {
        if selection_anchor.is_none() {
            *selection_anchor = Some(cur);
        }
        let next_pos = if word {
            prev_word_boundary(text, cur)
        } else {
            prev_char_boundary(text, cur)
        };
        *cursor_pos = next_pos;
    } else if let Some(range) = selection_range(text, cur, *selection_anchor) {
        *cursor_pos = range.start;
        *selection_anchor = None;
    } else {
        let next_pos = if word {
            prev_word_boundary(text, cur)
        } else {
            prev_char_boundary(text, cur)
        };
        *cursor_pos = next_pos;
        *selection_anchor = None;
    }
}

/// Moves cursor right with optional word jump and selection extension.
pub fn move_right(
    text: &str,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    word: bool,
    extend: bool,
) {
    let cur = (*cursor_pos).min(text.len());
    if extend {
        if selection_anchor.is_none() {
            *selection_anchor = Some(cur);
        }
        let next_pos = if word {
            next_word_boundary(text, cur)
        } else {
            next_char_boundary(text, cur)
        };
        *cursor_pos = next_pos;
    } else if let Some(range) = selection_range(text, cur, *selection_anchor) {
        *cursor_pos = range.end;
        *selection_anchor = None;
    } else {
        let next_pos = if word {
            next_word_boundary(text, cur)
        } else {
            next_char_boundary(text, cur)
        };
        *cursor_pos = next_pos;
        *selection_anchor = None;
    }
}

/// Moves cursor to start (0) with optional selection extension.
pub fn move_start(
    text: &str,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    extend: bool,
) {
    let cur = (*cursor_pos).min(text.len());
    if extend {
        if selection_anchor.is_none() {
            *selection_anchor = Some(cur);
        }
    } else {
        *selection_anchor = None;
    }
    *cursor_pos = 0;
}

/// Moves cursor to end with optional selection extension.
pub fn move_end(
    text: &str,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    extend: bool,
) {
    let cur = (*cursor_pos).min(text.len());
    if extend {
        if selection_anchor.is_none() {
            *selection_anchor = Some(cur);
        }
    } else {
        *selection_anchor = None;
    }
    *cursor_pos = text.len();
}

/// Selects all text.
pub fn select_all(text: &str, cursor_pos: &mut usize, selection_anchor: &mut Option<usize>) {
    *selection_anchor = Some(0);
    *cursor_pos = text.len();
}

/// Handles a single `TextInputAction` against mutable text, cursor, and selection state.
pub fn handle_action(
    text: &mut String,
    cursor_pos: &mut usize,
    selection_anchor: &mut Option<usize>,
    action: TextInputAction,
    clipboard_text: Option<&str>,
) -> TextInputResult {
    match action {
        TextInputAction::MoveLeft { word, extend } => {
            move_left(text, cursor_pos, selection_anchor, word, extend);
            TextInputResult::Changed
        }
        TextInputAction::MoveRight { word, extend } => {
            move_right(text, cursor_pos, selection_anchor, word, extend);
            TextInputResult::Changed
        }
        TextInputAction::MoveStart { extend } => {
            move_start(text, cursor_pos, selection_anchor, extend);
            TextInputResult::Changed
        }
        TextInputAction::MoveEnd { extend } => {
            move_end(text, cursor_pos, selection_anchor, extend);
            TextInputResult::Changed
        }
        TextInputAction::Backspace { word } => {
            if backspace(text, cursor_pos, selection_anchor, word) {
                TextInputResult::Changed
            } else {
                TextInputResult::None
            }
        }
        TextInputAction::Delete { word } => {
            if delete(text, cursor_pos, selection_anchor, word) {
                TextInputResult::Changed
            } else {
                TextInputResult::None
            }
        }
        TextInputAction::SelectAll => {
            select_all(text, cursor_pos, selection_anchor);
            TextInputResult::Changed
        }
        TextInputAction::Copy => {
            if let Some(range) = selection_range(text, *cursor_pos, *selection_anchor) {
                TextInputResult::Copied(text[range].to_string())
            } else {
                TextInputResult::None
            }
        }
        TextInputAction::Paste => {
            if let Some(clip) = clipboard_text.filter(|c| !c.is_empty()) {
                insert_str(text, cursor_pos, selection_anchor, clip);
                TextInputResult::Changed
            } else {
                TextInputResult::None
            }
        }
    }
}
