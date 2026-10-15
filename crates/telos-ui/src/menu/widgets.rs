//! Reusable pure-CPU menu widgets for title, settings, and modal screens.

use crate::font::BitmapFont;
use crate::hud::UiLayers;
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;
use crate::text_input::{self, TextInputAction, TextInputResult};

/// Visual theme style for a menu button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonStyle {
    /// Standard neutral dark slate button.
    #[default]
    Default,
    /// Electric cyan accent button for primary actions (e.g. Play, Save).
    Primary,
    /// Alert crimson button for destructive actions (e.g. Delete, Move to Trash).
    Danger,
}

/// Standard interactive button widget rendered to `UiQuad` primitives.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuButton {
    /// Unique identifier for event routing.
    pub id: u32,
    /// Top-left X in GUI pixels.
    pub x: f32,
    /// Top-left Y in GUI pixels.
    pub y: f32,
    /// Width in GUI pixels.
    pub width: f32,
    /// Height in GUI pixels.
    pub height: f32,
    /// Button display text label.
    pub label: String,
    /// Whether this button is enabled and clickable.
    pub enabled: bool,
    /// Whether the mouse cursor is currently hovering over this button.
    pub hovered: bool,
    /// Visual style theme.
    pub style: ButtonStyle,
}

impl MenuButton {
    /// Creates a new `MenuButton`.
    #[must_use]
    pub fn new(id: u32, x: f32, y: f32, width: f32, height: f32, label: impl Into<String>) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            label: label.into(),
            enabled: true,
            hovered: false,
            style: ButtonStyle::Default,
        }
    }

    /// Sets the visual style theme and returns `self`.
    #[must_use]
    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Hit-tests a mouse position in GUI pixels against this button.
    #[must_use]
    pub fn contains(&self, mouse_x: f32, mouse_y: f32) -> bool {
        mouse_x >= self.x
            && mouse_x <= self.x + self.width
            && mouse_y >= self.y
            && mouse_y <= self.y + self.height
    }

    /// Renders the button to the `UiQuad` output draw list using default UI asset layers.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render(&self, font: &BitmapFont, scale: u32, out: &mut Vec<UiQuad>) {
        self.render_animated(&UiLayers::default(), font, scale, 0, out);
    }

    /// Renders the button using 9-slice scalable sprite assets and text to `out`.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render_with_layers(
        &self,
        layers: &UiLayers,
        font: &BitmapFont,
        scale: u32,
        out: &mut Vec<UiQuad>,
    ) {
        self.render_animated(layers, font, scale, 0, out);
    }

    /// Renders the button with dynamic hover animations (visual elevation, sheen sweep, pulsing glow).
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::too_many_lines
    )]
    pub fn render_animated(
        &self,
        layers: &UiLayers,
        font: &BitmapFont,
        scale: u32,
        tick: u64,
        out: &mut Vec<UiQuad>,
    ) {
        let px_x = snap_to_physical(self.x, scale);
        let px_y = snap_to_physical(self.y, scale);
        let px_w = snap_to_physical(self.width, scale) as u16;
        let px_h = snap_to_physical(self.height, scale) as u16;
        let gui_scale = scale.max(1);

        let uv = if !self.enabled {
            layers.button_disabled_uv
        } else if self.hovered {
            layers.button_highlighted_uv
        } else {
            layers.button_normal_uv
        };

        let pulse = if self.hovered && tick > 0 {
            let t = if tick > 1000 {
                (tick as f32) * 0.001
            } else {
                (tick as f32) / 20.0
            };
            ((t * std::f32::consts::PI * 2.0).sin() * 12.0).round() as i32
        } else {
            0
        };

        let tint_col = match self.style {
            ButtonStyle::Default => {
                if self.hovered && self.enabled {
                    let b = (245 + pulse).clamp(210, 255) as u8;
                    UiQuad::rgba(255, 255, b, 255)
                } else {
                    UiQuad::rgba(255, 255, 255, 255)
                }
            }
            ButtonStyle::Primary => {
                if !self.enabled {
                    UiQuad::rgba(140, 160, 180, 200)
                } else if self.hovered {
                    let r = (200 + pulse).clamp(180, 240) as u8;
                    UiQuad::rgba(r, 245, 255, 255)
                } else {
                    UiQuad::rgba(180, 230, 255, 255)
                }
            }
            ButtonStyle::Danger => {
                if !self.enabled {
                    UiQuad::rgba(160, 140, 140, 200)
                } else if self.hovered {
                    let g = (210 + pulse).clamp(190, 240) as u8;
                    UiQuad::rgba(255, g, g, 255)
                } else {
                    UiQuad::rgba(255, 180, 180, 255)
                }
            }
        };

        // Nine-slice scalable button quad matching Minecraft widget buttons (border: 3 GUI px)
        let borders = [
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
        ];
        let src_size = [
            (200u32 * gui_scale).min(65535) as u16,
            (20u32 * gui_scale).min(65535) as u16,
        ];

        out.push(UiQuad::nine_slice(
            [px_x, px_y],
            [px_w, px_h],
            [uv[0], uv[1]],
            [uv[2], uv[3]],
            layers.button,
            borders,
            src_size,
            tint_col,
        ));

        // Animated sliding sheen beam across button surface
        if self.hovered && self.enabled && tick > 0 {
            let sheen_cycle = 75u64;
            let phase = (tick % sheen_cycle) as f32 / sheen_cycle as f32;
            let sheen_center_x = self.x - 24.0 + phase * (self.width + 48.0);
            let sheen_w = 20.0f32;
            let sheen_min_x = sheen_center_x.max(self.x + 2.0);
            let sheen_max_x = (sheen_center_x + sheen_w).min(self.x + self.width - 2.0);
            if sheen_max_x > sheen_min_x {
                let sx = snap_to_physical(sheen_min_x, scale);
                let sy = snap_to_physical(self.y + 2.0, scale);
                let sw = snap_to_physical(sheen_max_x - sheen_min_x, scale) as u16;
                let sh = snap_to_physical(self.height - 4.0, scale) as u16;
                out.push(UiQuad::solid(
                    [sx, sy],
                    [sw, sh],
                    UiQuad::rgba(255, 255, 255, 45),
                ));
            }
        }

        // Label text styling: classic Minecraft white/yellow hover text with drop shadows
        let text_col = if !self.enabled {
            UiQuad::rgba(160, 160, 160, 255)
        } else if self.hovered {
            match self.style {
                ButtonStyle::Default => UiQuad::rgba(255, 255, 160, 255),
                ButtonStyle::Primary => UiQuad::rgba(103, 232, 249, 255),
                ButtonStyle::Danger => UiQuad::rgba(248, 113, 113, 255),
            }
        } else {
            match self.style {
                ButtonStyle::Default => UiQuad::rgba(224, 224, 224, 255),
                ButtonStyle::Primary => UiQuad::rgba(56, 189, 248, 255),
                ButtonStyle::Danger => UiQuad::rgba(252, 165, 165, 255),
            }
        };

        let (text_w, text_h) = font.measure_text(&self.label);
        let text_x = self.x + (self.width - text_w) * 0.5;
        let text_y = self.y + (self.height - text_h) * 0.5 - if self.hovered { 0.5 } else { 0.0 };

        font.layout_text(&self.label, text_x, text_y, text_col, true, scale, out);
    }
}

