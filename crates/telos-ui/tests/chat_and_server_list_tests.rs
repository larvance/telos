//! Unit and integration tests for telos-ui chat HUD and multiplayer server browser.

use telos_ui::chat::{ChatHudState, MAX_CHAT_HISTORY, render_chat_hud};
use telos_ui::font::BitmapFont;
use telos_ui::server_list::{ServerEntry, ServerListScreen, render_server_list};

#[test]
fn test_chat_state_text_editing() {
    let mut chat = ChatHudState::new();
    assert!(!chat.is_open);

    // Open with prefix '/'
    chat.open(Some("/"));
    assert!(chat.is_open);
    assert_eq!(chat.input_buffer, "/");
    assert_eq!(chat.cursor_pos, 1);

    // Type "time set "
    for c in "time set ".chars() {
        chat.handle_char(c);
    }
    assert_eq!(chat.input_buffer, "/time set ");
    assert_eq!(chat.cursor_pos, 10);

    // Backspace twice
    chat.handle_backspace();
    chat.handle_backspace();
    assert_eq!(chat.input_buffer, "/time se");
    assert_eq!(chat.cursor_pos, 8);

    // Cursor navigation
    chat.move_cursor_left();
    chat.move_cursor_left();
    assert_eq!(chat.cursor_pos, 6);
    chat.handle_char('x');
    assert_eq!(chat.input_buffer, "/time xse");

    chat.move_cursor_start();
    assert_eq!(chat.cursor_pos, 0);

    chat.move_cursor_end();
    assert_eq!(chat.cursor_pos, 9);
}

#[test]
fn test_chat_suggestions_and_tab_completion() {
    let mut chat = ChatHudState::new();
    chat.open(Some("/time set "));

    // Provide suggestions
    chat.set_suggestions(
        10,
        0,
        vec!["day".to_string(), "night".to_string(), "noon".to_string()],
    );
    assert_eq!(chat.suggestions.len(), 3);
    assert_eq!(chat.selected_suggestion, Some(0));

    // Next suggestion
    chat.select_next_suggestion();
    assert_eq!(chat.selected_suggestion, Some(1)); // "night"

    // Apply suggestion
    assert!(chat.apply_selected_suggestion());
    assert_eq!(chat.input_buffer, "/time set night");
    assert_eq!(chat.cursor_pos, 15);
    assert!(chat.suggestions.is_empty());

    // Submit message
    let submitted = chat.take_submitted_message();
    assert_eq!(submitted.as_deref(), Some("/time set night"));
    assert!(!chat.is_open);
    assert_eq!(chat.sent_history.len(), 1);

    // Reopen and recall history via Up arrow
    chat.open(None);
    chat.history_up();
    assert_eq!(chat.input_buffer, "/time set night");
}

#[test]
fn test_chat_history_capping() {
    let mut chat = ChatHudState::new();
    for i in 0..120 {
        chat.add_message("Player", format!("Message {i}"), f64::from(i));
    }
    assert_eq!(chat.history.len(), MAX_CHAT_HISTORY);
    // Oldest 20 messages should have been discarded
    assert_eq!(chat.history[0].message, "Message 20");
    assert_eq!(chat.history[MAX_CHAT_HISTORY - 1].message, "Message 119");
}

#[test]
fn test_chat_hud_rendering_and_fading() {
    let font = BitmapFont::new_fallback(0);
    let mut chat = ChatHudState::new();

    let t0 = 100.0;
    chat.add_message("Player1", "Hello world!", t0);

    // 1. Fresh message (<5s) when closed -> should render
    let mut quads = Vec::new();
    render_chat_hud(&chat, &font, 1920, 1080, 2, t0 + 2.0, &mut quads);
    assert!(
        !quads.is_empty(),
        "Fresh chat message should generate quads"
    );

    // 2. Fading message (5.5s) -> should still render
    let mut quads_fading = Vec::new();
    render_chat_hud(&chat, &font, 1920, 1080, 2, t0 + 5.5, &mut quads_fading);
    assert!(
        !quads_fading.is_empty(),
        "Fading message should render quads"
    );

    // 3. Expired message (7.0s > 6.0s) when closed -> 0 quads
    let mut quads_expired = Vec::new();
    render_chat_hud(&chat, &font, 1920, 1080, 2, t0 + 7.0, &mut quads_expired);
    assert!(
        quads_expired.is_empty(),
        "Expired message when closed should produce 0 quads"
    );

    // 4. When chat is open, expired messages still render in log
    chat.open(Some("/"));
    let mut quads_open = Vec::new();
    render_chat_hud(&chat, &font, 1920, 1080, 2, t0 + 7.0, &mut quads_open);
    assert!(
        !quads_open.is_empty(),
        "Open chat should render log, input bar, and prompt"
    );
}

