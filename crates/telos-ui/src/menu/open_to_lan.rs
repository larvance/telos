//! Interactive Open to LAN / P2P invite configuration modal screen.

use crate::font::BitmapFont;
use crate::menu::widgets::{ButtonStyle, MenuButton, MenuTextInput};
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;

/// Action triggered by interaction in the Open to LAN dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenToLanAction {
    /// Confirm and launch LAN / P2P listener with the configured settings.
    StartLan {
        /// Port number (1024 - 65535).
        port: u16,
        /// Access password.
        password: String,
        /// Whether the world should automatically open to LAN on subsequent loads.
        auto_open: bool,
    },
    /// Generate a new cryptographically random password.
    GenerateRandomPassword,
    /// Cancel and return to the pause menu.
    Cancel,
}

/// Interactive Open to LAN configuration screen.
#[derive(Debug, Clone)]
pub struct OpenToLanScreen {
    /// Port text input widget.
    pub port_input: MenuTextInput,
    /// LAN password text input widget.
    pub password_input: MenuTextInput,
    /// Whether the server should automatically open to LAN when the world loads.
    pub auto_open: bool,
    /// Action and toggle buttons.
    pub buttons: Vec<MenuButton>,
    /// Screen title text.
    pub title: String,
    /// Optional status or error notice.
    pub status_message: Option<String>,
}

impl Default for OpenToLanScreen {
    fn default() -> Self {
        Self::new(25565, "", false)
    }
}

impl OpenToLanScreen {
    /// Creates a new `OpenToLanScreen` with initial settings.
    #[must_use]
    pub fn new(initial_port: u16, initial_password: &str, initial_auto_open: bool) -> Self {
        let mut port_input = MenuTextInput::new(10, 0.0, 0.0, 140.0, 22.0, "25565");
        port_input.text = initial_port.to_string();

        let mut password_input =
            MenuTextInput::new(11, 0.0, 0.0, 260.0, 22.0, "LAN Password (20 chars)");
        password_input.text = initial_password.to_string();

        Self {
            port_input,
            password_input,
            auto_open: initial_auto_open,
            buttons: Vec::new(),
            title: "Open to LAN / Invite Friends".to_string(),
            status_message: None,
        }
    }

    /// Sets the password text.
    pub fn set_password(&mut self, password: &str) {
        self.password_input.text = password.to_string();
        self.password_input.cursor_pos = password.len();
    }

    /// Updates widget layout for the given GUI dimensions.
    pub fn update_layout(&mut self, width_gui: f32, height_gui: f32) {
        let center_x = width_gui * 0.5;
        let card_w = 340.0f32.min(width_gui - 32.0);
        let card_x = center_x - card_w * 0.5;

        let start_y = (height_gui * 0.24).max(30.0);

        // Port input
        self.port_input.x = card_x + 16.0;
        self.port_input.y = start_y + 44.0;
        self.port_input.width = 100.0;

        // Password input
        self.password_input.x = card_x + 16.0;
        self.password_input.y = start_y + 96.0;
        self.password_input.width = card_w - 32.0;

        // Buttons
        let btn_h = 22.0;

        // Randomize Password button
        let btn_rand = MenuButton::new(
            1,
            card_x + 16.0,
            start_y + 128.0,
            card_w - 32.0,
            btn_h,
            "Generate New 20-Char Password",
        );

        // Auto-open toggle button
        let auto_open_label = if self.auto_open {
            "Auto-Open on World Load: ON"
        } else {
            "Auto-Open on World Load: OFF"
        };
        let btn_auto = MenuButton::new(
            2,
            card_x + 16.0,
            start_y + 158.0,
            card_w - 32.0,
            btn_h,
            auto_open_label,
        );

        // Action buttons at the bottom: Start LAN and Cancel
        let action_btn_w = (card_w - 40.0) * 0.5;
        let btn_start = MenuButton::new(
            3,
            card_x + 16.0,
            start_y + 196.0,
            action_btn_w,
            btn_h,
            "Open to LAN",
        )
        .with_style(ButtonStyle::Primary);

        let btn_cancel = MenuButton::new(
            4,
            card_x + 24.0 + action_btn_w,
            start_y + 196.0,
            action_btn_w,
            btn_h,
            "Cancel",
        );

        self.buttons = vec![btn_rand, btn_auto, btn_start, btn_cancel];
    }

    /// Handles mouse motion in GUI coordinates.
    pub fn handle_mouse_move(&mut self, mouse_x: f32, mouse_y: f32) {
        self.port_input.hovered = self.port_input.contains(mouse_x, mouse_y);
        self.password_input.hovered = self.password_input.contains(mouse_x, mouse_y);
        for btn in &mut self.buttons {
            btn.hovered = btn.contains(mouse_x, mouse_y);
        }
    }