/// Draggable value slider widget with numeric/custom formatting.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuSlider {
    /// Unique identifier.
    pub id: u32,
    /// Top-left X in GUI pixels.
    pub x: f32,
    /// Top-left Y in GUI pixels.
    pub y: f32,
    /// Width in GUI pixels.
    pub width: f32,
    /// Height in GUI pixels.
    pub height: f32,
    /// Label prefix (e.g. "FOV", "Master Volume").
    pub prefix: String,
    /// Minimum value.
    pub min: f32,
    /// Maximum value.
    pub max: f32,
    /// Current value.
    pub value: f32,
    /// Display format mode: suffix or percentage.
    pub suffix: String,
    /// Whether value is an integer step.
    pub is_int: bool,
    /// Whether hovered.
    pub hovered: bool,
    /// Whether actively dragged.
    pub dragging: bool,
}

impl MenuSlider {
    /// Creates a new `MenuSlider`.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        prefix: impl Into<String>,
        min: f32,
        max: f32,
        value: f32,
        suffix: impl Into<String>,
        is_int: bool,
    ) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            prefix: prefix.into(),
            min,
            max,
            value: value.clamp(min, max),
            suffix: suffix.into(),
            is_int,
            hovered: false,
            dragging: false,
        }
    }

    /// Hit tests mouse coordinate against slider bounding box.
    #[must_use]
    pub fn contains(&self, mouse_x: f32, mouse_y: f32) -> bool {
        mouse_x >= self.x
            && mouse_x <= self.x + self.width
            && mouse_y >= self.y
            && mouse_y <= self.y + self.height
    }

    /// Updates current value from mouse X coordinate.
    pub fn update_from_mouse_x(&mut self, mouse_x: f32) {
        let handle_w = 8.0f32;
        let scrub_w = (self.width - handle_w).max(1.0);
        let rel_x = (mouse_x - (self.x + handle_w * 0.5)).clamp(0.0, scrub_w);
        let frac = rel_x / scrub_w;
        let mut val = self.min + frac * (self.max - self.min);
        if self.is_int {
            val = val.round();
        }
        self.value = val.clamp(self.min, self.max);
    }

    /// Formats the current display string.
    #[must_use]
    pub fn display_text(&self) -> String {
        if self.prefix == "FPS Limit" {
            let val = self.value.round() as i32;
            if val <= 0 {
                return "FPS Limit: VSync".to_string();
            }
            if val >= self.max.round() as i32 {
                return "FPS Limit: Unlimited".to_string();
            }
            return format!("FPS Limit: {val} FPS");
        }
        if self.is_int {
            format!("{}: {}{}", self.prefix, self.value as i32, self.suffix)
        } else if self.suffix == "%" {
            format!("{}: {}%", self.prefix, (self.value * 100.0).round() as i32)
        } else {
            format!("{}: {:.1}{}", self.prefix, self.value, self.suffix)
        }
    }

    /// Renders track, fill, handle, and text to `out` using default UI asset layers.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render(&self, font: &BitmapFont, scale: u32, out: &mut Vec<UiQuad>) {
        self.render_with_layers(&UiLayers::default(), font, scale, out);
    }

    /// Renders track, fill, handle, and text to `out` using authentic widget assets.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render_with_layers(
        &self,
        layers: &UiLayers,
        font: &BitmapFont,
        scale: u32,
        out: &mut Vec<UiQuad>,
    ) {
        let px_x = snap_to_physical(self.x, scale);
        let px_y = snap_to_physical(self.y, scale);
        let px_w = snap_to_physical(self.width, scale) as u16;
        let px_h = snap_to_physical(self.height, scale) as u16;
        let gui_scale = scale.max(1);

        let track_uv = if self.hovered || self.dragging {
            layers.slider_track_highlighted_uv
        } else {
            layers.slider_track_uv
        };

        // Slider track 9-slice background
        let borders = [
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
        ];
        let src_size = [
            (200u32 * gui_scale).min(65535) as u16,
            (20u32 * gui_scale).min(65535) as u16,
        ];

        out.push(UiQuad::nine_slice(
            [px_x, px_y],
            [px_w, px_h],
            [track_uv[0], track_uv[1]],
            [track_uv[2], track_uv[3]],
            layers.button,
            borders,
            src_size,
            UiQuad::rgba(255, 255, 255, 255),
        ));

        // Slider handle
        let frac = if (self.max - self.min).abs() > 1e-4 {
            ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let handle_w = 8.0f32;
        let scrub_w = (self.width - handle_w).max(1.0);
        let handle_x_gui = self.x + frac * scrub_w;
        let h_px_x = snap_to_physical(handle_x_gui, scale);
        let h_px_w = snap_to_physical(handle_w, scale) as u16;

        let handle_uv = if self.dragging || self.hovered {
            layers.slider_handle_highlighted_uv
        } else {
            layers.slider_handle_uv
        };

        let handle_borders = [
            (2u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
            (2u32 * gui_scale).min(255) as u8,
            (3u32 * gui_scale).min(255) as u8,
        ];
        let handle_src_size = [
            (8u32 * gui_scale).min(65535) as u16,
            (20u32 * gui_scale).min(65535) as u16,
        ];

        out.push(UiQuad::nine_slice(
            [h_px_x, px_y],
            [h_px_w, px_h],
            [handle_uv[0], handle_uv[1]],
            [handle_uv[2], handle_uv[3]],
            layers.button,
            handle_borders,
            handle_src_size,
            UiQuad::rgba(255, 255, 255, 255),
        ));

        // Label
        let label = self.display_text();
        let (tw, th) = font.measure_text(&label);
        let tx = self.x + (self.width - tw) * 0.5;
        let ty = self.y + (self.height - th) * 0.5;
        let t_col = if self.hovered || self.dragging {
            UiQuad::rgba(255, 255, 160, 255)
        } else {
            UiQuad::rgba(224, 224, 224, 255)
        };
        font.layout_text(&label, tx, ty, t_col, true, scale, out);
    }
}

/// Interactive single-line text input widget with cursor, word navigation, selection, and clipboard support.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuTextInput {
    /// Unique identifier.
    pub id: u32,
    /// Top-left X in GUI pixels.
    pub x: f32,
    /// Top-left Y in GUI pixels.
    pub y: f32,
    /// Width in GUI pixels.
    pub width: f32,
    /// Height in GUI pixels.
    pub height: f32,
    /// Current text content.
    pub text: String,
    /// Placeholder text shown when text is empty.
    pub placeholder: String,
    /// Cursor character index.
    pub cursor_pos: usize,
    /// Selection anchor index where selection started when holding Shift.
    pub selection_anchor: Option<usize>,
    /// Whether currently focused and receiving keystrokes.
    pub focused: bool,
    /// Whether hovered.
    pub hovered: bool,
}

