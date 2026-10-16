//! Modernized sleek settings and options screen.
//!
//! Features full-width sleek capsule setting rows, a centered golden header,
//! warm gold tab navigation indicators, right-aligned stepper pills and diamond
//! sliders, interactive key rebinding with conflict detection, and smooth scrolling.

use crate::font::BitmapFont;
use crate::keybinds::{InputKey, KeyAction, KeyCategory};
use crate::menu::widgets::{MenuButton, MenuSlider};
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;
use crate::settings::GameSettings;
use telos_core::i18n::LanguageCatalog;

/// Active tab within the options screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    /// Video and graphics options.
    #[default]
    Video,
    /// Audio volume sliders.
    Audio,
    /// Mouse and keyboard controls with interactive rebinding.
    Controls,
    /// Language selection.
    Language,
}

/// Category item displayed in the settings list.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingRowItem {
    /// Section divider header (e.g. "RENDERING", "VOLUME CHANNELS").
    SectionHeader(String),
    /// Interactive slider row with label and right-aligned diamond track + readout.
    SliderRow {
        /// Index into `self.sliders`.
        slider_idx: usize,
        /// Label displayed on the left side of the row.
        label: String,
    },
    /// Interactive toggle/stepper row with label and right-aligned `< VALUE >` pill.
    ToggleRow {
        /// Index into `self.toggle_buttons`.
        button_idx: usize,
        /// Label displayed on the left side of the row.
        label: String,
    },
    /// Interactive key rebinding row.
    KeybindRow {
        /// Associated key action.
        action: KeyAction,
        /// Index into `self.keybind_buttons`.
        button_idx: usize,
        /// Display label for the key action.
        label: String,
    },
    /// Language selection row.
    LanguageRow {
        /// Index into `self.toggle_buttons`.
        button_idx: usize,
        /// Locale code (e.g. `en_us`).
        code: &'static str,
        /// Human-readable language display name.
        display: &'static str,
        /// Whether this language is currently active.
        is_active: bool,
    },
    /// Action button row (e.g. Reset Keybinds).
    ActionRow {
        /// Button display label.
        label: String,
    },
}

/// Positioned row container within the settings view.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingRow {
    /// Top Y position in GUI pixels (includes scroll offset).
    pub y: f32,
    /// Height in GUI pixels.
    pub height: f32,
    /// Concrete setting item kind.
    pub item: SettingRowItem,
}

/// Settings and options screen state with authentic modern layout.
#[derive(Debug, Clone)]
pub struct SettingsScreen {
    /// Current editable settings.
    pub settings: GameSettings,
    /// Active tab.
    pub active_tab: SettingsTab,
    /// Category tab buttons.
    pub tab_buttons: Vec<MenuButton>,
    /// Sliders for current active tab.
    pub sliders: Vec<MenuSlider>,
    /// Toggle buttons for current active tab.
    pub toggle_buttons: Vec<MenuButton>,
    /// Interactive keybind buttons for Controls tab: `(action, button)`.
    pub keybind_buttons: Vec<(KeyAction, MenuButton)>,
    /// Action currently awaiting a physical key press for rebinding.
    pub listening_action: Option<KeyAction>,
    /// Button to reset keybinds to defaults.
    pub reset_keybinds_button: MenuButton,
    /// Left footer Back / Escape pill button.
    pub back_button: MenuButton,
    /// Done / Save confirmation button.
    pub done_button: MenuButton,
    /// Dragging slider index.
    pub active_drag_slider: Option<usize>,
    /// Vertical scroll offset in GUI pixels for scrollable tabs.
    pub scroll_y: f32,
    /// Maximum scrollable distance in GUI pixels.
    pub max_scroll_y: f32,
    /// Screen title text.
    pub title: String,
    /// Data-driven row layout for active tab.
    pub rows: Vec<SettingRow>,
}

impl SettingsScreen {
    /// Creates a new `SettingsScreen` loaded with the given `GameSettings`.
    #[must_use]
    pub fn new(settings: GameSettings) -> Self {
        Self {
            settings,
            active_tab: SettingsTab::Video,
            tab_buttons: Vec::new(),
            sliders: Vec::new(),
            toggle_buttons: Vec::new(),
            keybind_buttons: Vec::new(),
            listening_action: None,
            reset_keybinds_button: MenuButton::new(501, 0.0, 0.0, 160.0, 22.0, "Reset Keybinds"),
            back_button: MenuButton::new(98, 0.0, 0.0, 75.0, 20.0, "[ ESC ] Back"),
            done_button: MenuButton::new(99, 0.0, 0.0, 95.0, 20.0, "Done"),
            active_drag_slider: None,
            scroll_y: 0.0,
            max_scroll_y: 0.0,
            title: "SETTINGS".to_string(),
            rows: Vec::new(),
        }
    }

    /// Returns true if an action is currently waiting for a physical key press.
    #[must_use]
    pub const fn is_rebinding(&self) -> bool {
        self.listening_action.is_some()
    }

    /// Cancels any active key rebinding.
    pub fn cancel_rebinding(&mut self) {
        self.listening_action = None;
    }

    /// Processes an incoming physical key for rebinding. Returns true if a key was rebound.
    pub fn handle_key_input(&mut self, key: InputKey) -> bool {
        if let Some(action) = self.listening_action {
            if key == InputKey::Escape {
                self.listening_action = None;
                return false;
            }
            if key != InputKey::None {
                self.settings.controls.keybinds.set(action, key);
                self.listening_action = None;
                return true;
            }
        }
        false
    }

    /// Adjusts the scroll offset from a mouse wheel delta.
    pub fn handle_mouse_wheel(&mut self, delta: f32) {
        if self.max_scroll_y > 0.0 {
            self.scroll_y = (self.scroll_y - delta * 24.0).clamp(0.0, self.max_scroll_y);
        }
    }

    /// Returns true if any interactive button or slider is currently hovered by the mouse.
    #[must_use]
    pub fn has_hovered_element(&self) -> bool {
        self.tab_buttons.iter().any(|b| b.hovered)
            || self.sliders.iter().any(|s| s.hovered)
            || self.toggle_buttons.iter().any(|b| b.hovered)
            || self.keybind_buttons.iter().any(|(_, b)| b.hovered)
            || self.reset_keybinds_button.hovered
            || self.back_button.hovered
            || self.done_button.hovered
    }

