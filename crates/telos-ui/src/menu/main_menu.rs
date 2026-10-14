//! Title main menu screen with logo banner, animated splash text, and navigation buttons.

use crate::font::BitmapFont;
use crate::hud::UiLayers;
use crate::menu::widgets::MenuButton;
use crate::quad::UiQuad;
use crate::scale::snap_to_physical;
use telos_core::i18n::LanguageCatalog;

/// Action triggered by user clicking a button on the main menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    /// Open singleplayer world select screen.
    Singleplayer,
    /// Open multiplayer server list screen.
    Multiplayer,
    /// Open options / settings screen.
    Options,
    /// Quit application.
    Quit,
}

/// Title main menu screen state and button layout.
#[derive(Debug, Clone)]
pub struct MainMenuScreen {
    /// Action buttons.
    pub buttons: Vec<MenuButton>,
    /// Splash text string.
    pub splash_text: String,
}

impl Default for MainMenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl MainMenuScreen {
    /// Creates a new `MainMenuScreen` with standard navigation buttons.
    #[must_use]
    pub fn new() -> Self {
        let splashes = [
            "Over-engineered in Rust!",
            "144 FPS pure Vulkan 1.3!",
            "Extreme render distance!",
            "Deterministic cubic chunks!",
            "Infinite possibilities!",
            "GPU-driven Hi-Z occlusion!",
        ];
        let splash_text = splashes[0].to_string();

        Self {
            buttons: Vec::new(),
            splash_text,
        }
    }

    /// Updates button layouts and localized labels based on current viewport size and language catalog.
    pub fn update_layout_i18n(
        &mut self,
        width_gui: f32,
        height_gui: f32,
        catalog: &LanguageCatalog,
    ) {
        let center_x = width_gui * 0.5;
        let btn_w = 200.0;
        let btn_h = 24.0;
        let btn_x = center_x - btn_w * 0.5;
        let logo_h = 260.0f32.min(width_gui - 32.0) * (64.0 / 256.0);
        let start_y = (height_gui * 0.44).max(12.0 + logo_h + 30.0);

        self.buttons = vec![
            MenuButton::new(
                1,
                btn_x,
                start_y,
                btn_w,
                btn_h,
                catalog.translate("menu.singleplayer"),
            ),
            MenuButton::new(
                2,
                btn_x,
                start_y + 30.0,
                btn_w,
                btn_h,
                catalog.translate("menu.multiplayer"),
            ),
            MenuButton::new(
                3,
                btn_x,
                start_y + 60.0,
                btn_w,
                btn_h,
                catalog.translate("menu.options"),
            ),
            MenuButton::new(
                4,
                btn_x,
                start_y + 90.0,
                btn_w,
                btn_h,
                catalog.translate("menu.quit"),
            ),
        ];
    }

    /// Updates button layouts based on current viewport size using default embedded translations.
    pub fn update_layout(&mut self, width_gui: f32, height_gui: f32) {
        let catalog = LanguageCatalog::with_default_embedded();
        self.update_layout_i18n(width_gui, height_gui, &catalog);
    }

    /// Handles mouse motion in GUI units and updates hover states.
    pub fn handle_mouse_move(&mut self, mouse_x: f32, mouse_y: f32) {
        for btn in &mut self.buttons {
            btn.hovered = btn.contains(mouse_x, mouse_y);
        }
    }

    /// Handles mouse click and returns the triggered action, if any.
    #[must_use]
    pub fn handle_mouse_click(&mut self, mouse_x: f32, mouse_y: f32) -> Option<MainMenuAction> {
        for btn in &self.buttons {
            if btn.enabled && btn.contains(mouse_x, mouse_y) {
                return match btn.id {
                    1 => Some(MainMenuAction::Singleplayer),
                    2 => Some(MainMenuAction::Multiplayer),
                    3 => Some(MainMenuAction::Options),
                    4 => Some(MainMenuAction::Quit),
                    _ => None,
                };
            }
        }
        None
    }