impl MenuTextInput {
    /// Creates a new `MenuTextInput`.
    #[must_use]
    pub fn new(
        id: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        placeholder: impl Into<String>,
    ) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            text: String::new(),
            placeholder: placeholder.into(),
            cursor_pos: 0,
            selection_anchor: None,
            focused: false,
            hovered: false,
        }
    }

    /// Hit tests mouse coordinate against input box.
    #[must_use]
    pub fn contains(&self, mouse_x: f32, mouse_y: f32) -> bool {
        mouse_x >= self.x
            && mouse_x <= self.x + self.width
            && mouse_y >= self.y
            && mouse_y <= self.y + self.height
    }

    /// Returns the active selection range in byte indices if anchor is present and non-empty.
    #[must_use]
    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        text_input::selection_range(&self.text, self.cursor_pos, self.selection_anchor)
    }

    /// Returns the currently selected slice of text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        self.selection_range().map(|r| &self.text[r])
    }

    /// Inserts a typed character at the active cursor position, replacing any selection.
    pub fn insert_char(&mut self, ch: char) {
        text_input::insert_char(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            ch,
        );
    }

    /// Inserts a string at the active cursor position, replacing any selection.
    pub fn insert_str(&mut self, s: &str) {
        text_input::insert_str(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            s,
        );
    }

    /// Deletes the character before the active cursor or the active selection.
    pub fn backspace(&mut self) {
        text_input::backspace(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            false,
        );
    }

    /// Deletes the character after the active cursor or the active selection.
    pub fn delete(&mut self) {
        text_input::delete(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            false,
        );
    }

    /// Moves cursor left by single character.
    pub fn move_cursor_left(&mut self, extend_selection: bool) {
        text_input::move_left(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            false,
            extend_selection,
        );
    }

    /// Moves cursor right by single character.
    pub fn move_cursor_right(&mut self, extend_selection: bool) {
        text_input::move_right(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            false,
            extend_selection,
        );
    }

    /// Moves cursor left by word.
    pub fn move_word_left(&mut self, extend_selection: bool) {
        text_input::move_left(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            true,
            extend_selection,
        );
    }

    /// Moves cursor right by word.
    pub fn move_word_right(&mut self, extend_selection: bool) {
        text_input::move_right(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            true,
            extend_selection,
        );
    }

    /// Moves cursor to beginning of input.
    pub fn move_cursor_start(&mut self, extend_selection: bool) {
        text_input::move_start(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend_selection,
        );
    }

    /// Moves cursor to end of input.
    pub fn move_cursor_end(&mut self, extend_selection: bool) {
        text_input::move_end(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            extend_selection,
        );
    }

    /// Selects all text in the input box.
    pub fn select_all(&mut self) {
        text_input::select_all(
            &self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
        );
    }

    /// Clears any active selection without moving the cursor.
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Handles a unified `TextInputAction` event.
    pub fn handle_action(
        &mut self,
        action: TextInputAction,
        clipboard_text: Option<&str>,
    ) -> TextInputResult {
        text_input::handle_action(
            &mut self.text,
            &mut self.cursor_pos,
            &mut self.selection_anchor,
            action,
            clipboard_text,
        )
    }

    /// Renders the text box, text, selection highlight, and active cursor.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render(&self, font: &BitmapFont, scale: u32, tick: u64, out: &mut Vec<UiQuad>) {
        let px_x = snap_to_physical(self.x, scale);
        let px_y = snap_to_physical(self.y, scale);
        let px_w = snap_to_physical(self.width, scale) as u16;
        let px_h = snap_to_physical(self.height, scale) as u16;
        let border_th = scale.max(1) as u16;

        // Background
        out.push(UiQuad::solid(
            [px_x, px_y],
            [px_w, px_h],
            UiQuad::rgba(20, 20, 25, 240),
        ));

        // Border: gold/white if focused
        let border_col = if self.focused {
            UiQuad::rgba(255, 255, 255, 255)
        } else if self.hovered {
            UiQuad::rgba(160, 160, 160, 255)
        } else {
            UiQuad::rgba(40, 40, 45, 255)
        };
        out.push(UiQuad::solid([px_x, px_y], [px_w, border_th], border_col));
        out.push(UiQuad::solid(
            [px_x, px_y + i32::from(px_h) - i32::from(border_th)],
            [px_w, border_th],
            border_col,
        ));
        out.push(UiQuad::solid([px_x, px_y], [border_th, px_h], border_col));
        out.push(UiQuad::solid(
            [px_x + i32::from(px_w) - i32::from(border_th), px_y],
            [border_th, px_h],
            border_col,
        ));

        // Text & placeholder
        let text_y = self.y + (self.height - 8.0) * 0.5;
        let text_x = self.x + 6.0;

        if self.text.is_empty() && !self.focused {
            font.layout_text(
                &self.placeholder,
                text_x,
                text_y,
                UiQuad::rgba(120, 120, 120, 255),
                false,
                scale,
                out,
            );
        } else {
            // Render selection highlight box behind text if active
            if self.focused && let Some(range) = self.selection_range() {
                let (prefix_w, _) = font.measure_text(&self.text[..range.start]);
                let (sel_w, _) = font.measure_text(&self.text[range.start..range.end]);
                let sel_x = text_x + prefix_w;
                let px_sel_x = snap_to_physical(sel_x, scale);
                let px_sel_y = snap_to_physical(text_y - 1.0, scale);
                let px_sel_w = snap_to_physical(sel_w, scale) as u16;
                let px_sel_h = snap_to_physical(10.0, scale) as u16;

                out.push(UiQuad::solid(
                    [px_sel_x, px_sel_y],
                    [px_sel_w, px_sel_h],
                    UiQuad::rgba(40, 110, 200, 180),
                ));
            }

            font.layout_text(
                &self.text,
                text_x,
                text_y,
                UiQuad::rgba(255, 255, 255, 255),
                true,
                scale,
                out,
            );
        }

        // Blinking cursor
        if self.focused && (tick / 30).is_multiple_of(2) {
            let prefix = &self.text[..self.cursor_pos.min(self.text.len())];
            let (cursor_offset_x, _) = font.measure_text(prefix);
            let cur_x = text_x + cursor_offset_x;
            font.layout_text(
                "_",
                cur_x,
                text_y,
                UiQuad::rgba(255, 255, 255, 255),
                true,
                scale,
                out,
            );
        }
    }
}
