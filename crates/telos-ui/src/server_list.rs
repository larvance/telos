//! Multiplayer server browser screen, collapsible groups, drag-and-drop reordering, and direct connect.

use std::path::Path;

use crate::font::BitmapFont;
use crate::menu::widgets::{ButtonStyle, MenuButton, MenuTextInput};
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;
use crate::text_input::{TextInputAction, TextInputResult};

fn default_group_name() -> String {
    String::new()
}

/// A collapsible group organizing server entries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ServerGroup {
    /// Group display name (e.g. "Favorites", "Custom Servers", "LAN Servers").
    pub name: String,
    /// Whether this group is collapsed in the UI.
    pub collapsed: bool,
}

impl ServerGroup {
    /// Creates a new server group.
    #[must_use]
    pub fn new(name: impl Into<String>, collapsed: bool) -> Self {
        Self {
            name: name.into(),
            collapsed,
        }
    }
}

/// Information for a single server shown in the server list.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServerEntry {
    /// Server display name.
    pub name: String,
    /// Network address (e.g. `127.0.0.1:47679`).
    pub address: String,
    /// Server MOTD / description.
    pub motd: String,
    /// Current number of connected players.
    pub current_players: u16,
    /// Maximum player capacity.
    pub max_players: u16,
    /// Round-trip ping time in milliseconds, if measured.
    pub ping_ms: Option<u32>,
    /// Whether this server was discovered via local LAN broadcast/multicast.
    pub is_lan: bool,
    /// Group this server belongs to.
    #[serde(default = "default_group_name")]
    pub group: String,
}

impl ServerEntry {
    /// Creates a new server entry.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        address: impl Into<String>,
        motd: impl Into<String>,
        current_players: u16,
        max_players: u16,
        is_lan: bool,
    ) -> Self {
        let is_lan_val = is_lan;
        let group = if is_lan_val {
            "LAN Servers".to_string()
        } else {
            String::new()
        };
        Self {
            name: name.into(),
            address: address.into(),
            motd: motd.into(),
            current_players,
            max_players,
            ping_ms: None,
            is_lan: is_lan_val,
            group,
        }
    }

    /// Sets the group name for this server entry.
    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }
}

/// Serialized schema for saving custom servers and groups to disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedServerList {
    /// Saved groups.
    pub groups: Vec<ServerGroup>,
    /// Saved custom servers.
    pub servers: Vec<ServerEntry>,
}

/// Action produced by user interaction with the server list screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerListAction {
    /// Connect to a server by address or ticket.
    Connect(String),
    /// Return to main menu.
    Cancel,
    /// Server entries or groups were modified or reordered (trigger save to disk).
    ServersModified,
}

/// Item in the visual list layout for hit-testing and rendering.
#[derive(Debug, Clone)]
pub enum ServerListItem {
    /// A collapsible group header row.
    GroupHeader {
        /// Index into `groups`.
        group_idx: usize,
        /// Name of the group.
        name: String,
        /// Whether group is currently collapsed.
        collapsed: bool,
        /// Number of servers in this group.
        count: usize,
        /// Top Y in GUI pixels.
        y: f32,
        /// Height in GUI pixels.
        height: f32,
    },
    /// A single server card row.
    ServerCard {
        /// Global index into `servers`.
        server_idx: usize,
        /// Top Y in GUI pixels.
        y: f32,
        /// Height in GUI pixels.
        height: f32,
    },
}

/// Interactive state and card layout for the multiplayer server browser.
#[derive(Debug, Clone)]
pub struct ServerListScreen {
    /// Registered and discovered servers.
    pub servers: Vec<ServerEntry>,
    /// Server groups.
    pub groups: Vec<ServerGroup>,
    /// Index of selected server in `servers`, if any.
    pub selected_index: Option<usize>,
    /// Direct connect address input buffer.
    pub direct_connect_address: String,
    /// Whether the direct connect input box is active and capturing input.
    pub direct_connect_focused: bool,
    /// Vertical scroll offset in GUI pixels.
    pub scroll_offset: f32,
    /// Whether the direct connect input view is currently open.
    pub direct_connect_mode: bool,
    /// Direct connect text input widget.
    pub direct_input: MenuTextInput,
    /// Bottom navigation buttons for server list view.
    pub buttons: Vec<MenuButton>,
    /// Navigation buttons for direct connect view.
    pub direct_buttons: Vec<MenuButton>,