    /// Rebuilds tab and widget layouts for the given viewport using the specified language catalog.
    #[allow(clippy::too_many_lines)]
    pub fn update_layout_i18n(
        &mut self,
        width_gui: f32,
        height_gui: f32,
        catalog: &LanguageCatalog,
    ) {
        let center_x = width_gui * 0.5;
        let col_w = 440.0f32.min(width_gui - 32.0);
        let col_x = center_x - col_w * 0.5;

        // Centered tab header buttons
        let tab_w = 76.0;
        let tab_h = 18.0;
        let tab_gap = 12.0;
        let total_tabs_w = 4.0 * tab_w + 3.0 * tab_gap;
        let start_tab_x = center_x - total_tabs_w * 0.5;
        let tab_y = 28.0;

        self.tab_buttons = vec![
            MenuButton::new(
                101,
                start_tab_x,
                tab_y,
                tab_w,
                tab_h,
                catalog.translate("options.tab.video"),
            ),
            MenuButton::new(
                102,
                start_tab_x + tab_w + tab_gap,
                tab_y,
                tab_w,
                tab_h,
                catalog.translate("options.tab.audio"),
            ),
            MenuButton::new(
                103,
                start_tab_x + (tab_w + tab_gap) * 2.0,
                tab_y,
                tab_w,
                tab_h,
                catalog.translate("options.tab.controls"),
            ),
            MenuButton::new(
                104,
                start_tab_x + (tab_w + tab_gap) * 3.0,
                tab_y,
                tab_w,
                tab_h,
                catalog.translate("options.tab.language"),
            ),
        ];

        let content_top = 54.0;
        let content_bottom = height_gui - 34.0;
        let visible_h = (content_bottom - content_top).max(50.0);

        self.sliders.clear();
        self.toggle_buttons.clear();
        self.keybind_buttons.clear();
        self.rows.clear();

        let row_h = 24.0;
        let row_step = 28.0;
        let pill_w = 130.0;
        let pill_h = 18.0;
        let pill_x = col_x + col_w - pill_w - 4.0;
        let track_w = 110.0;
        let readout_w = 65.0;
        let slider_x = col_x + col_w - track_w - readout_w - 6.0;

        let on_str = catalog.translate("options.on");
        let off_str = catalog.translate("options.off");

        let mut cur_y = content_top + 6.0 - self.scroll_y;

        match self.active_tab {
            SettingsTab::Video => {
                // --- Section: RENDERING ---
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("RENDERING".to_string()),
                });
                cur_y += 20.0;

                // Slider 1: Render Distance
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    1,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Render Distance",
                    2.0,
                    32.0,
                    self.settings.video.view_distance as f32,
                    " Chunks",
                    true,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Render Distance".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 2: Vertical Distance
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    2,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Vertical Distance",
                    2.0,
                    24.0,
                    self.settings.video.vertical_view_distance as f32,
                    " Chunks",
                    true,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Vertical Distance".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 5: Simulation Distance
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    5,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Simulation Distance",
                    2.0,
                    32.0,
                    self.settings.video.simulation_distance as f32,
                    " Chunks",
                    true,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Simulation Distance".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 3: FOV
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    3,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Field of View",
                    30.0,
                    110.0,
                    self.settings.video.fov,
                    "°",
                    true,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Field of View (FOV)".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 4: Framerate Limit
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    4,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "FPS Limit",
                    0.0,
                    360.0,
                    self.settings.video.fps_limit as f32,
                    " FPS",
                    true,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Framerate Limit".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 201: VSync
                let b_idx = self.toggle_buttons.len();
                let vsync_val = if self.settings.video.vsync {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    201,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {vsync_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Vertical Sync (VSync)".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 203: GUI Scale
                let b_idx = self.toggle_buttons.len();
                let gui_val = if self.settings.video.gui_scale == 0 {
                    catalog.translate("options.guiScale.auto")
                } else {
                    &format!("{}", self.settings.video.gui_scale)
                };
                self.toggle_buttons.push(MenuButton::new(
                    203,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {gui_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "GUI Scale".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 202: Hi-Z Occlusion Culling
                let b_idx = self.toggle_buttons.len();
                let cull_val = if self.settings.video.no_cull {
                    off_str
                } else {
                    on_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    202,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {cull_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Hi-Z Occlusion Culling".to_string(),
                    },
                });
                cur_y += row_step;

                // --- Section: ADVANCED RENDERING & SHADERS ---
                cur_y += 8.0;
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("ADVANCED RENDERING & SHADERS".to_string()),
                });
                cur_y += 20.0;

                // Toggle 205: Post-Processing
                let b_idx = self.toggle_buttons.len();
                let pp_val = if self.settings.video.post_processing {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    205,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {pp_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Post-Processing Pipeline".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 206: SSAO
                let b_idx = self.toggle_buttons.len();
                let ssao_val = if self.settings.video.ssao {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    206,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {ssao_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Screen Space Ambient Occlusion (SSAO)".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 207: Volumetric Fog
                let b_idx = self.toggle_buttons.len();
                let fog_val = if self.settings.video.volumetric_fog {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    207,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {fog_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Volumetric Fog & Sunlight".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 209: Volumetric Clouds
                let b_idx = self.toggle_buttons.len();
                let clouds_val = if self.settings.video.volumetric_clouds {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    209,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {clouds_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Volumetric Clouds".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 212: Shadows
                let b_idx = self.toggle_buttons.len();
                let shadows_val = if self.settings.video.shadows {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    212,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {shadows_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Cascaded Directional Shadows".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 213: FXAA
                let b_idx = self.toggle_buttons.len();
                let fxaa_val = if self.settings.video.fxaa {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    213,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {fxaa_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Fast Approximate Anti-Aliasing (FXAA)".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 214: Underwater Effects
                let b_idx = self.toggle_buttons.len();
                let underwater_val = if self.settings.video.underwater_effects {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    214,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {underwater_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Underwater Visual Distortion & FX".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 215: SSR
                let b_idx = self.toggle_buttons.len();
                let ssr_val = if self.settings.video.ssr {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    215,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {ssr_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Water Screen-Space Reflections (SSR)".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 208: Tonemapping
                let b_idx = self.toggle_buttons.len();
                let tone_val = if self.settings.video.tonemapping {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    208,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {tone_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "ACES Cinematic Tonemapping".to_string(),
                    },
                });
                cur_y += row_step;
            }
            SettingsTab::Audio => {
                // --- Section: VOLUME CHANNELS ---
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("VOLUME CHANNELS".to_string()),
                });
                cur_y += 20.0;

                // Slider 11: Master
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    11,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Master",
                    0.0,
                    1.0,
                    self.settings.audio.master_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Master Volume".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 12: Music
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    12,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Music",
                    0.0,
                    1.0,
                    self.settings.audio.music_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Music & Ambience".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 13: Weather
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    13,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Weather",
                    0.0,
                    1.0,
                    self.settings.audio.weather_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Weather & Environment".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 14: Blocks
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    14,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Blocks",
                    0.0,
                    1.0,
                    self.settings.audio.blocks_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Blocks & Actions".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 15: Entities
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    15,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Entities",
                    0.0,
                    1.0,
                    self.settings.audio.entities_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Creatures & Entities".to_string(),
                    },
                });
                cur_y += row_step;

                // --- Section: PROXIMITY VOICE CHAT ---
                cur_y += 8.0;
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("PROXIMITY VOICE CHAT".to_string()),
                });
                cur_y += 20.0;

                // Toggle 210: Voice Chat
                let b_idx = self.toggle_buttons.len();
                let vc_val = if self.settings.audio.voice_chat_enabled {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    210,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {vc_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Voice Chat".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 16: Voice Volume
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    16,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Voice Volume",
                    0.0,
                    2.0,
                    self.settings.audio.voice_chat_volume,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Voice Volume".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 211: Mic Mode
                let b_idx = self.toggle_buttons.len();
                let ptt_val = if self.settings.audio.push_to_talk {
                    "PTT (V)"
                } else {
                    "Open Mic"
                };
                self.toggle_buttons.push(MenuButton::new(
                    211,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {ptt_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Microphone Activation Mode".to_string(),
                    },
                });
                cur_y += row_step;

                // Slider 17: Mic Sensitivity
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    17,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Mic Sensitivity",
                    0.005,
                    0.20,
                    self.settings.audio.mic_sensitivity,
                    "%",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Microphone Sensitivity".to_string(),
                    },
                });
                cur_y += row_step;
            }
            SettingsTab::Controls => {
                // --- Section: MOUSE SETTINGS ---
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("MOUSE SETTINGS".to_string()),
                });
                cur_y += 20.0;

                // Slider 21: Mouse Sensitivity
                let s_idx = self.sliders.len();
                self.sliders.push(MenuSlider::new(
                    21,
                    slider_x,
                    cur_y + 2.0,
                    track_w,
                    row_h - 4.0,
                    "Sensitivity",
                    0.1,
                    3.0,
                    self.settings.controls.mouse_sensitivity,
                    "x",
                    false,
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::SliderRow {
                        slider_idx: s_idx,
                        label: "Mouse Sensitivity".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 204: Invert Y
                let b_idx = self.toggle_buttons.len();
                let inv_val = if self.settings.controls.invert_mouse_y {
                    on_str
                } else {
                    off_str
                };
                self.toggle_buttons.push(MenuButton::new(
                    204,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {inv_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Invert Mouse Y-Axis".to_string(),
                    },
                });
                cur_y += row_step;

                // Toggle 216: Main Hand
                let b_idx = self.toggle_buttons.len();
                let hand_val = if self.settings.controls.left_handed {
                    "Left"
                } else {
                    "Right"
                };
                self.toggle_buttons.push(MenuButton::new(
                    216,
                    pill_x,
                    cur_y + 3.0,
                    pill_w,
                    pill_h,
                    format!("< {hand_val} >"),
                ));
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ToggleRow {
                        button_idx: b_idx,
                        label: "Main Hand".to_string(),
                    },
                });
                cur_y += row_step;

                // --- Keybind Sections ---
                let categories = [
                    (KeyCategory::Movement, "MOVEMENT CONTROLS"),
                    (KeyCategory::Gameplay, "GAMEPLAY & INTERACTION"),
                    (KeyCategory::Hotbar, "HOTBAR SLOTS"),
                    (KeyCategory::Multiplayer, "MULTIPLAYER & VOICE"),
                    (KeyCategory::System, "SYSTEM & DEBUG"),
                ];

                let kb_btn_w = 110.0;
                let kb_btn_h = 18.0;
                let kb_btn_x = col_x + col_w - kb_btn_w - 4.0;

                for (cat, title) in categories {
                    cur_y += 8.0;
                    self.rows.push(SettingRow {
                        y: cur_y,
                        height: 18.0,
                        item: SettingRowItem::SectionHeader(title.to_string()),
                    });
                    cur_y += 20.0;

                    for &action in KeyAction::all() {
                        if action.category() != cat {
                            continue;
                        }

                        let is_listening = self.listening_action == Some(action);
                        let label = if is_listening {
                            "> PRESS KEY <".to_string()
                        } else {
                            let key = self.settings.controls.keybinds.get(action);
                            format!("[ {} ]", key.display_name())
                        };

                        let btn_id = 1000 + action as u32;
                        let btn = MenuButton::new(
                            btn_id,
                            kb_btn_x,
                            cur_y + 3.0,
                            kb_btn_w,
                            kb_btn_h,
                            label,
                        );
                        let b_idx = self.keybind_buttons.len();
                        self.keybind_buttons.push((action, btn));

                        self.rows.push(SettingRow {
                            y: cur_y,
                            height: row_h,
                            item: SettingRowItem::KeybindRow {
                                action,
                                button_idx: b_idx,
                                label: action.display_name().to_string(),
                            },
                        });

                        cur_y += row_step;
                    }
                }

                // Reset keybinds button
                cur_y += 8.0;
                let rst_w = 180.0;
                let rst_x = center_x - rst_w * 0.5;
                self.reset_keybinds_button = MenuButton::new(
                    501,
                    rst_x,
                    cur_y + 2.0,
                    rst_w,
                    20.0,
                    "Reset Keybinds to Defaults",
                );
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: row_h,
                    item: SettingRowItem::ActionRow {
                        label: "Reset Keybinds to Defaults".to_string(),
                    },
                });
                cur_y += row_step + 4.0;
            }
            SettingsTab::Language => {
                self.rows.push(SettingRow {
                    y: cur_y,
                    height: 18.0,
                    item: SettingRowItem::SectionHeader("SELECT LANGUAGE".to_string()),
                });
                cur_y += 20.0;

                let langs: [(&str, &str); 5] = [
                    ("en_us", "English (US)"),
                    ("es_es", "Espanol (Espana)"),
                    ("de_de", "Deutsch (Deutschland)"),
                    ("fr_fr", "Francais (France)"),
                    ("tr_tr", "Turkce (Turkiye)"),
                ];

                for (i, (code, display)) in langs.iter().enumerate() {
                    let is_active = self.settings.gameplay.language == *code;
                    let label = if is_active {
                        format!("> {display} <")
                    } else {
                        (*display).to_string()
                    };

                    let b_idx = self.toggle_buttons.len();
                    self.toggle_buttons.push(MenuButton::new(
                        301 + i as u32,
                        col_x,
                        cur_y,
                        col_w,
                        row_h,
                        label,
                    ));

                    self.rows.push(SettingRow {
                        y: cur_y,
                        height: row_h,
                        item: SettingRowItem::LanguageRow {
                            button_idx: b_idx,
                            code,
                            display,
                            is_active,
                        },
                    });

                    cur_y += row_step;
                }
            }
        }

        // Calculate max scroll bounds
        let total_content_h = cur_y + self.scroll_y - content_top;
        self.max_scroll_y = (total_content_h - visible_h + 12.0).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y);

        // Footer buttons
        let foot_y = height_gui - 26.0;
        self.back_button = MenuButton::new(98, col_x, foot_y, 75.0, 20.0, "[ ESC ] Back");

        let done_w = 100.0;
        self.done_button = MenuButton::new(
            99,
            col_x + col_w - done_w,
            foot_y,
            done_w,
            20.0,
            catalog.translate("gui.done"),
        );
    }

    /// Rebuilds tab and widget layouts using embedded default catalog.
    pub fn update_layout(&mut self, width_gui: f32, height_gui: f32) {
        let catalog = LanguageCatalog::with_default_embedded();
        self.update_layout_i18n(width_gui, height_gui, &catalog);
    }

    /// Handles mouse motion in GUI pixels.
    pub fn handle_mouse_move(&mut self, mouse_x: f32, mouse_y: f32) {
        if let Some(idx) = self.active_drag_slider {
            if let Some(slider) = self.sliders.get_mut(idx) {
                slider.update_from_mouse_x(mouse_x);
                self.sync_settings_from_sliders();
            }
            return;
        }

        let content_top = 48.0;

        for btn in &mut self.tab_buttons {
            btn.hovered = btn.contains(mouse_x, mouse_y);
        }

        let in_content_zone = mouse_y >= content_top;

        for slider in &mut self.sliders {
            let is_visible = slider.y + slider.height > content_top;
            slider.hovered = in_content_zone && is_visible && slider.contains(mouse_x, mouse_y);
        }
        for btn in &mut self.toggle_buttons {
            let is_visible = btn.y + btn.height > content_top;
            btn.hovered = in_content_zone && is_visible && btn.contains(mouse_x, mouse_y);
        }
        for (_, btn) in &mut self.keybind_buttons {
            let is_visible = btn.y + btn.height > content_top;
            btn.hovered = in_content_zone && is_visible && btn.contains(mouse_x, mouse_y);
        }
        let reset_kb_vis =
            self.reset_keybinds_button.y + self.reset_keybinds_button.height > content_top;
        self.reset_keybinds_button.hovered = in_content_zone
            && reset_kb_vis
            && self.reset_keybinds_button.contains(mouse_x, mouse_y);

        self.back_button.hovered = self.back_button.contains(mouse_x, mouse_y);
        self.done_button.hovered = self.done_button.contains(mouse_x, mouse_y);
    }

    /// Handles mouse click with internationalization support. Returns true if "Done" or "Back" was clicked.
    #[allow(clippy::too_many_lines)]
    pub fn handle_mouse_click_i18n(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        width_gui: f32,
        height_gui: f32,
        catalog: &LanguageCatalog,
    ) -> bool {
        if self.done_button.contains(mouse_x, mouse_y)
            || self.back_button.contains(mouse_x, mouse_y)
        {
            self.listening_action = None;
            return true;
        }

        let center_x = width_gui * 0.5;
        let col_w = 440.0f32.min(width_gui - 32.0);
        let col_x = center_x - col_w * 0.5;

        // Tab selection
        for btn in &self.tab_buttons {
            if btn.contains(mouse_x, mouse_y) {
                match btn.id {
                    101 => self.active_tab = SettingsTab::Video,
                    102 => self.active_tab = SettingsTab::Audio,
                    103 => self.active_tab = SettingsTab::Controls,
                    104 => self.active_tab = SettingsTab::Language,
                    _ => {}
                }
                self.scroll_y = 0.0;
                self.listening_action = None;
                self.update_layout_i18n(width_gui, height_gui, catalog);
                return false;
            }
        }

        let content_top = 48.0;

        // Clicks in top header area do not pass through to rows underneath
        if mouse_y < content_top {
            return false;
        }

        // Keybind buttons in Controls tab
        if self.active_tab == SettingsTab::Controls {
            for (action, btn) in &self.keybind_buttons {
                let is_visible = btn.y + btn.height > content_top;
                if !is_visible {
                    continue;
                }
                let row_contains = mouse_x >= col_x
                    && mouse_x <= col_x + col_w
                    && mouse_y >= btn.y - 3.0
                    && mouse_y <= btn.y + btn.height + 3.0;

                if btn.contains(mouse_x, mouse_y) || row_contains {
                    if self.listening_action == Some(*action) {
                        self.listening_action = None;
                    } else {
                        self.listening_action = Some(*action);
                    }
                    self.update_layout_i18n(width_gui, height_gui, catalog);
                    return false;
                }
            }

            // Reset keybinds button
            let reset_kb_vis =
                self.reset_keybinds_button.y + self.reset_keybinds_button.height > content_top;
            if reset_kb_vis && self.reset_keybinds_button.contains(mouse_x, mouse_y) {
                self.settings.controls.keybinds.reset_defaults();
                self.listening_action = None;
                self.update_layout_i18n(width_gui, height_gui, catalog);
                return false;
            }
        }

        // Sliders
        for (i, slider) in self.sliders.iter_mut().enumerate() {
            let is_visible = slider.y + slider.height > content_top;
            if is_visible && slider.contains(mouse_x, mouse_y) {
                slider.dragging = true;
                slider.update_from_mouse_x(mouse_x);
                self.active_drag_slider = Some(i);
                self.sync_settings_from_sliders();
                return false;
            }
        }

        // Toggles & language selectors
        for btn in &mut self.toggle_buttons {
            let is_visible = btn.y + btn.height > content_top;
            if !is_visible {
                continue;
            }
            let row_contains = mouse_x >= col_x
                && mouse_x <= col_x + col_w
                && mouse_y >= btn.y - 3.0
                && mouse_y <= btn.y + btn.height + 3.0;

            if btn.contains(mouse_x, mouse_y) || row_contains {
                match btn.id {
                    201 => {
                        self.settings.video.vsync = !self.settings.video.vsync;
                    }
                    202 => {
                        self.settings.video.no_cull = !self.settings.video.no_cull;
                    }
                    203 => {
                        self.settings.video.gui_scale = (self.settings.video.gui_scale + 1) % 5;
                    }
                    204 => {
                        self.settings.controls.invert_mouse_y =
                            !self.settings.controls.invert_mouse_y;
                    }
                    216 => {
                        self.settings.controls.left_handed = !self.settings.controls.left_handed;
                    }
                    205 => {
                        self.settings.video.post_processing = !self.settings.video.post_processing;
                    }
                    206 => {
                        self.settings.video.ssao = !self.settings.video.ssao;
                    }
                    207 => {
                        self.settings.video.volumetric_fog = !self.settings.video.volumetric_fog;
                    }
                    208 => {
                        self.settings.video.tonemapping = !self.settings.video.tonemapping;
                    }
                    209 => {
                        self.settings.video.volumetric_clouds =
                            !self.settings.video.volumetric_clouds;
                    }
                    212 => {
                        self.settings.video.shadows = !self.settings.video.shadows;
                    }
                    213 => {
                        self.settings.video.fxaa = !self.settings.video.fxaa;
                    }
                    214 => {
                        self.settings.video.underwater_effects =
                            !self.settings.video.underwater_effects;
                    }
                    215 => {
                        self.settings.video.ssr = !self.settings.video.ssr;
                    }
                    210 => {
                        self.settings.audio.voice_chat_enabled =
                            !self.settings.audio.voice_chat_enabled;
                    }
                    211 => {
                        self.settings.audio.push_to_talk = !self.settings.audio.push_to_talk;
                    }
                    301 => self.settings.gameplay.language = "en_us".to_string(),
                    302 => self.settings.gameplay.language = "es_es".to_string(),
                    303 => self.settings.gameplay.language = "de_de".to_string(),
                    304 => self.settings.gameplay.language = "fr_fr".to_string(),
                    305 => self.settings.gameplay.language = "tr_tr".to_string(),
                    _ => {}
                }

                // If language was changed, adapt the active locale for this layout update
                let mut local_catalog;
                let active_cat = if self.settings.gameplay.language == catalog.active_locale() {
                    catalog
                } else {
                    local_catalog = catalog.clone();
                    local_catalog.set_active_locale(&self.settings.gameplay.language);
                    &local_catalog
                };

                self.update_layout_i18n(width_gui, height_gui, active_cat);
                return false;
            }
        }

        false
    }

    /// Handles mouse button press. Returns true if "Done" or "Back" was clicked.
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        width_gui: f32,
        height_gui: f32,
    ) -> bool {
        let catalog = LanguageCatalog::with_default_embedded();
        self.handle_mouse_click_i18n(mouse_x, mouse_y, width_gui, height_gui, &catalog)
    }

    /// Handles mouse release.
    pub fn handle_mouse_up(&mut self) {
        if let Some(idx) = self.active_drag_slider {
            if let Some(slider) = self.sliders.get_mut(idx) {
                slider.dragging = false;
            }
            self.active_drag_slider = None;
        }
    }

    fn sync_settings_from_sliders(&mut self) {
        for s in &self.sliders {
            match s.id {
                1 => self.settings.video.view_distance = s.value as u32,
                2 => self.settings.video.vertical_view_distance = s.value as u32,
                3 => self.settings.video.fov = s.value,
                4 => self.settings.video.fps_limit = s.value as u32,
                5 => self.settings.video.simulation_distance = s.value as u32,
                11 => self.settings.audio.master_volume = s.value,
                12 => self.settings.audio.music_volume = s.value,
                13 => self.settings.audio.weather_volume = s.value,
                14 => self.settings.audio.blocks_volume = s.value,
                15 => self.settings.audio.entities_volume = s.value,
                16 => self.settings.audio.voice_chat_volume = s.value,
                17 => self.settings.audio.mic_sensitivity = s.value,
                21 => self.settings.controls.mouse_sensitivity = s.value,
                _ => {}
            }
        }
    }

    /// Renders the settings screen.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::similar_names
    )]
    pub fn render(
        &self,
        font: &BitmapFont,
        width_gui: f32,
        height_gui: f32,
        scale: u32,
        out: &mut Vec<UiQuad>,
    ) {
        let px_w = snap_to_physical(width_gui, scale) as u16;
        let px_h = snap_to_physical(height_gui, scale) as u16;
        let center_x = width_gui * 0.5;
        let col_w = 440.0f32.min(width_gui - 32.0);
        let col_x = center_x - col_w * 0.5;

        // Midnight-navy backdrop
        out.push(UiQuad::solid(
            [0, 0],
            [px_w, px_h],
            UiQuad::rgba(10, 13, 20, 248),
        ));

        // Visible scrolling bounds
        let tab_line_y = 48.0;
        let content_top = 48.0;
        let bar_h = 32.0;
        let content_bottom = height_gui - bar_h;

        let conflicts = if self.active_tab == SettingsTab::Controls {
            self.settings.controls.keybinds.find_conflicts()
        } else {
            Vec::new()
        };

        // Render capsule rows (Content Layer - drawn underneath sticky bars)
        for row in &self.rows {
            if row.y + row.height <= content_top || row.y >= content_bottom {
                continue;
            }

            match &row.item {
                SettingRowItem::SectionHeader(title) => {
                    font.layout_text(
                        title,
                        col_x + 2.0,
                        row.y + 4.0,
                        UiQuad::rgba(140, 160, 184, 255),
                        true,
                        scale,
                        out,
                    );
                }
                SettingRowItem::SliderRow { slider_idx, label } => {
                    if let Some(slider) = self.sliders.get(*slider_idx) {
                        let is_row_hovered = slider.hovered || slider.dragging;
                        render_capsule_row(
                            col_x,
                            row.y,
                            col_w,
                            row.height,
                            is_row_hovered,
                            scale,
                            out,
                        );

                        // Left label
                        font.layout_text(
                            label,
                            col_x + 12.0,
                            row.y + 7.5,
                            UiQuad::rgba(230, 237, 245, 255),
                            true,
                            scale,
                            out,
                        );

                        // Slider recessed track
                        let track_x = slider.x;
                        let track_w = slider.width;
                        let track_h = 3.0f32;
                        let track_y = row.y + 10.5;

                        let px_tx = snap_to_physical(track_x, scale);
                        let px_ty = snap_to_physical(track_y, scale);
                        let px_tw = snap_to_physical(track_w, scale) as u16;
                        let px_th = snap_to_physical(track_h, scale) as u16;

                        // Groove track border and fill
                        out.push(UiQuad::solid(
                            [px_tx, px_ty],
                            [px_tw, px_th],
                            UiQuad::rgba(14, 18, 28, 255),
                        ));

                        // Active fill bar
                        let frac = if (slider.max - slider.min).abs() > 1e-4 {
                            ((slider.value - slider.min) / (slider.max - slider.min))
                                .clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let fill_w = (track_w * frac).max(1.0);
                        let px_fw = snap_to_physical(fill_w, scale) as u16;
                        out.push(UiQuad::solid(
                            [px_tx, px_ty],
                            [px_fw, px_th],
                            UiQuad::rgba(45, 75, 120, 255),
                        ));

                        // Golden diamond thumb
                        let thumb_cx = track_x + frac * track_w;
                        let thumb_cy = track_y + track_h * 0.5;
                        render_diamond_thumb(
                            thumb_cx,
                            thumb_cy,
                            scale,
                            slider.hovered || slider.dragging,
                            out,
                        );

                        // Right value readout
                        let readout_str = format_slider_readout(slider);
                        let readout_x = track_x + track_w + 8.0;
                        font.layout_text(
                            &readout_str,
                            readout_x,
                            row.y + 7.5,
                            UiQuad::rgba(255, 215, 100, 255),
                            true,
                            scale,
                            out,
                        );
                    }
                }
                SettingRowItem::ToggleRow { button_idx, label } => {
                    if let Some(btn) = self.toggle_buttons.get(*button_idx) {
                        render_capsule_row(
                            col_x,
                            row.y,
                            col_w,
                            row.height,
                            btn.hovered,
                            scale,
                            out,
                        );

                        // Left label
                        font.layout_text(
                            label,
                            col_x + 12.0,
                            row.y + 7.5,
                            UiQuad::rgba(230, 237, 245, 255),
                            true,
                            scale,
                            out,
                        );

                        // Right-aligned stepper pill button: [ < VALUE > ]
                        let px_bx = snap_to_physical(btn.x, scale);
                        let px_by = snap_to_physical(btn.y, scale);
                        let px_bw = snap_to_physical(btn.width, scale) as u16;
                        let px_bh = snap_to_physical(btn.height, scale) as u16;
                        let b_th = scale.max(1) as u16;

                        let pill_border = if btn.hovered {
                            UiQuad::rgba(68, 96, 144, 255)
                        } else {
                            UiQuad::rgba(44, 60, 88, 200)
                        };
                        let pill_bg = if btn.hovered {
                            UiQuad::rgba(42, 60, 92, 255)
                        } else {
                            UiQuad::rgba(28, 38, 58, 235)
                        };

                        out.push(UiQuad::solid([px_bx, px_by], [px_bw, px_bh], pill_border));
                        let in_x = px_bx + i32::from(b_th);
                        let in_y = px_by + i32::from(b_th);
                        let in_w = px_bw.saturating_sub(b_th * 2);
                        let in_h = px_bh.saturating_sub(b_th * 2);
                        out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], pill_bg));

                        // Left chevron
                        font.layout_text(
                            "<",
                            btn.x + 8.0,
                            btn.y + 4.5,
                            UiQuad::rgba(110, 130, 160, 255),
                            true,
                            scale,
                            out,
                        );

                        // Right chevron
                        font.layout_text(
                            ">",
                            btn.x + btn.width - 14.0,
                            btn.y + 4.5,
                            UiQuad::rgba(110, 130, 160, 255),
                            true,
                            scale,
                            out,
                        );

                        // Centered text
                        let val_str = btn
                            .label
                            .trim_start_matches('<')
                            .trim_end_matches('>')
                            .trim();
                        let (vw, _) = font.measure_text(val_str);
                        let val_col = if btn.label.contains("ON") {
                            UiQuad::rgba(94, 234, 212, 255)
                        } else if btn.label.contains("OFF") {
                            UiQuad::rgba(148, 163, 184, 255)
                        } else {
                            UiQuad::rgba(240, 245, 252, 255)
                        };

                        font.layout_text(
                            val_str,
                            btn.x + (btn.width - vw) * 0.5,
                            btn.y + 4.5,
                            val_col,
                            true,
                            scale,
                            out,
                        );
                    }
                }
                SettingRowItem::KeybindRow {
                    action,
                    button_idx,
                    label,
                } => {
                    if let Some((_, btn)) = self.keybind_buttons.get(*button_idx) {
                        let has_conflict = conflicts
                            .iter()
                            .any(|(a1, a2, _)| *a1 == *action || *a2 == *action);
                        let is_listening = self.listening_action == Some(*action);

                        render_capsule_row(
                            col_x,
                            row.y,
                            col_w,
                            row.height,
                            btn.hovered || is_listening,
                            scale,
                            out,
                        );

                        // Left action name
                        let lbl_col = if has_conflict {
                            UiQuad::rgba(251, 146, 60, 255)
                        } else {
                            UiQuad::rgba(230, 237, 245, 255)
                        };
                        font.layout_text(
                            label,
                            col_x + 12.0,
                            row.y + 7.5,
                            lbl_col,
                            true,
                            scale,
                            out,
                        );

                        // Right keybind pill
                        let px_bx = snap_to_physical(btn.x, scale);
                        let px_by = snap_to_physical(btn.y, scale);
                        let px_bw = snap_to_physical(btn.width, scale) as u16;
                        let px_bh = snap_to_physical(btn.height, scale) as u16;
                        let b_th = scale.max(1) as u16;

                        if is_listening {
                            // Warm gold glowing capture state
                            out.push(UiQuad::solid(
                                [px_bx, px_by],
                                [px_bw, px_bh],
                                UiQuad::rgba(234, 179, 8, 240),
                            ));
                            let (bw, _) = font.measure_text(&btn.label);
                            font.layout_text(
                                &btn.label,
                                btn.x + (btn.width - bw) * 0.5,
                                btn.y + 4.5,
                                UiQuad::rgba(15, 20, 25, 255),
                                false,
                                scale,
                                out,
                            );
                        } else {
                            let pill_border = if has_conflict {
                                UiQuad::rgba(251, 146, 60, 255)
                            } else if btn.hovered {
                                UiQuad::rgba(70, 98, 146, 255)
                            } else {
                                UiQuad::rgba(44, 60, 88, 200)
                            };
                            let pill_bg = if btn.hovered {
                                UiQuad::rgba(38, 54, 82, 255)
                            } else {
                                UiQuad::rgba(26, 36, 54, 235)
                            };

                            out.push(UiQuad::solid([px_bx, px_by], [px_bw, px_bh], pill_border));
                            let in_x = px_bx + i32::from(b_th);
                            let in_y = px_by + i32::from(b_th);
                            let in_w = px_bw.saturating_sub(b_th * 2);
                            let in_h = px_bh.saturating_sub(b_th * 2);
                            out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], pill_bg));

                            let (bw, _) = font.measure_text(&btn.label);
                            font.layout_text(
                                &btn.label,
                                btn.x + (btn.width - bw) * 0.5,
                                btn.y + 4.5,
                                UiQuad::rgba(226, 232, 240, 255),
                                true,
                                scale,
                                out,
                            );
                        }
                    }
                }
                SettingRowItem::LanguageRow {
                    button_idx,
                    code: _,
                    display,
                    is_active,
                } => {
                    if let Some(btn) = self.toggle_buttons.get(*button_idx) {
                        let px_rx = snap_to_physical(col_x, scale);
                        let px_ry = snap_to_physical(row.y, scale);
                        let px_rw = snap_to_physical(col_w, scale) as u16;
                        let px_rh = snap_to_physical(row.height, scale) as u16;
                        let b_th = scale.max(1) as u16;

                        let (border_col, bg_col) = if *is_active {
                            (
                                UiQuad::rgba(220, 160, 50, 240),
                                UiQuad::rgba(32, 38, 54, 240),
                            )
                        } else if btn.hovered {
                            (
                                UiQuad::rgba(54, 76, 110, 255),
                                UiQuad::rgba(26, 38, 58, 240),
                            )
                        } else {
                            (UiQuad::rgba(32, 44, 64, 180), UiQuad::rgba(18, 24, 36, 210))
                        };

                        out.push(UiQuad::solid([px_rx, px_ry], [px_rw, px_rh], border_col));
                        let in_x = px_rx + i32::from(b_th);
                        let in_y = px_ry + i32::from(b_th);
                        let in_w = px_rw.saturating_sub(b_th * 2);
                        let in_h = px_rh.saturating_sub(b_th * 2);
                        out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], bg_col));

                        let text_col = if *is_active {
                            UiQuad::rgba(255, 224, 102, 255)
                        } else if btn.hovered {
                            UiQuad::rgba(255, 255, 255, 255)
                        } else {
                            UiQuad::rgba(226, 232, 240, 255)
                        };
                        font.layout_text(
                            display,
                            col_x + 14.0,
                            row.y + 7.5,
                            text_col,
                            true,
                            scale,
                            out,
                        );

                        let badge_str = if *is_active {
                            "[ ACTIVE ]"
                        } else {
                            "[ Select ]"
                        };
                        let badge_col = if *is_active {
                            UiQuad::rgba(255, 209, 84, 255)
                        } else {
                            UiQuad::rgba(122, 139, 159, 255)
                        };
                        let (bw, _) = font.measure_text(badge_str);
                        font.layout_text(
                            badge_str,
                            col_x + col_w - bw - 14.0,
                            row.y + 7.5,
                            badge_col,
                            true,
                            scale,
                            out,
                        );
                    }
                }
                SettingRowItem::ActionRow { label } => {
                    let btn = &self.reset_keybinds_button;
                    let px_bx = snap_to_physical(btn.x, scale);
                    let px_by = snap_to_physical(btn.y, scale);
                    let px_bw = snap_to_physical(btn.width, scale) as u16;
                    let px_bh = snap_to_physical(btn.height, scale) as u16;
                    let b_th = scale.max(1) as u16;

                    let (border_col, bg_col) = if btn.hovered {
                        (
                            UiQuad::rgba(70, 98, 146, 255),
                            UiQuad::rgba(38, 54, 82, 255),
                        )
                    } else {
                        (UiQuad::rgba(44, 60, 88, 200), UiQuad::rgba(24, 34, 50, 220))
                    };

                    out.push(UiQuad::solid([px_bx, px_by], [px_bw, px_bh], border_col));
                    let in_x = px_bx + i32::from(b_th);
                    let in_y = px_by + i32::from(b_th);
                    let in_w = px_bw.saturating_sub(b_th * 2);
                    let in_h = px_bh.saturating_sub(b_th * 2);
                    out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], bg_col));

                    let (lw, _) = font.measure_text(label);
                    font.layout_text(
                        label,
                        btn.x + (btn.width - lw) * 0.5,
                        btn.y + 5.5,
                        UiQuad::rgba(226, 232, 240, 255),
                        true,
                        scale,
                        out,
                    );
                }
            }
        }

        // Slim amber vertical scrollbar
        if self.max_scroll_y > 0.0 {
            let track_x = col_x + col_w + 6.0;
            let track_y = content_top + 4.0;
            let track_h = content_bottom - track_y - 4.0;

            let px_tx = snap_to_physical(track_x, scale);
            let px_ty = snap_to_physical(track_y, scale);
            let px_tw = (scale.max(1) * 3) as u16;
            let px_th = snap_to_physical(track_h, scale) as u16;

            out.push(UiQuad::solid(
                [px_tx, px_ty],
                [px_tw, px_th],
                UiQuad::rgba(14, 18, 28, 200),
            ));

            let frac = (self.scroll_y / self.max_scroll_y).clamp(0.0, 1.0);
            let thumb_h = (track_h * (visible_ratio(self.max_scroll_y, track_h))).max(18.0);
            let thumb_y = track_y + frac * (track_h - thumb_h);

            let px_sy = snap_to_physical(thumb_y, scale);
            let px_sh = snap_to_physical(thumb_h, scale) as u16;

            out.push(UiQuad::solid(
                [px_tx, px_sy],
                [px_tw, px_sh],
                UiQuad::rgba(220, 160, 50, 240),
            ));
        }

        // Sticky Top Header (Occlusion Shield + Tabs + Title - Layered ON TOP of content rows)
        let px_top_h = snap_to_physical(content_top, scale) as u16;
        out.push(UiQuad::solid(
            [0, 0],
            [px_w, px_top_h],
            UiQuad::rgba(10, 13, 20, 255),
        ));

        // Grand centered golden title: "SETTINGS"
        let title_text = "SETTINGS";
        let (tw, _) = font.measure_text(title_text);
        font.layout_text(
            title_text,
            (width_gui - tw) * 0.5,
            12.0,
            UiQuad::rgba(255, 209, 84, 255),
            true,
            scale,
            out,
        );

        // Horizontal separator line under tabs
        let px_lx = snap_to_physical(col_x, scale);
        let px_ly = snap_to_physical(tab_line_y, scale);
        let px_lw = snap_to_physical(col_w, scale) as u16;
        out.push(UiQuad::solid(
            [px_lx, px_ly],
            [px_lw, scale.max(1) as u16],
            UiQuad::rgba(32, 42, 60, 220),
        ));

        // Tab buttons with active golden underline
        for (i, btn) in self.tab_buttons.iter().enumerate() {
            let is_active = matches!(
                (i, self.active_tab),
                (0, SettingsTab::Video)
                    | (1, SettingsTab::Audio)
                    | (2, SettingsTab::Controls)
                    | (3, SettingsTab::Language)
            );

            let (text_col, text_y_off) = if is_active {
                // Golden active tab text
                (UiQuad::rgba(255, 224, 102, 255), 0.0)
            } else if btn.hovered {
                (UiQuad::rgba(200, 218, 240, 255), -0.5)
            } else {
                (UiQuad::rgba(122, 139, 159, 255), 0.0)
            };

            let tab_label_upper = btn.label.to_uppercase();
            let (bw, bh) = font.measure_text(&tab_label_upper);
            font.layout_text(
                &tab_label_upper,
                btn.x + (btn.width - bw) * 0.5,
                btn.y + (btn.height - bh) * 0.5 + text_y_off,
                text_col,
                true,
                scale,
                out,
            );

            if is_active {
                // Golden underline indicator bar
                let px_bx = snap_to_physical(btn.x, scale);
                let px_by = snap_to_physical(tab_line_y - 1.0, scale);
                let px_bw = snap_to_physical(btn.width, scale) as u16;
                let px_bh = (scale.max(1) * 2) as u16;
                out.push(UiQuad::solid(
                    [px_bx, px_by],
                    [px_bw, px_bh],
                    UiQuad::rgba(220, 160, 50, 255),
                ));
            }
        }

        // Bottom backdrop bar behind Action buttons
        let bar_h = 32.0;
        let bar_y = height_gui - bar_h;
        let px_by = snap_to_physical(bar_y, scale);
        let px_bh = snap_to_physical(bar_h, scale) as u16;

        out.push(UiQuad::solid(
            [0, px_by],
            [px_w, px_bh],
            UiQuad::rgba(10, 13, 19, 245),
        ));
        out.push(UiQuad::solid(
            [0, px_by],
            [px_w, scale.max(1) as u16],
            UiQuad::rgba(28, 36, 50, 200),
        ));

        // Footer Back / ESC button
        {
            let btn = &self.back_button;
            let px_x = snap_to_physical(btn.x, scale);
            let px_y = snap_to_physical(btn.y, scale);
            let px_bw = snap_to_physical(btn.width, scale) as u16;
            let px_bh = snap_to_physical(btn.height, scale) as u16;
            let b_th = scale.max(1) as u16;

            let border_col = if btn.hovered {
                UiQuad::rgba(65, 85, 120, 255)
            } else {
                UiQuad::rgba(38, 50, 72, 200)
            };
            let bg_col = if btn.hovered {
                UiQuad::rgba(30, 40, 60, 240)
            } else {
                UiQuad::rgba(20, 26, 40, 220)
            };

            out.push(UiQuad::solid([px_x, px_y], [px_bw, px_bh], border_col));
            let in_x = px_x + i32::from(b_th);
            let in_y = px_y + i32::from(b_th);
            let in_w = px_bw.saturating_sub(b_th * 2);
            let in_h = px_bh.saturating_sub(b_th * 2);
            out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], bg_col));

            // ESC keycap box
            let kbox_w = 26.0f32;
            let kbox_h = 14.0f32;
            let kbox_x = btn.x + 3.0;
            let kbox_y = btn.y + 3.0;
            let px_kx = snap_to_physical(kbox_x, scale);
            let px_ky = snap_to_physical(kbox_y, scale);
            let px_kw = snap_to_physical(kbox_w, scale) as u16;
            let px_kh = snap_to_physical(kbox_h, scale) as u16;
            out.push(UiQuad::solid(
                [px_kx, px_ky],
                [px_kw, px_kh],
                UiQuad::rgba(45, 60, 85, 255),
            ));
            font.layout_text(
                "ESC",
                kbox_x + 4.0,
                kbox_y + 3.0,
                UiQuad::rgba(220, 230, 245, 255),
                false,
                scale,
                out,
            );

            font.layout_text(
                "Back",
                btn.x + 34.0,
                btn.y + 5.5,
                UiQuad::rgba(180, 196, 215, 255),
                true,
                scale,
                out,
            );
        }

        // Vibrant illuminated blue Done button
        {
            let btn = &self.done_button;
            let px_x = snap_to_physical(btn.x, scale);
            let px_y = snap_to_physical(btn.y, scale);
            let px_bw = snap_to_physical(btn.width, scale) as u16;
            let px_bh = snap_to_physical(btn.height, scale) as u16;
            let b_th = scale.max(1) as u16;

            let border_col = if btn.hovered {
                UiQuad::rgba(85, 145, 235, 255)
            } else {
                UiQuad::rgba(60, 115, 195, 255)
            };
            let bg_col = if btn.hovered {
                UiQuad::rgba(55, 115, 200, 255)
            } else {
                UiQuad::rgba(45, 95, 168, 255)
            };

            out.push(UiQuad::solid([px_x, px_y], [px_bw, px_bh], border_col));
            let in_x = px_x + i32::from(b_th);
            let in_y = px_y + i32::from(b_th);
            let in_w = px_bw.saturating_sub(b_th * 2);
            let in_h = px_bh.saturating_sub(b_th * 2);
            out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], bg_col));

            let done_upper = btn.label.to_uppercase();
            let (lw, _) = font.measure_text(&done_upper);
            font.layout_text(
                &done_upper,
                btn.x + (btn.width - lw) * 0.5,
                btn.y + 5.5,
                UiQuad::rgba(255, 255, 255, 255),
                true,
                scale,
                out,
            );
        }
    }
}