#[test]
fn test_server_list_screen() {
    let font = BitmapFont::new_fallback(0);
    let mut screen = ServerListScreen::new();

    let lan_server = ServerEntry::new(
        "Local Test Server",
        "192.168.1.50:47679",
        "A survival voxel world",
        1,
        8,
        true,
    );
    screen.add_or_update_lan_server(lan_server);
    assert_eq!(screen.servers.len(), 1);
    assert_eq!(screen.selected_index, Some(0));
    assert!(screen.selected_server().unwrap().is_lan);

    // Update the same server (player count updates)
    let updated_lan = ServerEntry::new(
        "Local Test Server",
        "192.168.1.50:47679",
        "A survival voxel world",
        3,
        8,
        true,
    );
    screen.add_or_update_lan_server(updated_lan);
    assert_eq!(screen.servers.len(), 1);
    assert_eq!(screen.servers[0].current_players, 3);

    // Render server list screen
    let mut quads = Vec::new();
    render_server_list(&screen, &font, 1920, 1080, 2, &mut quads);
    assert!(!quads.is_empty(), "Server list should generate UI quads");

    // Stale server removal
    screen.retain_active_lan_addresses(&[]);
    assert!(screen.servers.is_empty());
}

#[test]
fn test_server_list_direct_connect_actions() {
    use telos_ui::ServerListAction;

    let font = BitmapFont::new_fallback(0);
    let mut screen = ServerListScreen::new();
    screen.add_or_update_lan_server(ServerEntry::new(
        "Test Server",
        "127.0.0.1:25565",
        "MOTD",
        1,
        10,
        true,
    ));

    screen.update_layout(800.0, 600.0);

    // Join selected server action
    let join_action = screen.handle_enter();
    assert_eq!(
        join_action,
        Some(ServerListAction::Connect("127.0.0.1:25565".to_string()))
    );

    // Click direct connect button
    let direct_btn_x = screen.buttons[1].x + 5.0;
    let direct_btn_y = screen.buttons[1].y + 5.0;
    let click_res = screen.handle_mouse_click(direct_btn_x, direct_btn_y, 800.0, 600.0);
    assert_eq!(click_res, None);
    assert!(screen.direct_connect_mode);

    // Type invite link into direct connect input
    for ch in "telos://connect/xyz".chars() {
        screen.handle_char(ch);
    }
    assert_eq!(screen.direct_input.text, "telos://connect/xyz");

    // Press enter to connect
    let conn_action = screen.handle_enter();
    assert_eq!(
        conn_action,
        Some(ServerListAction::Connect("telos://connect/xyz".to_string()))
    );

    // Render direct connect view
    let mut quads = Vec::new();
    render_server_list(&screen, &font, 800, 600, 1, &mut quads);
    assert!(!quads.is_empty());
}

#[test]
fn test_server_groups_collapsible() {
    use telos_ui::server_list::ServerListItem;

    let font = BitmapFont::new_fallback(0);
    let mut screen = ServerListScreen::new();

    screen.servers.push(
        ServerEntry::new("Fav 1", "fav1.telos.io", "Favorite 1", 10, 50, false)
            .with_group("Favorites"),
    );
    screen.servers.push(
        ServerEntry::new("Fav 2", "fav2.telos.io", "Favorite 2", 5, 20, false)
            .with_group("Favorites"),
    );
    screen.servers.push(
        ServerEntry::new("Custom 1", "cust1.telos.io", "Custom 1", 2, 10, false)
            .with_group("Custom Servers"),
    );

    // Initial layout: all groups expanded
    let items_expanded = screen.compute_list_layout(32.0);
    let card_count = items_expanded
        .iter()
        .filter(|it| matches!(it, ServerListItem::ServerCard { .. }))
        .count();
    assert_eq!(card_count, 3, "All 3 servers should be visible when expanded");

    // Collapse 'Favorites' group (group 0)
    screen.groups[0].collapsed = true;
    let items_collapsed = screen.compute_list_layout(32.0);
    let card_count_collapsed = items_collapsed
        .iter()
        .filter(|it| matches!(it, ServerListItem::ServerCard { .. }))
        .count();
    assert_eq!(
        card_count_collapsed, 1,
        "Only 1 server should be visible when Favorites is collapsed"
    );

    // Toggle collapse via clicking header
    screen.update_layout(800.0, 600.0);
    let header_y = match &items_collapsed[0] {
        ServerListItem::GroupHeader { y, .. } => *y + 2.0,
        ServerListItem::ServerCard { .. } => panic!("Expected first item to be GroupHeader"),
    };
    let toggle_action = screen.handle_mouse_click(400.0, header_y, 800.0, 600.0);
    assert_eq!(toggle_action, Some(telos_ui::ServerListAction::ServersModified));
    assert!(
        !screen.groups[0].collapsed,
        "Clicking group header should expand it"
    );

    // Render screen with collapsed and expanded groups
    let mut quads = Vec::new();
    render_server_list(&screen, &font, 800, 600, 1, &mut quads);
    assert!(!quads.is_empty());
}

