//! In-game chat HUD overlay, text input editing, message fading, and auto-complete popup.

use crate::font::BitmapFont;
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;
use crate::text_input::{self, TextInputAction, TextInputResult};

/// Maximum number of messages preserved in the chat log.
pub const MAX_CHAT_HISTORY: usize = 100;

/// Duration in seconds during which a chat message is displayed at full opacity.
pub const CHAT_DISPLAY_DURATION_SECS: f64 = 5.0;

/// Duration in seconds over which a chat message fades out after the display duration.
pub const CHAT_FADE_DURATION_SECS: f64 = 1.0;

/// A single chat log message entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatEntry {
    /// Sender username or `[System]`.
    pub sender: String,
    /// Raw message body.
    pub message: String,
    /// Formatted line: `<Sender> Message` or `[System] Message`.
    pub formatted: String,
    /// Base text color (RGBA8 little-endian).
    pub color: u32,
    /// Monotonic timestamp when the message was received.
    pub timestamp_secs: f64,
}

impl ChatEntry {
    /// Creates a new chat entry.
    #[must_use]
    pub fn new(
        sender: impl Into<String>,
        message: impl Into<String>,
        color: u32,
        timestamp_secs: f64,
    ) -> Self {
        let sender_str = sender.into();
        let message_str = message.into();
        let formatted = if sender_str.is_empty() || sender_str == "System" || sender_str == "Server"
        {
            message_str.clone()
        } else {
            format!("<{sender_str}> {message_str}")
        };

        Self {
            sender: sender_str,
            message: message_str,
            formatted,
            color,
            timestamp_secs,
        }
    }
}

/// In-game chat HUD state containing message history, open status, text input, and suggestions.
#[derive(Debug, Clone)]
pub struct ChatHudState {
    /// Whether the chat input bar is currently open and capturing keyboard focus.
    pub is_open: bool,
    /// Current input string in the text box.
    pub input_buffer: String,
    /// Cursor position as a UTF-8 byte offset in `input_buffer`.
    pub cursor_pos: usize,
    /// Selection anchor index where selection started when holding Shift.
    pub selection_anchor: Option<usize>,
    /// Received chat message history (oldest first, newest at the end).
    pub history: Vec<ChatEntry>,
    /// Previously sent commands and messages for Up/Down arrow recall.
    pub sent_history: Vec<String>,
    /// Active index within `sent_history` during history navigation.
    pub sent_history_index: Option<usize>,
    /// Auto-completion suggestion candidates currently active.
    pub suggestions: Vec<String>,
    /// Index of currently selected suggestion candidate, if any.
    pub selected_suggestion: Option<usize>,
    /// Start character index in `input_buffer` to replace when accepting a suggestion.
    pub suggestion_start: usize,
    /// Length of character slice in `input_buffer` to replace.
    pub suggestion_len: usize,
}

