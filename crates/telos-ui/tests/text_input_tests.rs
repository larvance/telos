//! Unit and integration tests for unified text input navigation, selection, and clipboard actions.

use telos_ui::chat::ChatHudState;
use telos_ui::menu::widgets::MenuTextInput;
use telos_ui::text_input::{
    TextInputAction, TextInputBuffer, TextInputResult,
    next_char_boundary, next_word_boundary, prev_char_boundary, prev_word_boundary,
};

#[test]
fn test_char_boundaries_utf8() {
    let s = "A🦀B世界C";
    // Byte offsets:
    // A: [0..1]
    // 🦀: [1..5]
    // B: [5..6]
    // 世: [6..9]
    // 界: [9..12]
    // C: [12..13]
    assert_eq!(next_char_boundary(s, 0), 1);
    assert_eq!(next_char_boundary(s, 1), 5);
    assert_eq!(next_char_boundary(s, 2), 5); // inside crab
    assert_eq!(next_char_boundary(s, 5), 6);
    assert_eq!(next_char_boundary(s, 6), 9);
    assert_eq!(next_char_boundary(s, 9), 12);
    assert_eq!(next_char_boundary(s, 12), 13);
    assert_eq!(next_char_boundary(s, 13), 13);

    assert_eq!(prev_char_boundary(s, 13), 12);
    assert_eq!(prev_char_boundary(s, 12), 9);
    assert_eq!(prev_char_boundary(s, 9), 6);
    assert_eq!(prev_char_boundary(s, 6), 5);
    assert_eq!(prev_char_boundary(s, 5), 1);
    assert_eq!(prev_char_boundary(s, 3), 1); // inside crab
    assert_eq!(prev_char_boundary(s, 1), 0);
    assert_eq!(prev_char_boundary(s, 0), 0);
}

#[test]
fn test_word_boundaries() {
    let s = "hello world_123  foo.bar";
    // Test prev_word_boundary
    assert_eq!(prev_word_boundary(s, 5), 0);
    assert_eq!(prev_word_boundary(s, 11), 6); // "world_123"
    assert_eq!(prev_word_boundary(s, 15), 6);
    assert_eq!(prev_word_boundary(s, 17), 6); // from spaces
    assert_eq!(prev_word_boundary(s, 20), 17); // "foo"
    assert_eq!(prev_word_boundary(s, 24), 21); // "bar"

    // Test next_word_boundary
    assert_eq!(next_word_boundary(s, 0), 5); // "hello"
    assert_eq!(next_word_boundary(s, 5), 15); // " world_123" -> 15
    assert_eq!(next_word_boundary(s, 15), 20); // "  foo" -> 20
    assert_eq!(next_word_boundary(s, 20), 21); // "." -> 21
    assert_eq!(next_word_boundary(s, 21), 24); // "bar" -> 24
}

#[test]
fn test_buffer_left_right_navigation() {
    let mut buf = TextInputBuffer::with_text("Rust 🦀 test");
    assert_eq!(buf.cursor_pos, 14);

    // Move left character by character
    buf.move_left(false, false); // after 's' in test
    assert_eq!(buf.cursor_pos, 13);

    buf.move_left(false, false);
    buf.move_left(false, false);
    buf.move_left(false, false); // before 't' in test
    assert_eq!(buf.cursor_pos, 10);

    buf.move_left(false, false); // before space
    assert_eq!(buf.cursor_pos, 9);

    // Across UTF-8 multi-byte emoji 🦀 (4 bytes: 5..9)
    buf.move_left(false, false);
    assert_eq!(buf.cursor_pos, 5);

    // Move right across emoji
    buf.move_right(false, false);
    assert_eq!(buf.cursor_pos, 9);
}

#[test]
fn test_buffer_ctrl_word_navigation() {
    let mut buf = TextInputBuffer::with_text("first second third");
    assert_eq!(buf.cursor_pos, 18);

    // Ctrl + Left jumps to beginning of "third"
    buf.move_left(true, false);
    assert_eq!(buf.cursor_pos, 13);

    // Ctrl + Left jumps to beginning of "second"
    buf.move_left(true, false);
    assert_eq!(buf.cursor_pos, 6);

    // Ctrl + Left jumps to beginning of "first"
    buf.move_left(true, false);
    assert_eq!(buf.cursor_pos, 0);

    // Clamp at 0
    buf.move_left(true, false);
    assert_eq!(buf.cursor_pos, 0);

    // Ctrl + Right jumps to end of "first"
    buf.move_right(true, false);
    assert_eq!(buf.cursor_pos, 5);

    // Ctrl + Right jumps to end of "second"
    buf.move_right(true, false);
    assert_eq!(buf.cursor_pos, 12);

    // Ctrl + Right jumps to end of "third"
    buf.move_right(true, false);
    assert_eq!(buf.cursor_pos, 18);

    // Clamp at text length
    buf.move_right(true, false);
    assert_eq!(buf.cursor_pos, 18);
}