    // Add server dialog mode
    /// Whether the Add Server dialog is currently open.
    pub add_server_mode: bool,
    /// Add server name input widget.
    pub add_name_input: MenuTextInput,
    /// Add server address input widget.
    pub add_address_input: MenuTextInput,
    /// Add server group input widget.
    pub add_group_input: MenuTextInput,
    /// Navigation buttons for Add Server view.
    pub add_buttons: Vec<MenuButton>,

    // Drag-and-drop state
    /// Global server index currently being dragged, if any.
    pub dragging_index: Option<usize>,
    /// Mouse Y coordinate where dragging began.
    pub drag_start_y: f32,
    /// Current mouse Y coordinate during drag.
    pub drag_mouse_y: f32,
}

impl Default for ServerListScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerListScreen {
    /// Creates a new empty `ServerListScreen` with default empty groups.
    #[must_use]
    pub fn new() -> Self {
        let direct_input = MenuTextInput::new(
            10,
            0.0,
            0.0,
            320.0,
            24.0,
            "Enter host:port or telos://connect/<ticket>",
        );

        let add_name_input = MenuTextInput::new(21, 0.0, 0.0, 300.0, 24.0, "Server Name");
        let add_address_input =
            MenuTextInput::new(22, 0.0, 0.0, 300.0, 24.0, "Server Address (host:port)");
        let add_group_input =
            MenuTextInput::new(23, 0.0, 0.0, 300.0, 24.0, "Group Name (optional)");

        Self {
            servers: Vec::new(),
            groups: Vec::new(),
            selected_index: None,
            direct_connect_address: String::new(),
            direct_connect_focused: false,
            scroll_offset: 0.0,
            direct_connect_mode: false,
            direct_input,
            buttons: Vec::new(),
            direct_buttons: Vec::new(),
            add_server_mode: false,
            add_name_input,
            add_address_input,
            add_group_input,
            add_buttons: Vec::new(),
            dragging_index: None,
            drag_start_y: 0.0,
            drag_mouse_y: 0.0,
        }
    }

    /// Computes the visual layout of group headers and server cards given the current scroll offset.
    #[must_use]
    pub fn compute_list_layout(&self, list_top: f32) -> Vec<ServerListItem> {
        let header_h = 20.0f32;
        let card_h = 36.0f32;
        let spacing = 4.0f32;

        let mut items = Vec::new();
        let mut cur_y = list_top - self.scroll_offset;

        // 1. Groupless non-LAN servers (rendered directly without group headers)
        for (s_idx, s) in self.servers.iter().enumerate() {
            if !s.is_lan && s.group.trim().is_empty() {
                items.push(ServerListItem::ServerCard {
                    server_idx: s_idx,
                    y: cur_y,
                    height: card_h,
                });
                cur_y += card_h + spacing;
            }
        }

        // 2. Custom groups that have servers
        for (g_idx, group) in self.groups.iter().enumerate() {
            if group.name == "LAN Servers" || group.name == "LAN Worlds" {
                continue;
            }
            let matching_servers: Vec<usize> = self
                .servers
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.is_lan && s.group == group.name)
                .map(|(i, _)| i)
                .collect();

            if !matching_servers.is_empty() {
                items.push(ServerListItem::GroupHeader {
                    group_idx: g_idx,
                    name: group.name.clone(),
                    collapsed: group.collapsed,
                    count: matching_servers.len(),
                    y: cur_y,
                    height: header_h,
                });
                cur_y += header_h + spacing;

                if !group.collapsed {
                    for s_idx in matching_servers {
                        items.push(ServerListItem::ServerCard {
                            server_idx: s_idx,
                            y: cur_y,
                            height: card_h,
                        });
                        cur_y += card_h + spacing;
                    }
                }
            }
        }