impl Default for ChatHudState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatHudState {
    /// Creates a new empty `ChatHudState`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_open: false,
            input_buffer: String::new(),
            cursor_pos: 0,
            selection_anchor: None,
            history: Vec::new(),
            sent_history: Vec::new(),
            sent_history_index: None,
            suggestions: Vec::new(),
            selected_suggestion: None,
            suggestion_start: 0,
            suggestion_len: 0,
        }
    }

    /// Opens the chat input box, optionally pre-populating with a prefix (e.g. `'/'`).
    pub fn open(&mut self, prefix: Option<&str>) {
        self.is_open = true;
        self.input_buffer.clear();
        if let Some(p) = prefix {
            self.input_buffer.push_str(p);
        }
        self.cursor_pos = self.input_buffer.len();
        self.selection_anchor = None;
        self.sent_history_index = None;
        self.clear_suggestions();
    }

    /// Closes the chat input box and resets input state.
    pub fn close(&mut self) {
        self.is_open = false;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.selection_anchor = None;
        self.sent_history_index = None;
        self.clear_suggestions();
    }

    /// Appends a new chat message to history, maintaining maximum history capacity.
    pub fn add_message(
        &mut self,
        sender: impl Into<String>,
        message: impl Into<String>,
        now_secs: f64,
    ) {
        let entry = ChatEntry::new(sender, message, UiQuad::rgba(255, 255, 255, 255), now_secs);
        self.history.push(entry);
        if self.history.len() > MAX_CHAT_HISTORY {
            self.history.remove(0);
        }
    }

    /// Appends a system notification message (e.g. command feedback or error).
    pub fn add_system_message(&mut self, message: impl Into<String>, color: u32, now_secs: f64) {
        let entry = ChatEntry::new("System", message, color, now_secs);
        self.history.push(entry);
        if self.history.len() > MAX_CHAT_HISTORY {
            self.history.remove(0);
        }
    }

    /// Returns the active selection range in byte indices if anchor is present and non-empty.
    #[must_use]
    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        text_input::selection_range(&self.input_buffer, self.cursor_pos, self.selection_anchor)
    }

    /// Returns the currently selected slice of text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        self.selection_range().map(|r| &self.input_buffer[r])
    }

    /// Inserts a typed character at the current cursor position, replacing any selection.
    pub fn handle_char(&mut self, c: char) {
        if !c.is_control() {
            text_input::insert_char(
                &mut self.input_buffer,
                &mut self.cursor_pos,
                &mut self.selection_anchor,
                c,
            );
            self.clear_suggestions();
        }
    }

    /// Inserts a string at the current cursor position, replacing any selection.
    pub fn handle_str(&mut self, s: &str) {
        text_input::insert_str(
            &mut self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            s,
        );
        self.clear_suggestions();
    }

    /// Handles the Backspace key by deleting preceding character/word or active selection.
    pub fn handle_backspace(&mut self) {
        self.handle_backspace_mode(false);
    }

    /// Handles Backspace with optional word deletion (Ctrl+Backspace).
    pub fn handle_backspace_mode(&mut self, word: bool) {
        if text_input::backspace(
            &mut self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
        ) {
            self.clear_suggestions();
        }
    }

    /// Handles the Delete key by removing following character/word or active selection.
    pub fn handle_delete(&mut self) {
        self.handle_delete_mode(false);
    }

    /// Handles Delete with optional word deletion (Ctrl+Delete).
    pub fn handle_delete_mode(&mut self, word: bool) {
        if text_input::delete(
            &mut self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
        ) {
            self.clear_suggestions();
        }
    }

    /// Moves the cursor one character to the left.
    pub fn move_cursor_left(&mut self) {
        self.move_left(false, false);
    }

    /// Moves the cursor one character to the right.
    pub fn move_cursor_right(&mut self) {
        self.move_right(false, false);
    }

    /// Moves cursor left with optional word jump and selection extension.
    pub fn move_left(&mut self, word: bool, extend: bool) {
        text_input::move_left(
            &self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
            extend,
        );
    }

    /// Moves cursor right with optional word jump and selection extension.
    pub fn move_right(&mut self, word: bool, extend: bool) {
        text_input::move_right(
            &self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            word,
            extend,
        );
    }

    /// Moves the cursor to the beginning of the input string.
    pub fn move_cursor_start(&mut self) {
        self.move_start(false);
    }

    /// Moves cursor to beginning of input with optional selection extension.
    pub fn move_start(&mut self, extend: bool) {
        text_input::move_start(
            &self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend,
        );
    }

    /// Moves the cursor to the end of the input string.
    pub fn move_cursor_end(&mut self) {
        self.move_end(false);
    }

    /// Moves cursor to end of input with optional selection extension.
    pub fn move_end(&mut self, extend: bool) {
        text_input::move_end(
            &self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend,
        );
    }

    /// Selects all text in the chat input buffer.
    pub fn select_all(&mut self) {
        text_input::select_all(
            &self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
        );
    }

    /// Clears any active selection without moving the cursor.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Handles a unified `TextInputAction` event against the chat buffer.
    pub fn handle_action(
        &mut self,
        action: TextInputAction,
        clipboard_text: Option<&str>,
    ) -> TextInputResult {
        let res = text_input::handle_action(
            &mut self.input_buffer,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            action,
            clipboard_text,
        );
        if res == TextInputResult::Changed {
            self.clear_suggestions();
        }
        res
    }

    /// Navigates older in sent command history.
    pub fn history_up(&mut self) {
        if self.sent_history.is_empty() {
            return;
        }

        let next_idx = match self.sent_history_index {
            None => self.sent_history.len().saturating_sub(1),
            Some(idx) => idx.saturating_sub(1),
        };

        self.sent_history_index = Some(next_idx);
        self.input_buffer = self.sent_history[next_idx].clone();
        self.cursor_pos = self.input_buffer.len();
        self.clear_suggestions();
    }

    /// Navigates newer in sent command history.
    pub fn history_down(&mut self) {
        if let Some(idx) = self.sent_history_index {
            if idx + 1 < self.sent_history.len() {
                let next_idx = idx + 1;
                self.sent_history_index = Some(next_idx);
                self.input_buffer = self.sent_history[next_idx].clone();
                self.cursor_pos = self.input_buffer.len();
            } else {
                self.sent_history_index = None;
                self.input_buffer.clear();
                self.cursor_pos = 0;
            }
            self.clear_suggestions();
        }
    }

    /// Updates auto-complete candidates.
    pub fn set_suggestions(&mut self, start: usize, len: usize, candidates: Vec<String>) {
        self.suggestion_start = start;
        self.suggestion_len = len;
        self.suggestions = candidates;
        self.selected_suggestion = if self.suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Clears any active suggestions.
    pub fn clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.selected_suggestion = None;
        self.suggestion_start = 0;
        self.suggestion_len = 0;
    }

    /// Selects next suggestion in list (wrapping).
    pub fn select_next_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        self.selected_suggestion = Some(match self.selected_suggestion {
            None => 0,
            Some(idx) => (idx + 1) % self.suggestions.len(),
        });
    }

    /// Selects previous suggestion in list (wrapping).
    pub fn select_prev_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        self.selected_suggestion = Some(match self.selected_suggestion {
            None | Some(0) => self.suggestions.len().saturating_sub(1),
            Some(idx) => idx - 1,
        });
    }

    /// Applies the currently selected suggestion into the input buffer.
    pub fn apply_selected_suggestion(&mut self) -> bool {
        let Some(idx) = self.selected_suggestion else {
            return false;
        };
        let Some(candidate) = self.suggestions.get(idx) else {
            return false;
        };

        let start = self.suggestion_start.min(self.input_buffer.len());
        let end = (start + self.suggestion_len).min(self.input_buffer.len());

        let mut new_buf = String::new();
        new_buf.push_str(&self.input_buffer[..start]);
        new_buf.push_str(candidate);
        new_buf.push_str(&self.input_buffer[end..]);

        self.input_buffer = new_buf;
        self.cursor_pos = start + candidate.len();
        self.clear_suggestions();
        true
    }

    /// Finalizes the current input, adds to sent history, closes chat, and returns submitted string.
    pub fn take_submitted_message(&mut self) -> Option<String> {
        let trimmed = self.input_buffer.trim().to_string();
        self.close();

        if trimmed.is_empty() {
            None
        } else {
            self.sent_history.push(trimmed.clone());
            if self.sent_history.len() > 100 {
                self.sent_history.remove(0);
            }
            Some(trimmed)
        }
    }
}