#[test]
fn test_shift_selection_and_collapse() {
    let mut buf = TextInputBuffer::with_text("hello world");
    buf.cursor_pos = 0;

    // Shift + Right x5 to select "hello"
    for _ in 0..5 {
        buf.move_right(false, true);
    }
    assert_eq!(buf.cursor_pos, 5);
    assert_eq!(buf.selection_anchor, Some(0));
    assert_eq!(buf.selection_range(), Some(0..5));
    assert_eq!(buf.selected_text(), Some("hello"));

    // Moving left WITHOUT shift collapses selection to start of selection
    buf.move_left(false, false);
    assert_eq!(buf.cursor_pos, 0);
    assert_eq!(buf.selection_anchor, None);
    assert_eq!(buf.selection_range(), None);

    // Select "world" with Shift + Left from end
    buf.cursor_pos = 11;
    for _ in 0..5 {
        buf.move_left(false, true);
    }
    assert_eq!(buf.cursor_pos, 6);
    assert_eq!(buf.selection_anchor, Some(11));
    assert_eq!(buf.selection_range(), Some(6..11));
    assert_eq!(buf.selected_text(), Some("world"));

    // Moving right WITHOUT shift collapses selection to end of selection
    buf.move_right(false, false);
    assert_eq!(buf.cursor_pos, 11);
    assert_eq!(buf.selection_anchor, None);
    assert_eq!(buf.selection_range(), None);
}

#[test]
fn test_ctrl_shift_word_selection() {
    let mut buf = TextInputBuffer::with_text("one two three");
    buf.cursor_pos = 0;

    // Ctrl + Shift + Right selects "one"
    buf.move_right(true, true);
    assert_eq!(buf.cursor_pos, 3);
    assert_eq!(buf.selected_text(), Some("one"));

    // Ctrl + Shift + Right extends selection across " two"
    buf.move_right(true, true);
    assert_eq!(buf.cursor_pos, 7);
    assert_eq!(buf.selected_text(), Some("one two"));
}

#[test]
fn test_backspace_and_delete_with_and_without_selection() {
    // Backspace single char
    let mut buf = TextInputBuffer::with_text("abc");
    assert!(buf.backspace(false));
    assert_eq!(buf.text, "ab");
    assert_eq!(buf.cursor_pos, 2);

    // Delete single char
    buf.cursor_pos = 0;
    assert!(buf.delete(false));
    assert_eq!(buf.text, "b");
    assert_eq!(buf.cursor_pos, 0);

    // Ctrl + Backspace (delete word)
    let mut buf2 = TextInputBuffer::with_text("hello world");
    assert!(buf2.backspace(true));
    assert_eq!(buf2.text, "hello ");
    assert_eq!(buf2.cursor_pos, 6);

    // Ctrl + Delete (delete word)
    buf2.cursor_pos = 0;
    assert!(buf2.delete(true));
    assert_eq!(buf2.text, " ");
    assert_eq!(buf2.cursor_pos, 0);

    // Backspace active selection
    let mut buf3 = TextInputBuffer::with_text("say hello friend");
    buf3.cursor_pos = 4;
    buf3.move_right(true, true); // select "hello"
    assert_eq!(buf3.selected_text(), Some("hello"));
    assert!(buf3.backspace(false));
    assert_eq!(buf3.text, "say  friend");
    assert_eq!(buf3.cursor_pos, 4);
    assert_eq!(buf3.selection_anchor, None);

    // Delete active selection
    let mut buf4 = TextInputBuffer::with_text("foo bar baz");
    buf4.cursor_pos = 4;
    buf4.move_right(true, true); // select "bar"
    assert_eq!(buf4.selected_text(), Some("bar"));
    assert!(buf4.delete(false));
    assert_eq!(buf4.text, "foo  baz");
    assert_eq!(buf4.cursor_pos, 4);
    assert_eq!(buf4.selection_anchor, None);
}

#[test]
fn test_typing_replaces_selection() {
    let mut buf = TextInputBuffer::with_text("12345");
    buf.select_all();
    assert_eq!(buf.selected_text(), Some("12345"));

    // Typing a character replaces selected text
    buf.insert_char('9');
    assert_eq!(buf.text, "9");
    assert_eq!(buf.cursor_pos, 1);
    assert_eq!(buf.selection_anchor, None);

    // Typing a string replaces partial selection
    let mut buf2 = TextInputBuffer::with_text("the quick fox");
    buf2.cursor_pos = 4;
    buf2.move_right(true, true); // "quick"
    buf2.insert_str("slow");
    assert_eq!(buf2.text, "the slow fox");
    assert_eq!(buf2.cursor_pos, 8);
    assert_eq!(buf2.selection_anchor, None);
}