/// Helper function to compute visible scrollbar thumb ratio.
fn visible_ratio(max_scroll: f32, visible_h: f32) -> f32 {
    let total = max_scroll + visible_h;
    if total > 0.0 {
        (visible_h / total).clamp(0.15, 0.8)
    } else {
        1.0
    }
}

/// Helper function rendering a sleek capsule row container.
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)]
fn render_capsule_row(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hovered: bool,
    scale: u32,
    out: &mut Vec<UiQuad>,
) {
    let px_x = snap_to_physical(x, scale);
    let px_y = snap_to_physical(y, scale);
    let px_w = snap_to_physical(w, scale) as u16;
    let px_h = snap_to_physical(h, scale) as u16;
    let b_th = scale.max(1) as u16;

    let border_col = if hovered {
        UiQuad::rgba(54, 76, 110, 255)
    } else {
        UiQuad::rgba(32, 44, 64, 180)
    };
    let bg_col = if hovered {
        UiQuad::rgba(26, 38, 58, 240)
    } else {
        UiQuad::rgba(18, 24, 36, 210)
    };

    // Row border
    out.push(UiQuad::solid([px_x, px_y], [px_w, px_h], border_col));

    // Row interior fill
    let in_x = px_x + i32::from(b_th);
    let in_y = px_y + i32::from(b_th);
    let in_w = px_w.saturating_sub(b_th * 2);
    let in_h = px_h.saturating_sub(b_th * 2);
    out.push(UiQuad::solid([in_x, in_y], [in_w, in_h], bg_col));
}