/// Renders the in-game chat HUD into the output `UiQuad` buffer.
#[allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
pub fn render_chat_hud(
    state: &ChatHudState,
    font: &BitmapFont,
    screen_w: u32,
    screen_h: u32,
    gui_scale: u32,
    now_secs: f64,
    out: &mut Vec<UiQuad>,
) {
    let scale = gui_scale.max(1);
    let line_height_gui = 9.0;

    // Chat bottom anchor is 48 GUI pixels above the bottom of the screen (safely above HUD hotbar & health hearts)
    let chat_bottom_gui = (screen_h as f32 / scale as f32) - 48.0;
    let chat_x_gui = 4.0;

    // 1. Render message history lines
    let max_lines = if state.is_open { 20 } else { 10 };
    let start_idx = state.history.len().saturating_sub(max_lines);
    let visible_entries = &state.history[start_idx..];

    let mut current_y_gui = chat_bottom_gui;

    for entry in visible_entries.iter().rev() {
        let age = now_secs - entry.timestamp_secs;
        if age < 0.0 {
            continue;
        }

        // Compute opacity fade factor (1.0 for <=5s, fading to 0.0 at 6s)
        let alpha = if state.is_open || age < CHAT_DISPLAY_DURATION_SECS {
            1.0f32
        } else if age < CHAT_DISPLAY_DURATION_SECS + CHAT_FADE_DURATION_SECS {
            1.0f32 - ((age - CHAT_DISPLAY_DURATION_SECS) / CHAT_FADE_DURATION_SECS) as f32
        } else {
            0.0f32
        };

        if alpha <= 0.0 {
            continue;
        }

        current_y_gui -= line_height_gui;

        let (text_w, _) = font.measure_text(&entry.formatted);
        let bg_w = (text_w + 4.0) * scale as f32;
        let bg_h = line_height_gui * scale as f32;

        let bg_x = snap_to_physical(chat_x_gui, scale);
        let bg_y = snap_to_physical(current_y_gui, scale);

        // Background box
        let bg_alpha = (128.0 * alpha).round() as u8;
        out.push(UiQuad::solid(
            [bg_x, bg_y],
            [bg_w.round() as u16, bg_h.round() as u16],
            UiQuad::rgba(0, 0, 0, bg_alpha),
        ));

        // Text
        let text_alpha = (255.0 * alpha).round() as u8;
        let r = (entry.color & 0xFF) as u8;
        let g = ((entry.color >> 8) & 0xFF) as u8;
        let b = ((entry.color >> 16) & 0xFF) as u8;
        let text_color = UiQuad::rgba(r, g, b, text_alpha);

        font.layout_text(
            &entry.formatted,
            chat_x_gui + 2.0,
            current_y_gui + 1.0,
            text_color,
            true,
            scale,
            out,
        );
    }

    // 2. Render input box and cursor when open
    if state.is_open {
        let input_y_gui = (screen_h as f32 / scale as f32) - 14.0;
        let input_w_gui = 320.0f32.min((screen_w as f32 / scale as f32) - 8.0);
        let input_h_gui = 12.0f32;

        let phys_x = snap_to_physical(chat_x_gui, scale);
        let phys_y = snap_to_physical(input_y_gui, scale);
        let phys_w = (input_w_gui * scale as f32).round() as u16;
        let phys_h = (input_h_gui * scale as f32).round() as u16;

        // Dark background box (clean, semi-transparent, no harsh outline borders colliding with HUD)
        out.push(UiQuad::solid(
            [phys_x, phys_y],
            [phys_w, phys_h],
            UiQuad::rgba(0, 0, 0, 160),
        ));

        // Prompt symbol
        font.layout_text(
            ">",
            chat_x_gui + 2.0,
            input_y_gui + 2.0,
            UiQuad::rgba(255, 255, 85, 255),
            true,
            scale,
            out,
        );

        // Input text
        let text_offset_x = chat_x_gui + 10.0;

        // Render selection highlight box behind text if active
        if let Some(range) = state.selection_range() {
            let (prefix_w, _) = font.measure_text(&state.input_buffer[..range.start]);
            let (sel_w, _) = font.measure_text(&state.input_buffer[range.start..range.end]);
            let sel_x = text_offset_x + prefix_w;
            let px_sel_x = snap_to_physical(sel_x, scale);
            let px_sel_y = snap_to_physical(input_y_gui + 1.0, scale);
            let px_sel_w = snap_to_physical(sel_w, scale) as u16;
            let px_sel_h = snap_to_physical(10.0, scale) as u16;

            out.push(UiQuad::solid(
                [px_sel_x, px_sel_y],
                [px_sel_w, px_sel_h],
                UiQuad::rgba(40, 110, 200, 180),
            ));
        }

        font.layout_text(
            &state.input_buffer,
            text_offset_x,
            input_y_gui + 2.0,
            UiQuad::rgba(255, 255, 255, 255),
            true,
            scale,
            out,
        );

        // Blinking cursor
        let blink = (now_secs * 2.0).fract() < 0.5;
        if blink {
            let cursor_clamped = state.cursor_pos.min(state.input_buffer.len());
            let (cursor_x_offset, _) = font.measure_text(&state.input_buffer[..cursor_clamped]);
            let cursor_phys_x = snap_to_physical(text_offset_x + cursor_x_offset, scale);
            let cursor_phys_y = snap_to_physical(input_y_gui + 2.0, scale);
            let cursor_h = (9.0 * scale as f32).round() as u16;
            let cursor_w = (scale as u16).max(1);

            out.push(UiQuad::solid(
                [cursor_phys_x, cursor_phys_y],
                [cursor_w, cursor_h],
                UiQuad::rgba(255, 255, 255, 255),
            ));
        }

        // 3. Render auto-complete suggestion popup box
        if !state.suggestions.is_empty() {
            let max_shown = 8.min(state.suggestions.len());
            let popup_item_h_gui = 10.0f32;
            let popup_h_gui = (max_shown as f32 * popup_item_h_gui) + 4.0;
            let popup_w_gui = 160.0f32;
            let popup_y_gui = input_y_gui - popup_h_gui - 2.0;

            let pop_x = snap_to_physical(chat_x_gui + 10.0, scale);
            let pop_y = snap_to_physical(popup_y_gui, scale);
            let pop_w = (popup_w_gui * scale as f32).round() as u16;
            let pop_h = (popup_h_gui * scale as f32).round() as u16;

            // Popup background and border
            out.push(UiQuad::solid(
                [pop_x, pop_y],
                [pop_w, pop_h],
                UiQuad::rgba(20, 20, 20, 230),
            ));

            for (i, candidate) in state.suggestions.iter().take(max_shown).enumerate() {
                let item_y_gui = popup_y_gui + 2.0 + (i as f32 * popup_item_h_gui);
                let is_selected = state.selected_suggestion == Some(i);

                if is_selected {
                    let item_phys_y = snap_to_physical(item_y_gui - 1.0, scale);
                    let item_phys_h = (popup_item_h_gui * scale as f32).round() as u16;
                    out.push(UiQuad::solid(
                        [pop_x, item_phys_y],
                        [pop_w, item_phys_h],
                        UiQuad::rgba(50, 70, 120, 255),
                    ));
                }

                let text_color = if is_selected {
                    UiQuad::rgba(255, 255, 255, 255)
                } else {
                    UiQuad::rgba(200, 200, 200, 255)
                };

                font.layout_text(
                    candidate,
                    chat_x_gui + 14.0,
                    item_y_gui,
                    text_color,
                    true,
                    scale,
                    out,
                );
            }
        }
    }
}