#[test]
fn test_actions_select_all_copy_paste() {
    let mut buf = TextInputBuffer::with_text("sample content");

    // Ctrl + A (SelectAll)
    let res = buf.handle_action(TextInputAction::SelectAll, None);
    assert_eq!(res, TextInputResult::Changed);
    assert_eq!(buf.selected_text(), Some("sample content"));

    // Ctrl + C (Copy)
    let copy_res = buf.handle_action(TextInputAction::Copy, None);
    assert_eq!(
        copy_res,
        TextInputResult::Copied("sample content".to_string())
    );

    // Ctrl + V (Paste)
    let paste_res = buf.handle_action(TextInputAction::Paste, Some("replaced text"));
    assert_eq!(paste_res, TextInputResult::Changed);
    assert_eq!(buf.text, "replaced text");
    assert_eq!(buf.cursor_pos, 13);
    assert_eq!(buf.selection_anchor, None);
}

#[test]
fn test_home_and_end_with_selection() {
    let mut buf = TextInputBuffer::with_text("first line of text");
    buf.cursor_pos = 10;

    // Shift + Home selects from 10 to 0
    buf.move_start(true);
    assert_eq!(buf.cursor_pos, 0);
    assert_eq!(buf.selected_text(), Some("first line"));

    // Shift + End extends selection from anchor (10) to end
    buf.move_end(true);
    assert_eq!(buf.cursor_pos, 18);
    assert_eq!(buf.selected_text(), Some(" of text"));

    // End without shift moves to end and clears selection
    buf.move_end(false);
    assert_eq!(buf.cursor_pos, 18);
    assert_eq!(buf.selection_anchor, None);

    // Home without shift moves to start and clears selection
    buf.move_start(false);
    assert_eq!(buf.cursor_pos, 0);
    assert_eq!(buf.selection_anchor, None);
}

#[test]
fn test_menu_text_input_integration() {
    let mut input = MenuTextInput::new(1, 0.0, 0.0, 200.0, 20.0, "Placeholder");
    input.text = "127.0.0.1:25565".to_string();
    input.cursor_pos = 15;
    assert_eq!(input.text, "127.0.0.1:25565");
    assert_eq!(input.cursor_pos, 15);

    // Select all
    input.handle_action(TextInputAction::SelectAll, None);
    assert_eq!(input.selected_text(), Some("127.0.0.1:25565"));

    // Paste replacement
    input.handle_action(TextInputAction::Paste, Some("localhost:8080"));
    assert_eq!(input.text, "localhost:8080");
    assert_eq!(input.cursor_pos, 14);
    assert_eq!(input.selection_anchor, None);

    // Move left word
    input.handle_action(
        TextInputAction::MoveLeft {
            word: true,
            extend: false,
        },
        None,
    );
    assert_eq!(input.cursor_pos, 10); // after ':'

    // Move left across ':'
    input.handle_action(
        TextInputAction::MoveLeft {
            word: false,
            extend: false,
        },
        None,
    );
    assert_eq!(input.cursor_pos, 9); // before ':'

    // Shift+Left across "localhost"
    input.handle_action(
        TextInputAction::MoveLeft {
            word: true,
            extend: true,
        },
        None,
    );
    assert_eq!(input.cursor_pos, 0);
    assert_eq!(input.selected_text(), Some("localhost"));

    // Backspace deletes selected "localhost"
    input.backspace();
    assert_eq!(input.text, ":8080");
    assert_eq!(input.cursor_pos, 0);
}

#[test]
fn test_chat_hud_state_text_actions() {
    let mut chat = ChatHudState::new();
    chat.open(Some("/gamemode survival"));
    assert_eq!(chat.input_buffer, "/gamemode survival");
    assert_eq!(chat.cursor_pos, 18);

    // Select all and copy
    chat.handle_action(TextInputAction::SelectAll, None);
    let copy_result = chat.handle_action(TextInputAction::Copy, None);
    assert_eq!(
        copy_result,
        TextInputResult::Copied("/gamemode survival".to_string())
    );

    // Ctrl + Backspace deletes entire selection
    chat.handle_action(TextInputAction::Backspace { word: true }, None);
    assert_eq!(chat.input_buffer, "");
    assert_eq!(chat.cursor_pos, 0);

    // Paste into empty chat
    chat.handle_action(TextInputAction::Paste, Some("/tp 0 100 0"));
    assert_eq!(chat.input_buffer, "/tp 0 100 0");
    assert_eq!(chat.cursor_pos, 11);
}