        // 3. Any custom servers with an unknown group name
        let known_group_names: Vec<&str> = self.groups.iter().map(|g| g.name.as_str()).collect();
        let orphan_servers: Vec<usize> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !s.is_lan
                    && !s.group.trim().is_empty()
                    && !known_group_names.contains(&s.group.as_str())
            })
            .map(|(i, _)| i)
            .collect();

        for s_idx in orphan_servers {
            items.push(ServerListItem::ServerCard {
                server_idx: s_idx,
                y: cur_y,
                height: card_h,
            });
            cur_y += card_h + spacing;
        }

        // 4. LAN Servers at the bottom
        let lan_servers: Vec<usize> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_lan)
            .map(|(i, _)| i)
            .collect();

        if !lan_servers.is_empty() {
            let lan_group_idx = self.groups.iter().position(|g| g.name == "LAN Servers");
            let collapsed = lan_group_idx.is_some_and(|idx| self.groups[idx].collapsed);
            let g_idx = lan_group_idx.unwrap_or(usize::MAX);

            items.push(ServerListItem::GroupHeader {
                group_idx: g_idx,
                name: "LAN Servers".to_string(),
                collapsed,
                count: lan_servers.len(),
                y: cur_y,
                height: header_h,
            });
            cur_y += header_h + spacing;

            if !collapsed {
                for s_idx in lan_servers {
                    items.push(ServerListItem::ServerCard {
                        server_idx: s_idx,
                        y: cur_y,
                        height: card_h,
                    });
                    cur_y += card_h + spacing;
                }
            }
        }

        items
    }

    /// Updates layout of buttons and input widgets based on current GUI dimensions.
    pub fn update_layout(&mut self, width_gui: f32, height_gui: f32) {
        let btn_y = height_gui - 36.0;
        let btn_w = 76.0f32;
        let btn_h = 20.0f32;
        let spacing = 6.0f32;

        let total_btns_w = (btn_w * 4.0) + (spacing * 3.0);
        let start_btn_x = (width_gui - total_btns_w) / 2.0;

        let has_sel = self.selected_index.is_some();

        let mut join_btn = MenuButton::new(1, start_btn_x, btn_y, btn_w, btn_h, "Join Server");
        join_btn.enabled = has_sel;

        let direct_btn = MenuButton::new(
            2,
            start_btn_x + btn_w + spacing,
            btn_y,
            btn_w,
            btn_h,
            "Direct Connect",
        );

        let add_btn = MenuButton::new(
            3,
            start_btn_x + (btn_w + spacing) * 2.0,
            btn_y,
            btn_w,
            btn_h,
            "Add Server",
        );

        let cancel_btn = MenuButton::new(
            4,
            start_btn_x + (btn_w + spacing) * 3.0,
            btn_y,
            btn_w,
            btn_h,
            "Cancel",
        );

        self.buttons = vec![join_btn, direct_btn, add_btn, cancel_btn];

        // Direct connect mode layout
        let input_w = 320.0f32.min(width_gui - 32.0);
        let input_x = (width_gui - input_w) / 2.0;
        let input_y = height_gui * 0.40;

        self.direct_input.x = input_x;
        self.direct_input.y = input_y;
        self.direct_input.width = input_w;

        let d_total_w = (btn_w * 2.0) + 8.0;
        let d_start_x = (width_gui - d_total_w) / 2.0;
        let d_btn_y = input_y + 36.0;

        let conn_btn = MenuButton::new(11, d_start_x, d_btn_y, btn_w, btn_h, "Connect")
            .with_style(ButtonStyle::Primary);
        let back_btn = MenuButton::new(12, d_start_x + btn_w + 8.0, d_btn_y, btn_w, btn_h, "Back");

        self.direct_buttons = vec![conn_btn, back_btn];

        // Add server mode layout
        let add_input_w = 300.0f32.min(width_gui - 32.0);
        let add_input_x = (width_gui - add_input_w) / 2.0;

        let add_y1 = height_gui * 0.28;
        let add_y2 = add_y1 + 38.0;
        let add_y3 = add_y2 + 38.0;

        self.add_name_input.x = add_input_x;
        self.add_name_input.y = add_y1;
        self.add_name_input.width = add_input_w;

        self.add_address_input.x = add_input_x;
        self.add_address_input.y = add_y2;
        self.add_address_input.width = add_input_w;

        self.add_group_input.x = add_input_x;
        self.add_group_input.y = add_y3;
        self.add_group_input.width = add_input_w;

        let add_btn_w = 90.0f32;
        let add_actions_total_w = (add_btn_w * 2.0) + 12.0;
        let add_actions_start_x = (width_gui - add_actions_total_w) / 2.0;
        let add_actions_y = add_y3 + 40.0;

        let confirm_add_btn = MenuButton::new(
            21,
            add_actions_start_x,
            add_actions_y,
            add_btn_w,
            btn_h,
            "Add Server",
        )
        .with_style(ButtonStyle::Primary);

        let cancel_add_btn = MenuButton::new(
            22,
            add_actions_start_x + add_btn_w + 12.0,
            add_actions_y,
            add_btn_w,
            btn_h,
            "Cancel",
        );

        self.add_buttons = vec![confirm_add_btn, cancel_add_btn];
    }

    /// Handles mouse motion in GUI pixels and processes drag reordering.
    pub fn handle_mouse_move(&mut self, mouse_x: f32, mouse_y: f32) {
        if self.direct_connect_mode {
            self.direct_input.hovered = self.direct_input.contains(mouse_x, mouse_y);
            for btn in &mut self.direct_buttons {
                btn.hovered = btn.contains(mouse_x, mouse_y);
            }
            return;
        }

        if self.add_server_mode {
            self.add_name_input.hovered = self.add_name_input.contains(mouse_x, mouse_y);
            self.add_address_input.hovered = self.add_address_input.contains(mouse_x, mouse_y);
            self.add_group_input.hovered = self.add_group_input.contains(mouse_x, mouse_y);
            for btn in &mut self.add_buttons {
                btn.hovered = btn.contains(mouse_x, mouse_y);
            }
            return;
        }

        for btn in &mut self.buttons {
            btn.hovered = btn.contains(mouse_x, mouse_y);
        }

        // Handle drag-and-drop server card reordering
        if let Some(drag_idx) = self.dragging_index {
            self.drag_mouse_y = mouse_y;
            let dy = mouse_y - self.drag_start_y;
            let threshold = 36.0 + 4.0; // card height + spacing

            if dy > threshold && drag_idx + 1 < self.servers.len() {
                // Dragged downward past next server
                self.servers.swap(drag_idx, drag_idx + 1);
                self.dragging_index = Some(drag_idx + 1);
                self.selected_index = Some(drag_idx + 1);
                self.drag_start_y += threshold;
            } else if dy < -threshold && drag_idx > 0 {
                // Dragged upward past previous server
                self.servers.swap(drag_idx, drag_idx - 1);
                self.dragging_index = Some(drag_idx - 1);
                self.selected_index = Some(drag_idx - 1);
                self.drag_start_y -= threshold;
            }
        }
    }

    /// Finalizes drag-and-drop on mouse release and returns whether the server order changed.
    pub fn handle_mouse_release(&mut self) -> Option<ServerListAction> {
        if self.dragging_index.is_some() {
            self.dragging_index = None;
            return Some(ServerListAction::ServersModified);
        }
        None
    }

    /// Handles mouse clicks in GUI pixels, returning an action if triggered.
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        width_gui: f32,
        height_gui: f32,
    ) -> Option<ServerListAction> {
        self.update_layout(width_gui, height_gui);

        if self.direct_connect_mode {
            let inside_input = self.direct_input.contains(mouse_x, mouse_y);
            self.direct_input.focused = inside_input;
            self.direct_connect_focused = inside_input;

            for btn in &self.direct_buttons {
                if btn.enabled && btn.contains(mouse_x, mouse_y) {
                    match btn.id {
                        11 => {
                            let text = self.direct_input.text.trim().to_string();
                            if !text.is_empty() {
                                return Some(ServerListAction::Connect(text));
                            }
                        }
                        12 => {
                            self.direct_connect_mode = false;
                            self.direct_connect_focused = false;
                        }
                        _ => {}
                    }
                }
            }
            return None;
        }

        if self.add_server_mode {
            self.add_name_input.focused = self.add_name_input.contains(mouse_x, mouse_y);
            self.add_address_input.focused = self.add_address_input.contains(mouse_x, mouse_y);
            self.add_group_input.focused = self.add_group_input.contains(mouse_x, mouse_y);

            for btn in &self.add_buttons {
                if btn.enabled && btn.contains(mouse_x, mouse_y) {
                    match btn.id {
                        21 => {
                            let addr = self.add_address_input.text.trim().to_string();
                            if !addr.is_empty() {
                                let mut name = self.add_name_input.text.trim().to_string();
                                if name.is_empty() {
                                    name = "Voxel Server".to_string();
                                }
                                let group = self.add_group_input.text.trim().to_string();

                                if !group.is_empty() && !self.groups.iter().any(|g| g.name == group)
                                {
                                    self.groups.push(ServerGroup::new(&group, false));
                                }

                                let new_server =
                                    ServerEntry::new(name, addr, "Custom Server", 0, 0, false)
                                        .with_group(group);

                                self.servers.push(new_server);
                                self.selected_index = Some(self.servers.len() - 1);
                                self.add_server_mode = false;
                                return Some(ServerListAction::ServersModified);
                            }
                        }
                        22 => {
                            self.add_server_mode = false;
                        }
                        _ => {}
                    }
                }
            }
            return None;
        }

        // 1. Check card list clicks and group header toggles
        let list_top_gui = 32.0f32;
        let list_bottom_gui = height_gui - 52.0f32;
        let card_w_gui = 300.0f32.min(width_gui - 20.0);
        let card_x_gui = (width_gui - card_w_gui) / 2.0;

        if mouse_x >= card_x_gui
            && mouse_x <= card_x_gui + card_w_gui
            && mouse_y >= list_top_gui
            && mouse_y <= list_bottom_gui
        {
            let layout_items = self.compute_list_layout(list_top_gui);
            for item in layout_items {
                match item {
                    ServerListItem::GroupHeader {
                        group_idx,
                        y,
                        height,
                        ..
                    } => {
                        if mouse_y >= y && mouse_y <= y + height {
                            if let Some(group) = self.groups.get_mut(group_idx) {
                                group.collapsed = !group.collapsed;
                                return Some(ServerListAction::ServersModified);
                            } else if group_idx == usize::MAX {
                                self.groups.push(ServerGroup::new("LAN Servers", true));
                                return Some(ServerListAction::ServersModified);
                            }
                            break;
                        }
                    }
                    ServerListItem::ServerCard {
                        server_idx,
                        y,
                        height,
                    } => {
                        if mouse_y >= y && mouse_y <= y + height {
                            self.select_server(server_idx);
                            // Initiate drag state
                            self.dragging_index = Some(server_idx);
                            self.drag_start_y = mouse_y;
                            self.drag_mouse_y = mouse_y;
                            break;
                        }
                    }
                }
            }
        }

        // 2. Check bottom buttons
        for btn in &self.buttons {
            if btn.enabled && btn.contains(mouse_x, mouse_y) {
                match btn.id {
                    1 => {
                        if let Some(server) = self.selected_server() {
                            return Some(ServerListAction::Connect(server.address.clone()));
                        }
                    }
                    2 => {
                        self.direct_connect_mode = true;
                        self.direct_input.focused = true;
                        self.direct_connect_focused = true;
                    }
                    3 => {
                        self.add_server_mode = true;
                        self.add_name_input.text.clear();
                        self.add_name_input.cursor_pos = 0;
                        self.add_name_input.focused = true;
                        self.add_address_input.text.clear();
                        self.add_address_input.cursor_pos = 0;
                        self.add_group_input.text.clear();
                        self.add_group_input.cursor_pos = 0;
                    }
                    4 => {
                        return Some(ServerListAction::Cancel);
                    }
                    _ => {}
                }
            }
        }

        None
    }

    /// Handles typed characters into active input boxes.
    pub fn handle_char(&mut self, ch: char) {
        if self.direct_connect_mode && self.direct_input.focused {
            self.direct_input.insert_char(ch);
            self.direct_connect_address = self.direct_input.text.clone();
        } else if self.add_server_mode {
            if self.add_name_input.focused {
                self.add_name_input.insert_char(ch);
            } else if self.add_address_input.focused {
                self.add_address_input.insert_char(ch);
            } else if self.add_group_input.focused {
                self.add_group_input.insert_char(ch);
            }
        }
    }

    /// Handles backspace in active input boxes.
    pub fn handle_backspace(&mut self) {
        if self.direct_connect_mode && self.direct_input.focused {
            self.direct_input.backspace();
            self.direct_connect_address = self.direct_input.text.clone();
        } else if self.add_server_mode {
            if self.add_name_input.focused {
                self.add_name_input.backspace();
            } else if self.add_address_input.focused {
                self.add_address_input.backspace();
            } else if self.add_group_input.focused {
                self.add_group_input.backspace();
            }
        }
    }

    /// Handles a unified text editing action against whichever server list text input is focused.
    pub fn handle_text_action(
        &mut self,
        action: TextInputAction,
        clipboard_text: Option<&str>,
    ) -> TextInputResult {
        if self.direct_connect_mode && self.direct_input.focused {
            let res = self.direct_input.handle_action(action, clipboard_text);
            self.direct_connect_address = self.direct_input.text.clone();
            res
        } else if self.add_server_mode {
            if self.add_name_input.focused {
                self.add_name_input.handle_action(action, clipboard_text)
            } else if self.add_address_input.focused {
                self.add_address_input.handle_action(action, clipboard_text)
            } else if self.add_group_input.focused {
                self.add_group_input.handle_action(action, clipboard_text)
            } else {
                TextInputResult::None
            }
        } else {
            TextInputResult::None
        }
    }

    /// Handles Enter key, confirming connection or adding server.
    pub fn handle_enter(&mut self) -> Option<ServerListAction> {
        if self.direct_connect_mode {
            let text = self.direct_input.text.trim().to_string();
            if !text.is_empty() {
                return Some(ServerListAction::Connect(text));
            }
        } else if self.add_server_mode {
            let addr = self.add_address_input.text.trim().to_string();
            if !addr.is_empty() {
                let mut name = self.add_name_input.text.trim().to_string();
                if name.is_empty() {
                    name = "Voxel Server".to_string();
                }
                let group = self.add_group_input.text.trim().to_string();

                if !group.is_empty() && !self.groups.iter().any(|g| g.name == group) {
                    self.groups.push(ServerGroup::new(&group, false));
                }

                let new_server =
                    ServerEntry::new(name, addr, "Custom Server", 0, 0, false).with_group(group);

                self.servers.push(new_server);
                self.selected_index = Some(self.servers.len() - 1);
                self.add_server_mode = false;
                return Some(ServerListAction::ServersModified);
            }
        } else if let Some(server) = self.selected_server() {
            return Some(ServerListAction::Connect(server.address.clone()));
        }
        None
    }

    /// Adds or updates a LAN discovered server entry by address.
    pub fn add_or_update_lan_server(&mut self, entry: ServerEntry) {
        if let Some(existing) = self.servers.iter_mut().find(|s| s.address == entry.address) {
            existing.motd = entry.motd;
            existing.current_players = entry.current_players;
            existing.max_players = entry.max_players;
            existing.ping_ms = entry.ping_ms;
        } else {
            let mut new_entry = entry;
            new_entry.group = "LAN Servers".to_string();
            if !self.groups.iter().any(|g| g.name == "LAN Servers") {
                self.groups.push(ServerGroup::new("LAN Servers", false));
            }
            self.servers.push(new_entry);
            if self.selected_index.is_none() {
                self.selected_index = Some(self.servers.len() - 1);
            }
        }
    }

    /// Selects a server card by index.
    pub fn select_server(&mut self, idx: usize) {
        if idx < self.servers.len() {
            self.selected_index = Some(idx);
        }
    }

    /// Returns a reference to the currently selected server entry, if any.
    #[must_use]
    pub fn selected_server(&self) -> Option<&ServerEntry> {
        let idx = self.selected_index?;
        self.servers.get(idx)
    }

    /// Removes LAN servers not present in `active_addresses`.
    pub fn retain_active_lan_addresses(&mut self, active_addresses: &[String]) {
        self.servers
            .retain(|s| !s.is_lan || active_addresses.contains(&s.address));
        if matches!(self.selected_index, Some(idx) if idx >= self.servers.len()) {
            self.selected_index = self.servers.len().checked_sub(1);
        }
    }

    /// Loads custom servers and group configurations from a JSON file.
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let path_ref = path.as_ref();
        if !path_ref.is_file() {
            return Ok(());
        }
        let content = std::fs::read_to_string(path_ref)?;
        if let Ok(data) = serde_json::from_str::<SavedServerList>(&content) {
            if !data.groups.is_empty() {
                self.groups = data.groups;
            }
            let lan_servers: Vec<ServerEntry> =
                self.servers.drain(..).filter(|s| s.is_lan).collect();
            self.servers = data.servers;
            self.servers.extend(lan_servers);
            if !self.servers.is_empty() && self.selected_index.is_none() {
                self.selected_index = Some(0);
            }
        }
        Ok(())
    }

    /// Saves custom (non-LAN) servers and group states to a JSON file.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let path_ref = path.as_ref();
        let custom_servers: Vec<ServerEntry> =
            self.servers.iter().filter(|s| !s.is_lan).cloned().collect();
        let store = SavedServerList {
            groups: self.groups.clone(),
            servers: custom_servers,
        };
        let json = serde_json::to_string_pretty(&store)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = path_ref.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path_ref, json)
    }
}