/// Helper function rendering a warm golden diamond slider thumb.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::similar_names
)]
fn render_diamond_thumb(
    cx_gui: f32,
    cy_gui: f32,
    scale: u32,
    hovered: bool,
    out: &mut Vec<UiQuad>,
) {
    let px_cx = snap_to_physical(cx_gui, scale);
    let px_cy = snap_to_physical(cy_gui, scale);
    let r = (3 * scale.max(1)) as i32;

    let core_col = if hovered {
        UiQuad::rgba(255, 235, 120, 255)
    } else {
        UiQuad::rgba(255, 209, 84, 255)
    };
    let border_col = if hovered {
        UiQuad::rgba(200, 150, 30, 255)
    } else {
        UiQuad::rgba(170, 120, 20, 255)
    };

    // Outer diamond border
    for dy in -r..=r {
        let span = r - dy.abs();
        let w = (span * 2 + 1) as u16;
        out.push(UiQuad::solid(
            [px_cx - span, px_cy + dy],
            [w, 1],
            border_col,
        ));
    }

    // Inner diamond core
    let inner_r = (r - scale.max(1) as i32).max(0);
    for dy in -inner_r..=inner_r {
        let span = inner_r - dy.abs();
        let w = (span * 2 + 1) as u16;
        out.push(UiQuad::solid([px_cx - span, px_cy + dy], [w, 1], core_col));
    }
}

/// Helper function formatting a concise value readout for a slider.
fn format_slider_readout(s: &MenuSlider) -> String {
    if s.prefix == "FPS Limit" || s.prefix == "Framerate Limit" {
        let val = s.value.round() as i32;
        if val <= 0 {
            return "VSync".to_string();
        }
        if val >= s.max.round() as i32 {
            return "Unlimited".to_string();
        }
        return format!("{val} FPS");
    }
    if s.is_int {
        format!("{}{}", s.value as i32, s.suffix)
    } else if s.suffix == "%" {
        format!("{}%", (s.value * 100.0).round() as i32)
    } else {
        format!("{:.1}{}", s.value, s.suffix)
    }
}