#[test]
fn test_server_add_mode_and_actions() {
    use telos_ui::ServerListAction;

    let mut screen = ServerListScreen::new();
    screen.update_layout(800.0, 600.0);

    // Click 'Add Server' button (button ID 3 at index 2)
    let add_btn_x = screen.buttons[2].x + 5.0;
    let add_btn_y = screen.buttons[2].y + 5.0;
    let click_res = screen.handle_mouse_click(add_btn_x, add_btn_y, 800.0, 600.0);
    assert_eq!(click_res, None);
    assert!(screen.add_server_mode);

    // Input server name
    for ch in "Hypixel-Style".chars() {
        screen.handle_char(ch);
    }
    assert_eq!(screen.add_name_input.text, "Hypixel-Style");

    // Switch focus to address input
    screen.add_name_input.focused = false;
    screen.add_address_input.focused = true;
    for ch in "mc.hypixel.net:25565".chars() {
        screen.handle_char(ch);
    }
    assert_eq!(screen.add_address_input.text, "mc.hypixel.net:25565");

    // Press Enter to confirm adding server
    let add_action = screen.handle_enter();
    assert_eq!(add_action, Some(ServerListAction::ServersModified));
    assert!(!screen.add_server_mode);

    assert_eq!(screen.servers.len(), 1);
    assert_eq!(screen.servers[0].name, "Hypixel-Style");
    assert_eq!(screen.servers[0].address, "mc.hypixel.net:25565");
    assert_eq!(screen.selected_index, Some(0));
}

#[test]
fn test_server_drag_and_drop_reorder() {
    use telos_ui::ServerListAction;

    let mut screen = ServerListScreen::new();
    screen.servers.push(ServerEntry::new("Alpha", "alpha.io", "A", 1, 10, false));
    screen.servers.push(ServerEntry::new("Beta", "beta.io", "B", 2, 10, false));
    screen.servers.push(ServerEntry::new("Gamma", "gamma.io", "C", 3, 10, false));

    // Start drag on Alpha (index 0)
    screen.dragging_index = Some(0);
    screen.drag_start_y = 100.0;
    screen.drag_mouse_y = 100.0;

    // Drag downward past threshold (100.0 + 45.0 = 145.0)
    screen.handle_mouse_move(400.0, 145.0);

    // Alpha and Beta should have swapped
    assert_eq!(screen.servers[0].name, "Beta");
    assert_eq!(screen.servers[1].name, "Alpha");
    assert_eq!(screen.dragging_index, Some(1));
    assert_eq!(screen.selected_index, Some(1));

    // Release mouse
    let release_action = screen.handle_mouse_release();
    assert_eq!(release_action, Some(ServerListAction::ServersModified));
    assert!(screen.dragging_index.is_none());
}

#[test]
fn test_server_list_persistence_roundtrip() {
    let mut screen = ServerListScreen::new();
    screen.servers.push(
        ServerEntry::new("Permanent Server", "play.example.com", "MOTD", 5, 20, false)
            .with_group("Favorites"),
    );
    screen.servers.push(
        ServerEntry::new("LAN Ephemeral", "192.168.1.10:47679", "LAN", 1, 8, true)
            .with_group("LAN Worlds"),
    );

    let tmp = tempfile::NamedTempFile::new().expect("Temp file failed");
    let tmp_path = tmp.path();

    // Save to disk (only non-LAN servers should be saved)
    screen.save_to_file(tmp_path).expect("Failed to save servers");

    // Load into fresh screen
    let mut fresh_screen = ServerListScreen::new();
    fresh_screen
        .load_from_file(tmp_path)
        .expect("Failed to load servers");

    assert_eq!(fresh_screen.servers.len(), 1);
    assert_eq!(fresh_screen.servers[0].name, "Permanent Server");
    assert_eq!(fresh_screen.servers[0].address, "play.example.com");
    assert_eq!(fresh_screen.servers[0].group, "Favorites");
}

#[test]
fn test_menu_button_hover_animation() {
    use telos_ui::font::BitmapFont;
    use telos_ui::hud::UiLayers;
    use telos_ui::menu::widgets::MenuButton;
    use telos_ui::quad::QuadKind;

    let font = BitmapFont::new_fallback(0);
    let layers = UiLayers::default();
    let mut btn = MenuButton::new(1, 50.0, 50.0, 100.0, 20.0, "Hover Me");
    btn.hovered = true;

    // Render with active animation tick
    let mut quads = Vec::new();
    btn.render_animated(&layers, &font, 2, 20, &mut quads);

    assert!(!quads.is_empty());
    // Quad 0 must always be the NineSlice button
    assert_eq!(quads[0].kind(), QuadKind::NineSlice);
    assert_eq!(quads[0].uv[2], layers.button);
}