/// Renders the multiplayer server list screen into the output `UiQuad` buffer.
#[allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
pub fn render_server_list(
    screen: &ServerListScreen,
    font: &BitmapFont,
    screen_w: u32,
    screen_h: u32,
    gui_scale: u32,
    out: &mut Vec<UiQuad>,
) {
    let scale = gui_scale.max(1);
    let screen_w_gui = screen_w as f32 / scale as f32;
    let screen_h_gui = screen_h as f32 / scale as f32;

    // 1. Full-screen backdrop
    out.push(UiQuad::solid(
        [0, 0],
        [screen_w as u16, screen_h as u16],
        UiQuad::rgba(16, 16, 16, 235),
    ));

    if screen.add_server_mode {
        // Add Server Mode View
        let title = "Add Server";
        let (title_w, _) = font.measure_text(title);
        let title_x = (screen_w_gui - title_w) / 2.0;
        font.layout_text(
            title,
            title_x,
            screen_h_gui * 0.16,
            UiQuad::rgba(255, 255, 255, 255),
            true,
            scale,
            out,
        );

        let lbl1 = "Server Name:";
        font.layout_text(
            lbl1,
            screen.add_name_input.x,
            screen.add_name_input.y - 12.0,
            UiQuad::rgba(180, 180, 180, 255),
            true,
            scale,
            out,
        );
        screen.add_name_input.render(font, scale, 0, out);

        let lbl2 = "Server Address:";
        font.layout_text(
            lbl2,
            screen.add_address_input.x,
            screen.add_address_input.y - 12.0,
            UiQuad::rgba(180, 180, 180, 255),
            true,
            scale,
            out,
        );
        screen.add_address_input.render(font, scale, 0, out);

        let lbl3 = "Server Group:";
        font.layout_text(
            lbl3,
            screen.add_group_input.x,
            screen.add_group_input.y - 12.0,
            UiQuad::rgba(180, 180, 180, 255),
            true,
            scale,
            out,
        );
        screen.add_group_input.render(font, scale, 0, out);

        for btn in &screen.add_buttons {
            btn.render(font, scale, out);
        }

        return;
    }

    if screen.direct_connect_mode {
        // Direct Connect View
        let title = "Direct Connect";
        let (title_w, _) = font.measure_text(title);
        let title_x = (screen_w_gui - title_w) / 2.0;
        font.layout_text(
            title,
            title_x,
            screen_h_gui * 0.25,
            UiQuad::rgba(255, 255, 255, 255),
            true,
            scale,
            out,
        );

        let prompt = "Enter server address or invite link (telos://connect/...):";
        let (prompt_w, _) = font.measure_text(prompt);
        let prompt_x = (screen_w_gui - prompt_w) / 2.0;
        font.layout_text(
            prompt,
            prompt_x,
            screen_h_gui * 0.35,
            UiQuad::rgba(180, 180, 180, 255),
            true,
            scale,
            out,
        );

        screen.direct_input.render(font, scale, 0, out);

        for btn in &screen.direct_buttons {
            btn.render(font, scale, out);
        }

        return;
    }

    // 2. Header title: "Play Multiplayer"
    let title = "Play Multiplayer";
    let (title_w, _) = font.measure_text(title);
    let title_x = (screen_w_gui - title_w) / 2.0;
    font.layout_text(
        title,
        title_x,
        12.0,
        UiQuad::rgba(255, 255, 255, 255),
        true,
        scale,
        out,
    );

    // 3. Server list cards area
    let list_top_gui = 32.0f32;
    let list_bottom_gui = screen_h_gui - 52.0f32;
    let card_w_gui = 300.0f32.min(screen_w_gui - 20.0);
    let card_x_gui = (screen_w_gui - card_w_gui) / 2.0;

    let layout_items = screen.compute_list_layout(list_top_gui);

    if layout_items.is_empty() {
        let msg = "Scanning for LAN games...";
        let (msg_w, _) = font.measure_text(msg);
        let msg_x = (screen_w_gui - msg_w) / 2.0;
        font.layout_text(
            msg,
            msg_x,
            list_top_gui + 24.0,
            UiQuad::rgba(160, 160, 160, 255),
            true,
            scale,
            out,
        );
    } else {
        for item in layout_items {
            match item {
                ServerListItem::GroupHeader {
                    name,
                    collapsed,
                    count,
                    y,
                    height,
                    ..
                } => {
                    if y + height < list_top_gui || y > list_bottom_gui {
                        continue;
                    }
                    let phys_x = snap_to_physical(card_x_gui, scale);
                    let phys_y = snap_to_physical(y, scale);
                    let phys_w = (card_w_gui * scale as f32).round() as u16;
                    let phys_h = (height * scale as f32).round() as u16;

                    // Group header bar
                    out.push(UiQuad::solid(
                        [phys_x, phys_y],
                        [phys_w, phys_h],
                        UiQuad::rgba(35, 42, 58, 220),
                    ));
                    out.push(UiQuad::solid(
                        [phys_x, phys_y],
                        [phys_w, scale as u16],
                        UiQuad::rgba(60, 75, 100, 255),
                    ));

                    let icon = if collapsed { "▶ " } else { "▼ " };
                    let header_text = format!("{icon}{name} ({count})");
                    font.layout_text(
                        &header_text,
                        card_x_gui + 6.0,
                        y + 5.0,
                        UiQuad::rgba(220, 230, 255, 255),
                        true,
                        scale,
                        out,
                    );

                    let toggle_hint = if collapsed {
                        "Click to expand"
                    } else {
                        "Click to collapse"
                    };
                    let (th_w, _) = font.measure_text(toggle_hint);
                    font.layout_text(
                        toggle_hint,
                        card_x_gui + card_w_gui - th_w - 6.0,
                        y + 5.0,
                        UiQuad::rgba(130, 140, 165, 255),
                        true,
                        scale,
                        out,
                    );
                }
                ServerListItem::ServerCard {
                    server_idx,
                    y,
                    height,
                } => {
                    let Some(server) = screen.servers.get(server_idx) else {
                        continue;
                    };

                    let is_being_dragged = screen.dragging_index == Some(server_idx);
                    let effective_y = if is_being_dragged {
                        y + (screen.drag_mouse_y - screen.drag_start_y)
                    } else {
                        y
                    };

                    if effective_y + height < list_top_gui || effective_y > list_bottom_gui {
                        continue;
                    }

                    let is_selected = screen.selected_index == Some(server_idx);

                    let phys_x = snap_to_physical(card_x_gui, scale);
                    let phys_y = snap_to_physical(effective_y, scale);
                    let phys_w = (card_w_gui * scale as f32).round() as u16;
                    let phys_h = (height * scale as f32).round() as u16;

                    // If dragged, draw elevated shadow underneath
                    if is_being_dragged {
                        out.push(UiQuad::solid(
                            [phys_x + (3 * scale as i32), phys_y + (4 * scale as i32)],
                            [phys_w, phys_h],
                            UiQuad::rgba(0, 0, 0, 90),
                        ));
                    }

                    // Card background
                    let bg_color = if is_being_dragged {
                        UiQuad::rgba(55, 75, 120, 240)
                    } else if is_selected {
                        UiQuad::rgba(40, 50, 80, 220)
                    } else {
                        UiQuad::rgba(25, 25, 25, 200)
                    };
                    out.push(UiQuad::solid([phys_x, phys_y], [phys_w, phys_h], bg_color));

                    if is_selected || is_being_dragged {
                        let border_col = if is_being_dragged {
                            UiQuad::rgba(255, 230, 100, 255)
                        } else {
                            UiQuad::rgba(255, 255, 255, 255)
                        };
                        out.push(UiQuad::solid(
                            [phys_x, phys_y],
                            [phys_w, scale as u16],
                            border_col,
                        ));
                        out.push(UiQuad::solid(
                            [phys_x, phys_y + i32::from(phys_h) - scale.cast_signed()],
                            [phys_w, scale as u16],
                            border_col,
                        ));
                    }

                    // Drag grip handle icon "≡"
                    let grip = if is_being_dragged { "§6≡" } else { "§7≡" };
                    font.layout_text(
                        grip,
                        card_x_gui + 4.0,
                        effective_y + 10.0,
                        UiQuad::rgba(200, 200, 200, 255),
                        true,
                        scale,
                        out,
                    );

                    // Server title + LAN badge
                    let mut name_text = server.name.clone();
                    if server.is_lan {
                        name_text.push_str(" §a[LAN]");
                    }
                    font.layout_text(
                        &name_text,
                        card_x_gui + 14.0,
                        effective_y + 4.0,
                        UiQuad::rgba(255, 255, 255, 255),
                        true,
                        scale,
                        out,
                    );

                    // Server MOTD
                    font.layout_text(
                        &server.motd,
                        card_x_gui + 14.0,
                        effective_y + 16.0,
                        UiQuad::rgba(180, 180, 180, 255),
                        true,
                        scale,
                        out,
                    );

                    // Player count (right aligned)
                    let player_text = format!("{}/{}", server.current_players, server.max_players);
                    let (pw, _) = font.measure_text(&player_text);
                    font.layout_text(
                        &player_text,
                        card_x_gui + card_w_gui - pw - 6.0,
                        effective_y + 4.0,
                        UiQuad::rgba(180, 180, 180, 255),
                        true,
                        scale,
                        out,
                    );

                    // Ping text
                    let ping_text = server
                        .ping_ms
                        .map_or_else(|| "---".to_string(), |p| format!("{p}ms"));
                    let (ping_w, _) = font.measure_text(&ping_text);
                    font.layout_text(
                        &ping_text,
                        card_x_gui + card_w_gui - ping_w - 6.0,
                        effective_y + 16.0,
                        UiQuad::rgba(85, 255, 85, 255),
                        true,
                        scale,
                        out,
                    );
                }
            }
        }
    }

    // 4. Bottom action buttons
    for btn in &screen.buttons {
        btn.render(font, scale, out);
    }
}