    /// Handles mouse button clicks.
    pub fn handle_mouse_click(&mut self, mouse_x: f32, mouse_y: f32) -> Option<OpenToLanAction> {
        self.port_input.focused = self.port_input.contains(mouse_x, mouse_y);
        self.password_input.focused = self.password_input.contains(mouse_x, mouse_y);

        for btn in &self.buttons {
            if btn.contains(mouse_x, mouse_y) {
                match btn.id {
                    1 => return Some(OpenToLanAction::GenerateRandomPassword),
                    2 => {
                        self.auto_open = !self.auto_open;
                        return None;
                    }
                    3 => {
                        let port = self
                            .port_input
                            .text
                            .trim()
                            .parse::<u16>()
                            .unwrap_or(25565)
                            .clamp(1024, 65535);
                        let password = self.password_input.text.trim().to_string();
                        return Some(OpenToLanAction::StartLan {
                            port,
                            password,
                            auto_open: self.auto_open,
                        });
                    }
                    4 => return Some(OpenToLanAction::Cancel),
                    _ => {}
                }
            }
        }
        None
    }

    /// Handles typed character input into the active text field.
    pub fn handle_char(&mut self, ch: char) {
        if self.port_input.focused {
            if ch.is_ascii_digit() && self.port_input.text.len() < 5 {
                self.port_input.insert_char(ch);
            }
        } else if self.password_input.focused && !ch.is_control() && self.password_input.text.len() < 64 {
            self.password_input.insert_char(ch);
        }
    }

    /// Handles backspace key press in the active text field.
    pub fn handle_backspace(&mut self) {
        if self.port_input.focused {
            self.port_input.backspace();
        } else if self.password_input.focused {
            self.password_input.backspace();
        }
    }

    /// Handles Enter key press.
    pub fn handle_enter(&mut self) -> Option<OpenToLanAction> {
        let port = self
            .port_input
            .text
            .trim()
            .parse::<u16>()
            .unwrap_or(25565)
            .clamp(1024, 65535);
        let password = self.password_input.text.trim().to_string();
        Some(OpenToLanAction::StartLan {
            port,
            password,
            auto_open: self.auto_open,
        })
    }

    /// Renders the Open to LAN configuration dialog.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn render(
        &self,
        font: &BitmapFont,
        width_gui: f32,
        height_gui: f32,
        gui_scale: u32,
        out: &mut Vec<UiQuad>,
    ) {
        let scale = gui_scale.max(1);

        // 1. Semi-transparent backdrop overlay
        let px_w = snap_to_physical(width_gui, scale) as u16;
        let px_h = snap_to_physical(height_gui, scale) as u16;
        out.push(UiQuad::solid(
            [0, 0],
            [px_w, px_h],
            UiQuad::rgba(0, 0, 0, 180),
        ));

        // 2. Dialog card panel
        let center_x = width_gui * 0.5;
        let card_w = 340.0f32.min(width_gui - 32.0);
        let card_x = center_x - card_w * 0.5;
        let card_y = (height_gui * 0.24).max(30.0);
        let card_h = 240.0;

        let px = snap_to_physical(card_x, scale);
        let py = snap_to_physical(card_y, scale);
        let pw = snap_to_physical(card_w, scale) as u16;
        let ph = snap_to_physical(card_h, scale) as u16;
        let border_th = scale.max(1) as u16;

        // Card background
        out.push(UiQuad::solid(
            [px, py],
            [pw, ph],
            UiQuad::rgba(32, 34, 40, 245),
        ));

        // Card border
        out.push(UiQuad::solid(
            [px, py],
            [pw, border_th],
            UiQuad::rgba(80, 84, 96, 255),
        ));
        out.push(UiQuad::solid(
            [px, py + ph as i32 - border_th as i32],
            [pw, border_th],
            UiQuad::rgba(80, 84, 96, 255),
        ));
        out.push(UiQuad::solid(
            [px, py],
            [border_th, ph],
            UiQuad::rgba(80, 84, 96, 255),
        ));
        out.push(UiQuad::solid(
            [px + pw as i32 - border_th as i32, py],
            [border_th, ph],
            UiQuad::rgba(80, 84, 96, 255),
        ));

        // Title text
        let (title_w, _) = font.measure_text(&self.title);
        let title_x = card_x + (card_w - title_w) * 0.5;
        let title_y = card_y + 12.0;
        font.layout_text(
            &self.title,
            title_x,
            title_y,
            UiQuad::rgba(255, 255, 255, 255),
            true,
            scale,
            out,
        );

        // Port label
        let port_label_x = card_x + 16.0;
        let port_label_y = card_y + 32.0;
        font.layout_text(
            "Port (1024 - 65535):",
            port_label_x,
            port_label_y,
            UiQuad::rgba(200, 200, 200, 255),
            true,
            scale,
            out,
        );

        // Password label
        let pwd_label_x = card_x + 16.0;
        let pwd_label_y = card_y + 84.0;
        font.layout_text(
            "LAN Password (saved in world files):",
            pwd_label_x,
            pwd_label_y,
            UiQuad::rgba(200, 200, 200, 255),
            true,
            scale,
            out,
        );

        // Input widgets
        self.port_input.render(font, scale, 0, out);
        self.password_input.render(font, scale, 0, out);

        // Action buttons
        for btn in &self.buttons {
            btn.render(font, scale, out);
        }

        // Status message if any
        if let Some(msg) = &self.status_message {
            let (msg_w, _) = font.measure_text(msg);
            let msg_x = card_x + (card_w - msg_w) * 0.5;
            let msg_y = card_y + card_h - 16.0;
            font.layout_text(
                msg,
                msg_x,
                msg_y,
                UiQuad::rgba(255, 100, 100, 255),
                true,
                scale,
                out,
            );
        }
    }
}