    /// Renders the title banner, animated splash, buttons, and version footer using default layers.
    pub fn render(
        &self,
        font: &BitmapFont,
        width_gui: f32,
        height_gui: f32,
        scale: u32,
        tick: u64,
        out: &mut Vec<UiQuad>,
    ) {
        self.render_with_layers(
            &UiLayers::default(),
            font,
            width_gui,
            height_gui,
            scale,
            tick,
            out,
        );
    }

    /// Renders the title banner, animated splash, buttons, and version footer to `out`.
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn render_with_layers(
        &self,
        layers: &UiLayers,
        font: &BitmapFont,
        width_gui: f32,
        height_gui: f32,
        scale: u32,
        tick: u64,
        out: &mut Vec<UiQuad>,
    ) {
        // Translucent dark vignette overlay so 3D world panorama is visible
        let px_w = (width_gui * scale as f32).round() as u16;
        let px_h = (height_gui * scale as f32).round() as u16;
        out.push(UiQuad::solid(
            [0, 0],
            [px_w, px_h],
            UiQuad::rgba(0, 0, 0, 75),
        ));

        // Long stylized logo banner "TELOS" positioned at the very top
        let logo_w = 260.0f32.min(width_gui - 32.0);
        let logo_h = logo_w * (64.0 / 256.0);
        let logo_x = (width_gui - logo_w) * 0.5;
        let logo_y = 12.0f32.min(height_gui * 0.05);

        let phys_lx = snap_to_physical(logo_x, scale);
        let phys_ly = snap_to_physical(logo_y, scale);
        let phys_lw = (logo_w * scale as f32).round() as u16;
        let phys_lh = (logo_h * scale as f32).round() as u16;

        out.push(UiQuad::sprite(
            [phys_lx, phys_ly],
            [phys_lw, phys_lh],
            [layers.logo_uv[0], layers.logo_uv[1]],
            [layers.logo_uv[2], layers.logo_uv[3]],
            layers.logo,
            UiQuad::rgba(255, 255, 255, 255),
        ));

        // Subtitle centered neatly below the logo
        let subtitle = "§7Rust & Vulkan 1.3 Voxel Engine";
        let (sw, _) = font.measure_text(subtitle);
        font.layout_text(
            subtitle,
            (width_gui - sw) * 0.5,
            logo_y + logo_h + 3.0,
            UiQuad::rgba(160, 160, 170, 255),
            true,
            scale,
            out,
        );

        // Animated pulsing splash text tilted at -20 degrees at bottom right of title banner
        // Gentle 1.0 second per pulse heartbeat timing (independent of framerate)
        let time_sec = if tick > 1000 {
            (tick as f32) * 0.001
        } else {
            (tick as f32) / 20.0
        };
        let pulse = 1.0 + ((time_sec * std::f32::consts::PI).sin().abs() * 0.08);
        let splash = self.splash_text.as_str();
        let (splash_w, splash_h) = font.measure_text(splash);
        let pivot_x = logo_x + logo_w - 8.0;
        let pivot_y = logo_y + logo_h - 2.0;
        let angle_rad = -20.0f32.to_radians();

        let start_x = pivot_x - (splash_w * pulse) * 0.5;
        let start_y = pivot_y - (splash_h * pulse) * 0.5;

        font.layout_text_rotated(
            splash,
            start_x,
            start_y,
            pivot_x,
            pivot_y,
            angle_rad,
            pulse,
            UiQuad::rgba(255, 255, 0, 255), // Pure bright yellow
            true,                            // with drop shadow
            scale,
            out,
        );

        // Render buttons with texture and hover animation
        for btn in &self.buttons {
            btn.render_animated(layers, font, scale, tick, out);
        }

        // Version & platform footer
        let footer = "§8Telos v0.1.0-dev | Linux x86_64";
        font.layout_text(
            footer,
            6.0,
            height_gui - 14.0,
            UiQuad::rgba(120, 120, 130, 255),
            false,
            scale,
            out,
        );
    }
}
