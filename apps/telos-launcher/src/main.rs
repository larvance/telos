//! Telos Game Launcher & Version Manager.
//!
//! A rich desktop launcher providing authentication, repository release/snapshot fetching,
//! local development build coordination, installations management, and patch notes.

mod build_coordinator;
mod profile;
mod version_manager;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use build_coordinator::{resolve_repo_dir, start_build_worker, BuildEvent};
use profile::{load_profile, save_profile, LauncherProfile};
use version_manager::{
    fetch_remote_versions, get_default_installations, ChannelKind, Installation,
    RemoteVersionInfo,
};

static LOGO_BYTES: &[u8] = include_bytes!("../../../assets/telos/textures/gui/title/telos.png");

/// Active tab in the main launcher view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LauncherTab {
    Play,
    Installations,
    Servers,
    PatchNotes,
    Settings,
}

/// Navigation sidebar section.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SidebarSection {
    Home,
    TelosVoxel,
    TelosServer,
    WhatsNew,
    Settings,
}

/// Local dev build execution state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BuildState {
    Idle,
    Building,
    WaitingForLock,
    Success,
    Failed,
}

struct TelosLauncherApp {
    profile: LauncherProfile,
    repo_dir: PathBuf,
    #[allow(dead_code)]
    is_local_dev: bool,
    remote_versions: RemoteVersionInfo,
    installations: Vec<Installation>,
    active_tab: LauncherTab,
    sidebar_section: SidebarSection,
    selected_installation_id: String,

    // Login state
    #[allow(dead_code)]
    login_username_input: String,

    // UI state
    version_dropdown_open: bool,
    logo_texture: Option<egui::TextureHandle>,
    #[allow(dead_code)]
    account_menu_open: bool,

    // Dev Build / Execution state
    build_state: BuildState,
    build_rx: Option<Receiver<BuildEvent>>,
    build_status_text: String,
    build_completed_crates: usize,
    build_total_crates: usize,
    build_error_output: String,
    build_success_time: Option<Instant>,
    should_launch_game: Arc<AtomicBool>,
    is_maximized: bool,
}

impl TelosLauncherApp {
    fn new(repo_dir: PathBuf, should_launch_game: Arc<AtomicBool>) -> Self {
        let profile = load_profile();
        let is_local_dev = version_manager::detect_local_dev_repo(&repo_dir);
        let remote_versions = fetch_remote_versions(&repo_dir);
        let installations = get_default_installations(&repo_dir, &remote_versions);

        let selected_id = if profile.selected_installation_id.is_empty() {
            if is_local_dev {
                "local-dev".to_string()
            } else {
                "latest-release".to_string()
            }
        } else {
            profile.selected_installation_id.clone()
        };

        let initial_user = if profile.username.is_empty() {
            "Larvance".to_string()
        } else {
            profile.username.clone()
        };

        Self {
            profile,
            repo_dir,
            is_local_dev,
            remote_versions,
            installations,
            active_tab: LauncherTab::Play,
            sidebar_section: SidebarSection::TelosVoxel,
            selected_installation_id: selected_id,
            login_username_input: initial_user,
            version_dropdown_open: false,
            logo_texture: None,
            account_menu_open: false,
            build_state: BuildState::Idle,
            build_rx: None,
            build_status_text: String::new(),
            build_completed_crates: 0,
            build_total_crates: 28,
            build_error_output: String::new(),
            build_success_time: None,
            should_launch_game,
            is_maximized: false,
        }
    }

    fn ensure_logo_texture(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        if self.logo_texture.is_none() {
            if let Ok(img) = image::load_from_memory(LOGO_BYTES) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_img = egui::ColorImage::from_rgba_unmultiplied(
                    size,
                    rgba.as_flat_samples().as_slice(),
                );
                self.logo_texture = Some(ctx.load_texture(
                    "telos_logo",
                    color_img,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        self.logo_texture.as_ref()
    }

    fn get_selected_installation(&self) -> Option<&Installation> {
        self.installations
            .iter()
            .find(|i| i.id == self.selected_installation_id)
            .or_else(|| self.installations.first())
    }

    fn trigger_launch(&mut self) {
        let is_dev_selected = self
            .get_selected_installation()
            .map_or(false, |i| i.channel == ChannelKind::LocalDev);

        if is_dev_selected {
            // Start local dev build coordinator
            let (tx, rx) = channel::<BuildEvent>();
            self.build_rx = Some(rx);
            self.build_state = BuildState::Building;
            self.build_status_text = "Acquiring build lock...".to_string();
            self.build_completed_crates = 0;
            self.build_total_crates = 28;
            self.build_error_output.clear();

            let is_debug = std::env::var("TELOS_DEBUG").map_or(false, |v| v == "1");
            start_build_worker(self.repo_dir.clone(), is_debug, tx, SystemTime::now());
        } else {
            // Launch release binary
            let target_bin = self.repo_dir.join("target/release/telos");
            if target_bin.exists() {
                let _ = Command::new(&target_bin).spawn();
                if self.profile.auto_close_on_play {
                    self.should_launch_game.store(true, Ordering::SeqCst);
                }
            } else {
                // If not yet built locally, build release
                let (tx, rx) = channel::<BuildEvent>();
                self.build_rx = Some(rx);
                self.build_state = BuildState::Building;
                self.build_status_text = "Building release client...".to_string();
                start_build_worker(self.repo_dir.clone(), false, tx, SystemTime::now());
            }
        }
    }

    fn poll_build_events(&mut self, ctx: &egui::Context) {
        if let Some(ref rx) = self.build_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    BuildEvent::Status(msg) => {
                        self.build_status_text = msg;
                    }
                    BuildEvent::CrateStarted(name) => {
                        self.build_state = BuildState::Building;
                        self.build_status_text = format!("Compiling {name}...");
                    }
                    BuildEvent::ArtifactFinished {
                        crate_name,
                        is_target_bin,
                    } => {
                        self.build_completed_crates += 1;
                        if self.build_completed_crates > self.build_total_crates {
                            self.build_total_crates = self.build_completed_crates + 1;
                        }
                        if is_target_bin {
                            self.build_status_text = "Linking final executable...".to_string();
                        } else {
                            self.build_status_text = format!(
                                "Built {crate_name} ({}/{})",
                                self.build_completed_crates, self.build_total_crates
                            );
                        }
                    }
                    BuildEvent::LockHeld { message } => {
                        self.build_state = BuildState::WaitingForLock;
                        self.build_status_text = message;
                    }
                    BuildEvent::Success { already_built } => {
                        self.build_state = BuildState::Success;
                        self.build_success_time = Some(Instant::now());
                        if already_built {
                            self.build_status_text = "Skipped duplicate build! Launching...".into();
                        } else {
                            self.build_status_text = "Build successful! Launching Telos...".into();
                        }

                        // Execute game client
                        let is_debug = std::env::var("TELOS_DEBUG").map_or(false, |v| v == "1");
                        let bin = if is_debug {
                            self.repo_dir.join("target/debug/telos")
                        } else {
                            self.repo_dir.join("target/release/telos")
                        };
                        if bin.exists() {
                            let _ = Command::new(&bin).spawn();
                        }

                        if self.profile.auto_close_on_play {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    BuildEvent::Failed(err) => {
                        self.build_state = BuildState::Failed;
                        self.build_status_text = "Build Failed".to_string();
                        self.build_error_output = err;
                    }
                }
            }
        }

        if let Some(t) = self.build_success_time {
            if t.elapsed() >= Duration::from_secs(3) {
                self.build_state = BuildState::Idle;
                self.build_success_time = None;
                self.build_rx = None;
            }
        }
    }
}

impl eframe::App for TelosLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_build_events(ctx);

        if self.build_state != BuildState::Idle {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // Global theme styling
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(20, 22, 26);
        visuals.window_fill = egui::Color32::from_rgb(26, 29, 35);
        ctx.set_visuals(visuals);

        // Always render top authentic OS titlebar across the full window
        egui::TopBottomPanel::top("app_titlebar")
            .exact_height(22.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(34, 34, 34))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                render_titlebar(ui, "Telos Launcher", &mut self.is_maximized);
            });

        if !self.profile.logged_in {
            self.render_login_screen(ctx);
        } else {
            self.render_main_launcher(ctx);
        }
    }
}

/// Renders a pixel-accurate miniature United States flag (16x11 pixels).
fn draw_us_flag(painter: &egui::Painter, rect: egui::Rect) {
    let w = rect.width();
    let h = rect.height();

    // Base background: crisp white
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);

    // 7 Red stripes (indices 0, 2, 4, 6, 8, 10, 12 out of 13)
    let stripe_h = h / 13.0;
    let red = egui::Color32::from_rgb(178, 34, 52);
    for i in (0..13).step_by(2) {
        let y_top = rect.min.y + i as f32 * stripe_h;
        let y_bot = rect.min.y + (i + 1) as f32 * stripe_h;
        let stripe_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, y_top),
            egui::pos2(rect.max.x, y_bot.min(rect.max.y)),
        );
        painter.rect_filled(stripe_rect, 0.0, red);
    }

    // Blue Canton: 7 stripes tall (~5.92px), 42% width (~7.0px)
    let canton_w = (w * 0.42).round();
    let canton_h = stripe_h * 7.0;
    let canton_rect = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.min.x + canton_w, rect.min.y + canton_h),
    );
    painter.rect_filled(canton_rect, 0.0, egui::Color32::from_rgb(26, 38, 90));

    // Crisp star dots (6 subtle white points in 2 rows of 3)
    let star_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210);
    let dot = |x: f32, y: f32, p: &egui::Painter| {
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(rect.min.x + x, rect.min.y + y), egui::vec2(1.0, 1.0)),
            0.0,
            star_color,
        );
    };

    dot(1.4, 1.4, painter);
    dot(3.5, 1.4, painter);
    dot(5.4, 1.4, painter);

    dot(1.4, 3.8, painter);
    dot(3.5, 3.8, painter);
    dot(5.4, 3.8, painter);

    // Subtle dark border outline
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(0.5_f32, egui::Color32::from_black_alpha(80)),
        egui::StrokeKind::Inside,
    );
}

/// Renders the authentic desktop OS titlebar (matching Image 1 & 2).
fn render_titlebar(ui: &mut egui::Ui, title: &str, is_maximized: &mut bool) {
    let titlebar_rect = ui.max_rect();
    let painter = ui.painter();

    // Dark grey titlebar background (#222222 / #242424) with rounded top corners
    painter.rect_filled(
        titlebar_rect,
        egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 },
        egui::Color32::from_rgb(34, 34, 34),
    );

    // Centered title text
    painter.text(
        titlebar_rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(11.5),
        egui::Color32::from_rgb(220, 220, 220),
    );

    // Draggable area across titlebar (excluding right buttons)
    let drag_rect = egui::Rect::from_min_max(
        titlebar_rect.min,
        egui::pos2(titlebar_rect.max.x - 70.0, titlebar_rect.max.y),
    );
    let drag_resp = ui.interact(drag_rect, ui.id().with("window_drag"), egui::Sense::click_and_drag());
    if drag_resp.dragged() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag_resp.double_clicked() {
        *is_maximized = !*is_maximized;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(*is_maximized));
    }

    // Ubuntu circular window controls on right (Min, Max, Close)
    let cy = titlebar_rect.center().y;
    let btn_radius = 6.5;

    // Minimize button
    let min_center = egui::pos2(titlebar_rect.max.x - 46.0, cy);
    let min_rect = egui::Rect::from_center_size(min_center, egui::vec2(15.0, 15.0));
    let min_resp = ui.interact(min_rect, ui.id().with("btn_min"), egui::Sense::click());
    let min_hover = min_resp.hovered();
    painter.circle_filled(
        min_center,
        btn_radius,
        if min_hover {
            egui::Color32::from_rgb(72, 72, 72)
        } else {
            egui::Color32::from_rgb(52, 52, 52)
        },
    );
    painter.line_segment(
        [egui::pos2(min_center.x - 3.0, min_center.y), egui::pos2(min_center.x + 3.0, min_center.y)],
        egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
    );
    if min_resp.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    // Maximize button
    let max_center = egui::pos2(titlebar_rect.max.x - 28.0, cy);
    let max_rect = egui::Rect::from_center_size(max_center, egui::vec2(15.0, 15.0));
    let max_resp = ui.interact(max_rect, ui.id().with("btn_max"), egui::Sense::click());
    let max_hover = max_resp.hovered();
    painter.circle_filled(
        max_center,
        btn_radius,
        if max_hover {
            egui::Color32::from_rgb(72, 72, 72)
        } else {
            egui::Color32::from_rgb(52, 52, 52)
        },
    );
    let max_icon_rect = egui::Rect::from_center_size(max_center, egui::vec2(5.5, 5.5));
    painter.rect_stroke(
        max_icon_rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    if max_resp.clicked() {
        *is_maximized = !*is_maximized;
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(*is_maximized));
    }

    // Close button
    let close_center = egui::pos2(titlebar_rect.max.x - 10.0, cy);
    let close_rect = egui::Rect::from_center_size(close_center, egui::vec2(15.0, 15.0));
    let close_resp = ui.interact(close_rect, ui.id().with("btn_close"), egui::Sense::click());
    let close_hover = close_resp.hovered();
    painter.circle_filled(
        close_center,
        btn_radius,
        if close_hover {
            egui::Color32::from_rgb(232, 17, 35)
        } else {
            egui::Color32::from_rgb(52, 52, 52)
        },
    );
    let d = 2.4;
    painter.line_segment(
        [egui::pos2(close_center.x - d, close_center.y - d), egui::pos2(close_center.x + d, close_center.y + d)],
        egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
    );
    painter.line_segment(
        [egui::pos2(close_center.x - d, close_center.y + d), egui::pos2(close_center.x + d, close_center.y - d)],
        egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
    );
    if close_resp.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl TelosLauncherApp {
    /// Renders the authentic Login Request Screen (matching Image 2).
    fn render_login_screen(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(0, 0, 0))
            .inner_margin(egui::Margin::same(0));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            let avail = ui.available_size();
            let painter = ui.painter();

            // 1. Language Dropdown (Exact match to Image 2: x=5, y=28, w=162, h=26)
            let lang_pos = egui::pos2(5.0, 28.0);
            let lang_w = 162.0;
            let lang_h = 26.0;
            let lang_rect = egui::Rect::from_min_size(lang_pos, egui::vec2(lang_w, lang_h));
            let lang_hovered = ui.rect_contains_pointer(lang_rect);

            // Background & 1.0px Green border (#332f2e fill, #008018 border, sharp 0.0 rounding)
            painter.rect_filled(
                lang_rect,
                0.0,
                if lang_hovered {
                    egui::Color32::from_rgb(60, 56, 55)
                } else {
                    egui::Color32::from_rgb(51, 47, 46)
                },
            );
            painter.rect_stroke(
                lang_rect,
                0.0,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0, 128, 24)),
                egui::StrokeKind::Inside,
            );

            // Draw US Flag on left of dropdown (16x11, centered vertically in 26px)
            let flag_rect = egui::Rect::from_min_size(
                egui::pos2(lang_pos.x + 8.0, lang_pos.y + 7.5),
                egui::vec2(16.0, 11.0),
            );
            draw_us_flag(painter, flag_rect);

            // Dropdown text: "English - United States"
            painter.text(
                egui::pos2(lang_pos.x + 30.0, lang_pos.y + 13.0),
                egui::Align2::LEFT_CENTER,
                "English - United States",
                egui::FontId::proportional(10.5),
                egui::Color32::from_rgb(220, 220, 220),
            );

            // Vector Caret Arrow on far right (no unrenderable Unicode box!)
            let arrow_cx = lang_pos.x + lang_w - 11.0;
            let arrow_cy = lang_pos.y + 13.0;
            let arrow_color = if lang_hovered {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(160, 160, 160)
            };
            painter.line_segment(
                [egui::pos2(arrow_cx - 3.5, arrow_cy - 2.0), egui::pos2(arrow_cx, arrow_cy + 2.0)],
                egui::Stroke::new(1.2_f32, arrow_color),
            );
            painter.line_segment(
                [egui::pos2(arrow_cx, arrow_cy + 2.0), egui::pos2(arrow_cx + 3.5, arrow_cy - 2.0)],
                egui::Stroke::new(1.2_f32, arrow_color),
            );

            let lang_resp = ui.interact(lang_rect, ui.id().with("lang_btn"), egui::Sense::click());
            if lang_resp.clicked() {
                // Dropdown click
            }

            // 2. Central Login Card (Exact match to Image 2: width 321, height 174, y=184)
            let card_w = 321.0;
            let card_h = 174.0;
            let center_x = ((avail.x - card_w) / 2.0).round();
            let card_y = 184.0;
            let card_rect = egui::Rect::from_min_size(egui::pos2(center_x, card_y), egui::vec2(card_w, card_h));

            // Card background (#3d3938 fill)
            let gold = egui::Color32::from_rgb(226, 176, 36);
            painter.rect_filled(card_rect, 0.0, egui::Color32::from_rgb(61, 57, 56));

            let stroke_gold = egui::Stroke::new(2.0_f32, gold);

            // Left, Right, Bottom border segments
            painter.line_segment([card_rect.left_top(), card_rect.left_bottom()], stroke_gold);
            painter.line_segment([card_rect.right_top(), card_rect.right_bottom()], stroke_gold);
            painter.line_segment([card_rect.left_bottom(), card_rect.right_bottom()], stroke_gold);

            // Top border segments (interrupted by the TELOS logo in the middle, exactly matching Image 2)
            let logo_w = 236.0;
            let logo_h = 59.0;
            let logo_x = center_x + (card_w - logo_w) / 2.0;
            let logo_y = card_rect.min.y - 33.0;
            let logo_rect = egui::Rect::from_min_size(egui::pos2(logo_x, logo_y), egui::vec2(logo_w, logo_h));

            let cutout_pad = 8.0;
            painter.line_segment(
                [card_rect.left_top(), egui::pos2(logo_rect.min.x + cutout_pad, card_rect.min.y)],
                stroke_gold,
            );
            painter.line_segment(
                [egui::pos2(logo_rect.max.x - cutout_pad, card_rect.min.y), card_rect.right_top()],
                stroke_gold,
            );

            // Corner tabs (2x2 pixel blocks at the 4 outer corners matching Image 2)
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(card_rect.min.x - 2.0, card_rect.min.y - 2.0), egui::vec2(3.0, 3.0)), 0.0, gold);
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(card_rect.max.x - 1.0, card_rect.min.y - 2.0), egui::vec2(3.0, 3.0)), 0.0, gold);
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(card_rect.min.x - 2.0, card_rect.max.y - 1.0), egui::vec2(3.0, 3.0)), 0.0, gold);
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(card_rect.max.x - 1.0, card_rect.max.y - 1.0), egui::vec2(3.0, 3.0)), 0.0, gold);

            // Bottom center gold highlight
            painter.line_segment(
                [egui::pos2(card_rect.center().x - 30.0, card_rect.max.y - 1.0), egui::pos2(card_rect.center().x + 30.0, card_rect.max.y - 1.0)],
                egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(250, 204, 21)),
            );

            // 3. TELOS Logo Banner - overlaps the top edge of the card (Image 2)
            if let Some(tex) = self.ensure_logo_texture(ctx) {
                painter.image(
                    tex.id(),
                    logo_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                painter.text(
                    logo_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "TELOS",
                    egui::FontId::proportional(28.0),
                    gold,
                );
            }

            // 4. Green Sign In Button (Exact match: 180 x 26, top = card_rect.min.y + 69.0)
            let btn_w = 180.0;
            let btn_h = 26.0;
            let btn_x = center_x + (card_w - btn_w) / 2.0;
            let btn_y = card_rect.min.y + 69.0;
            let btn_rect = egui::Rect::from_min_size(egui::pos2(btn_x, btn_y), egui::vec2(btn_w, btn_h));
            let btn_hovered = ui.rect_contains_pointer(btn_rect);

            // Dark outer border & 3D bevel fill
            painter.rect_filled(
                btn_rect,
                2.0,
                if btn_hovered {
                    egui::Color32::from_rgb(24, 140, 32)
                } else {
                    egui::Color32::from_rgb(16, 124, 20)
                },
            );
            // 1px outer stroke
            painter.rect_stroke(
                btn_rect,
                2.0,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(16, 16, 16)),
                egui::StrokeKind::Outside,
            );
            // Highlight bevel line at top/left
            painter.line_segment(
                [btn_rect.left_top(), btn_rect.right_top()],
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 165, 48)),
            );
            // Shadow bevel line at bottom
            painter.line_segment(
                [btn_rect.left_bottom(), btn_rect.right_bottom()],
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(8, 75, 12)),
            );

            // Text with subtle drop shadow
            painter.text(
                btn_rect.center() + egui::vec2(0.8, 0.8),
                egui::Align2::CENTER_CENTER,
                "SIGN IN WITH TELOS",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(12, 48, 14),
            );
            painter.text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                "SIGN IN WITH TELOS",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(230, 230, 230),
            );

            let btn_resp = ui.interact(btn_rect, ui.id().with("sign_in_btn"), egui::Sense::click());
            if btn_resp.clicked() {
                self.profile.logged_in = true;
                let _ = save_profile(&self.profile);
            }

            // 5. "Sign Up ↗" link
            let link_pos = egui::pos2(center_x + card_w / 2.0, card_rect.min.y + 115.0);
            let link_rect = egui::Rect::from_center_size(link_pos, egui::vec2(70.0, 16.0));
            let link_hovered = ui.rect_contains_pointer(link_rect);

            painter.text(
                link_pos,
                egui::Align2::CENTER_CENTER,
                "Sign Up ↗",
                egui::FontId::proportional(10.5),
                if link_hovered {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(220, 220, 220)
                },
            );

            let link_resp = ui.interact(link_rect, ui.id().with("signup_link"), egui::Sense::click());
            if link_resp.clicked() {
                // Simulated link or open browser
            }

            // Optional secondary offline link
            let offline_pos = egui::pos2(center_x + card_w / 2.0, card_rect.min.y + 138.0);
            let offline_rect = egui::Rect::from_center_size(offline_pos, egui::vec2(90.0, 14.0));
            let off_hovered = ui.rect_contains_pointer(offline_rect);
            painter.text(
                offline_pos,
                egui::Align2::CENTER_CENTER,
                "Play Offline ↗",
                egui::FontId::proportional(9.5),
                if off_hovered {
                    egui::Color32::from_rgb(200, 200, 200)
                } else {
                    egui::Color32::from_rgb(130, 130, 130)
                },
            );
            let off_resp = ui.interact(offline_rect, ui.id().with("offline_link"), egui::Sense::click());
            if off_resp.clicked() {
                self.profile.logged_in = true;
                let _ = save_profile(&self.profile);
            }

            // 6. Bottom Edge: Subtle dark teal vignette & Version label
            let win_h = ui.max_rect().height();
            let vignette_rect = egui::Rect::from_min_size(
                egui::pos2(0.0, win_h - 8.0),
                egui::vec2(ui.available_width(), 8.0),
            );
            painter.rect_filled(vignette_rect, 0.0, egui::Color32::from_rgb(8, 22, 18));

            painter.text(
                egui::pos2(10.0, win_h - 13.0),
                egui::Align2::LEFT_CENTER,
                "v.3.41.18-2.6.2",
                egui::FontId::monospace(9.5),
                egui::Color32::from_rgb(85, 90, 95),
            );
        });
    }

    /// Renders the full Main Launcher Interface (matching Images 3 & 4).
    fn render_main_launcher(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("launcher_sidebar")
            .exact_width(200.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(24, 26, 30))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 42, 48)))
                    .inner_margin(egui::Margin::symmetric(10, 12)),
            )
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 20, 24))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                self.render_content_area(ctx, ui);
            });
    }

    /// Left navigation sidebar.
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        // --- User Profile Header ---
        let profile_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(32, 35, 42))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(48, 52, 60)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(8, 6));

        profile_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Circular Avatar with user initial
                let avatar_rect = ui.allocate_space(egui::vec2(28.0, 28.0)).1;
                ui.painter().circle_filled(
                    avatar_rect.center(),
                    14.0,
                    egui::Color32::from_rgb(234, 88, 12),
                );
                let initial = self
                    .profile
                    .username
                    .chars()
                    .next()
                    .unwrap_or('P')
                    .to_uppercase()
                    .to_string();
                ui.painter().text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initial,
                    egui::FontId::proportional(14.0),
                    egui::Color32::WHITE,
                );

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&self.profile.username)
                            .size(12.5)
                            .strong()
                            .color(egui::Color32::from_rgb(240, 243, 246)),
                    );
                    ui.label(
                        egui::RichText::new(&self.profile.account_type)
                            .size(10.0)
                            .color(egui::Color32::from_rgb(156, 163, 175)),
                    );
                });
            });
        });

        ui.add_space(14.0);

        // --- Navigation Section ---
        let is_home = self.sidebar_section == SidebarSection::Home;
        if ui
            .selectable_label(
                is_home,
                egui::RichText::new("⌂  HOME").size(12.5).strong(),
            )
            .clicked()
        {
            self.sidebar_section = SidebarSection::Home;
            self.active_tab = LauncherTab::Play;
        }

        ui.add_space(4.0);

        let is_voxel = self.sidebar_section == SidebarSection::TelosVoxel;
        let voxel_btn = ui.add(
            egui::Button::new(
                egui::RichText::new("▦  TELOS")
                    .size(13.0)
                    .strong()
                    .color(if is_voxel {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(180, 185, 195)
                    }),
            )
            .fill(if is_voxel {
                egui::Color32::from_rgb(38, 44, 52)
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(if is_voxel {
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(16, 185, 129))
            } else {
                egui::Stroke::NONE
            })
            .corner_radius(5.0)
            .min_size(egui::vec2(ui.available_width(), 32.0)),
        );
        if voxel_btn.clicked() {
            self.sidebar_section = SidebarSection::TelosVoxel;
        }

        ui.add_space(4.0);

        let is_server = self.sidebar_section == SidebarSection::TelosServer;
        if ui
            .selectable_label(
                is_server,
                egui::RichText::new("⚙  TELOS SERVER").size(12.0),
            )
            .clicked()
        {
            self.sidebar_section = SidebarSection::TelosServer;
        }

        // --- Footer Navigation ---
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("v1.0.0-release")
                    .monospace()
                    .size(10.5)
                    .color(egui::Color32::from_rgb(107, 114, 128)),
            );

            ui.add_space(8.0);
            if ui
                .selectable_label(
                    self.active_tab == LauncherTab::Settings,
                    egui::RichText::new("⚙  SETTINGS").size(12.0),
                )
                .clicked()
            {
                self.active_tab = LauncherTab::Settings;
            }

            ui.add_space(4.0);
            if ui
                .selectable_label(
                    self.active_tab == LauncherTab::PatchNotes,
                    egui::RichText::new("📰  WHAT'S NEW").size(12.0),
                )
                .clicked()
            {
                self.active_tab = LauncherTab::PatchNotes;
            }
        });
    }

    /// Main content area: header tabs and active view.
    fn render_content_area(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // --- Top Bar: Game Title & Tabs ---
        let header_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(26, 29, 35))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 42, 48)))
            .inner_margin(egui::Margin::symmetric(16, 8));

        header_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("TELOS")
                        .size(13.0)
                        .strong()
                        .monospace()
                        .color(egui::Color32::from_rgb(240, 243, 246)),
                );

                ui.add_space(20.0);

                let tabs = [
                    (LauncherTab::Play, "Play"),
                    (LauncherTab::Installations, "Installations"),
                    (LauncherTab::Servers, "Servers"),
                    (LauncherTab::PatchNotes, "Patch Notes"),
                ];

                for (tab, label) in tabs {
                    let active = self.active_tab == tab;
                    let text = egui::RichText::new(label).size(13.0).color(if active {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(156, 163, 175)
                    });

                    if ui.selectable_label(active, text).clicked() {
                        self.active_tab = tab;
                    }
                }

                // Notification pill on far right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("🔔")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(156, 163, 175)),
                    );
                });
            });
        });

        // --- Render Tab Body ---
        match self.active_tab {
            LauncherTab::Play => self.render_play_tab(ctx, ui),
            LauncherTab::Installations => self.render_installations_tab(ui),
            LauncherTab::Servers => self.render_servers_tab(ui),
            LauncherTab::PatchNotes => self.render_patch_notes_tab(ui),
            LauncherTab::Settings => self.render_settings_tab(ui),
        }
    }

    /// Renders the Play tab (matching Image 4).
    fn render_play_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // --- Hero Banner with TELOS Logo and Promo Card ---
            let hero_rect = ui.allocate_space(egui::vec2(ui.available_width(), 260.0)).1;
            let painter = ui.painter();

            // Gradient backdrop (dark sunset gold/amber to charcoal)
            painter.rect_filled(hero_rect, 0.0, egui::Color32::from_rgb(28, 24, 20));

            // Atmospheric horizon glow
            let glow_rect = egui::Rect::from_min_size(
                egui::pos2(hero_rect.min.x, hero_rect.min.y + 120.0),
                egui::vec2(hero_rect.width(), 140.0),
            );
            painter.rect_filled(glow_rect, 0.0, egui::Color32::from_rgb(45, 36, 26));

            // Foreground terrain silhouette
            let terrain_rect = egui::Rect::from_min_size(
                egui::pos2(hero_rect.min.x, hero_rect.min.y + 200.0),
                egui::vec2(hero_rect.width(), 60.0),
            );
            painter.rect_filled(terrain_rect, 0.0, egui::Color32::from_rgb(18, 16, 14));

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(hero_rect), |ui| {
                ui.horizontal(|ui| {
                    // Left Hero: TELOS Logo centered
                    ui.allocate_ui(egui::vec2(hero_rect.width() * 0.65, 260.0), |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(70.0);
                            if let Some(tex) = self.ensure_logo_texture(ctx) {
                                ui.image((tex.id(), egui::vec2(260.0, 65.0)));
                            } else {
                                ui.label(
                                    egui::RichText::new("TELOS")
                                        .size(42.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(255, 215, 0)),
                                );
                            }
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Infinite 3D Cubic Voxel Worlds")
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(220, 200, 160)),
                            );
                        });
                    });

                    // Right Hero: Promo Card (matching Image 4)
                    ui.vertical(|ui| {
                        ui.add_space(30.0);
                        let promo_frame = egui::Frame::new()
                            .fill(egui::Color32::from_rgba_premultiplied(20, 22, 28, 220))
                            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 75, 90)))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::same(16));

                        promo_frame.show(ui, |ui| {
                            ui.set_width(260.0);
                            ui.label(
                                egui::RichText::new("Try your own Dedicated Server!")
                                    .size(13.5)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Embark on multiplayer adventures with friends on your own always-online server.",
                                )
                                .size(11.5)
                                .color(egui::Color32::from_rgb(180, 185, 195)),
                            );
                            ui.add_space(10.0);
                            let try_btn = egui::Button::new(
                                egui::RichText::new("Try It Now")
                                    .size(11.5)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(139, 92, 246))
                            .corner_radius(4.0);
                            ui.add(try_btn);
                        });
                    });
                });
            });

            // --- Play Control Bar (matching Image 4) ---
            let playbar_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(24, 26, 32))
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 42, 50)))
                .inner_margin(egui::Margin::symmetric(16, 12));

            playbar_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: Version / Installation Selector Dropdown
                    let cur_install = self
                        .get_selected_installation()
                        .cloned()
                        .unwrap_or_else(|| Installation::new_release("v0.1.0"));

                    let selector_btn = egui::Button::new(
                        egui::RichText::new(format!("▦  {} ⌄", cur_install.name))
                            .size(13.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(34, 38, 46))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(55, 60, 72)))
                    .corner_radius(5.0)
                    .min_size(egui::vec2(220.0, 42.0));

                    if ui.add(selector_btn).clicked() {
                        self.version_dropdown_open = !self.version_dropdown_open;
                    }

                    // Center: Enormous Green PLAY Button
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let center_space = (ui.available_width() - 180.0) / 2.0;
                        if center_space > 0.0 {
                            ui.add_space(center_space.max(20.0));
                        }

                        let is_busy = self.build_state == BuildState::Building
                            || self.build_state == BuildState::WaitingForLock;

                        let play_btn = egui::Button::new(
                            egui::RichText::new(if is_busy { "BUILDING..." } else { "PLAY" })
                                .size(17.0)
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if is_busy {
                            egui::Color32::from_rgb(217, 119, 6)
                        } else {
                            egui::Color32::from_rgb(22, 163, 74)
                        })
                        .stroke(egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(34, 197, 94)))
                        .corner_radius(5.0)
                        .min_size(egui::vec2(160.0, 44.0));

                        if ui.add_enabled(!is_busy, play_btn).clicked() {
                            self.trigger_launch();
                        }
                    });

                    // Right: Version Tag & Status
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&cur_install.version_tag)
                                    .monospace()
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(156, 163, 175)),
                            );
                            ui.label(
                                egui::RichText::new("Ready")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(16, 185, 129)),
                            );
                        });
                    });
                });

                // Dropdown menu if opened
                if self.version_dropdown_open {
                    ui.add_space(6.0);
                    let dd_frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgb(30, 33, 40))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 66, 78)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(8));

                    dd_frame.show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Select Version Channel:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                        ui.add_space(4.0);

                        for inst in &self.installations {
                            let is_selected = inst.id == self.selected_installation_id;
                            let prefix = if is_selected { "● " } else { "○ " };
                            let label = format!("{prefix}{} ({})", inst.name, inst.version_tag);

                            if ui.selectable_label(is_selected, label).clicked() {
                                self.selected_installation_id = inst.id.clone();
                                self.profile.selected_installation_id = inst.id.clone();
                                let _ = save_profile(&self.profile);
                                self.version_dropdown_open = false;
                            }
                        }
                    });
                }

                // If currently building/compiling: render live progress bar!
                if self.build_state != BuildState::Idle {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&self.build_status_text)
                            .size(12.0)
                            .strong()
                            .color(if self.build_state == BuildState::Failed {
                                egui::Color32::from_rgb(239, 68, 68)
                            } else {
                                egui::Color32::from_rgb(220, 225, 235)
                            }),
                    );

                    ui.add_space(4.0);
                    let bar_rect = ui.allocate_space(egui::vec2(ui.available_width(), 6.0)).1;
                    let painter = ui.painter();
                    painter.rect_filled(bar_rect, 3.0, egui::Color32::from_rgb(38, 44, 54));

                    let frac = if self.build_state == BuildState::Success {
                        1.0
                    } else if self.build_total_crates == 0 {
                        0.1
                    } else {
                        ((self.build_completed_crates as f32) / (self.build_total_crates as f32))
                            .clamp(0.05, 0.95)
                    };

                    let fill_w = bar_rect.width() * frac;
                    let fill_rect = egui::Rect::from_min_max(
                        bar_rect.min,
                        egui::pos2(bar_rect.min.x + fill_w, bar_rect.max.y),
                    );
                    painter.rect_filled(fill_rect, 3.0, egui::Color32::from_rgb(16, 185, 129));
                }
            });

            ui.add_space(16.0);

            // --- News Strip / Announcements (matching Image 4) ---
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new("FEATURED UPDATES & NEWS")
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::from_rgb(156, 163, 175)),
                );
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.add_space(16.0);

                let cards = [
                    (
                        "Wilderness Bound",
                        "3D Multi-Noise Terrain",
                        "Continuous cubic generation with Voronoi aquifers & cheese caves.",
                        egui::Color32::from_rgb(217, 119, 6),
                    ),
                    (
                        "Vulkan 1.3 Pipeline",
                        "Modern Renderer",
                        "Bindless texture arrays, buffer device addresses, and 60+ FPS.",
                        egui::Color32::from_rgb(37, 99, 235),
                    ),
                    (
                        "Multiplayer Browser",
                        "Server Groups & Drag",
                        "Collapsible server categories with drag-and-drop reordering.",
                        egui::Color32::from_rgb(16, 185, 129),
                    ),
                    (
                        "Cubic Chunks 3D",
                        "Infinite Vertical Space",
                        "Independent 32³ subchunk cubes with uniform air/bedrock compaction.",
                        egui::Color32::from_rgb(147, 51, 234),
                    ),
                ];

                for (title, tag, desc, color) in cards {
                    let card_frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgb(26, 29, 36))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 46, 56)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(12));

                    card_frame.show(ui, |ui| {
                        ui.set_width(175.0);
                        ui.set_height(140.0);

                        // Card Thumbnail Header
                        let thumb_rect = ui.allocate_space(egui::vec2(ui.available_width(), 48.0)).1;
                        ui.painter().rect_filled(thumb_rect, 4.0, color);
                        ui.painter().text(
                            thumb_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            tag,
                            egui::FontId::proportional(11.0),
                            egui::Color32::WHITE,
                        );

                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(title)
                                .size(12.5)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(desc)
                                .size(10.5)
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                    });

                    ui.add_space(8.0);
                }
            });

            ui.add_space(20.0);
        });
    }

    /// Renders the Installations management tab.
    fn render_installations_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.heading("Installations");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                let _ = ui.button("+ New Installation");
            });
        });

        ui.add_space(14.0);

        for inst in &self.installations {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let card_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(26, 29, 36))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 46, 56)))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(14, 10));

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width() - 40.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("▦")
                                .size(20.0)
                                .color(egui::Color32::from_rgb(16, 185, 129)),
                        );
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&inst.name)
                                    .size(13.0)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(&inst.version_tag)
                                    .monospace()
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(156, 163, 175)),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_current = inst.id == self.selected_installation_id;
                            if ui
                                .button(if is_current { "Selected" } else { "Select" })
                                .clicked()
                            {
                                self.selected_installation_id = inst.id.clone();
                                self.profile.selected_installation_id = inst.id.clone();
                                let _ = save_profile(&self.profile);
                            }
                            let _ = ui.button("📁 Folder");
                        });
                    });
                });
            });
            ui.add_space(8.0);
        }
    }

    /// Renders the Multiplayer Servers tab.
    fn render_servers_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.heading("Multiplayer Servers");
        });
        ui.add_space(12.0);

        let sample_servers = [
            ("Official Telos Alpha Server", "play.telos.voxel:25565", "Official"),
            ("Local Dev Test Server", "127.0.0.1:25565", "LAN / Local"),
        ];

        for (name, addr, group) in sample_servers {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let card_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(26, 29, 36))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 46, 56)))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(14, 10));

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width() - 40.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("●")
                                .size(12.0)
                                .color(egui::Color32::from_rgb(16, 185, 129)),
                        );
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(name)
                                    .size(13.0)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{addr}  [{group}]"))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(156, 163, 175)),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Connect").clicked() {
                                self.trigger_launch();
                            }
                        });
                    });
                });
            });
            ui.add_space(8.0);
        }
    }

    /// Renders Patch Notes / What's New tab.
    fn render_patch_notes_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.heading("Patch Notes & Repository History");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(format!("Latest Commit: {}", self.remote_versions.latest_commit_hash))
                        .monospace()
                        .size(11.0)
                        .color(egui::Color32::from_rgb(16, 185, 129)),
                );
            });
        });
        ui.add_space(14.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in &self.remote_versions.patch_notes {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let card_frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgb(26, 29, 36))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(42, 46, 56)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(14));

                    card_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width() - 40.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&entry.title)
                                    .size(13.5)
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(format!("({})", entry.version))
                                    .monospace()
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(56, 189, 248)),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(&entry.date)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(156, 163, 175)),
                                );
                            });
                        });
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(&entry.summary)
                                .size(11.5)
                                .color(egui::Color32::from_rgb(209, 213, 219)),
                        );
                    });
                });
                ui.add_space(10.0);
            }
        });
    }

    /// Renders Settings tab.
    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.heading("Launcher Settings");
        });
        ui.add_space(14.0);

        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Launcher Version: v1.0.0-release")
                        .size(13.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(8.0);

                // Check for updates button (triggers standalone updater dialog!)
                if ui.button("Check for Launcher Updates").clicked() {
                    let updater_bin = self.repo_dir.join("target/release/telos-updater");
                    if updater_bin.exists() {
                        let _ = Command::new(&updater_bin).spawn();
                    } else {
                        let _ = Command::new("cargo")
                            .current_dir(&self.repo_dir)
                            .args(["run", "--bin", "telos-updater"])
                            .spawn();
                    }
                }

                ui.add_space(14.0);
                if ui
                    .checkbox(&mut self.profile.auto_close_on_play, "Keep launcher open when game launches")
                    .changed()
                {
                    let _ = save_profile(&self.profile);
                }

                ui.add_space(18.0);
                ui.separator();
                ui.add_space(12.0);

                if ui
                    .button(
                        egui::RichText::new("Log Out")
                            .color(egui::Color32::from_rgb(239, 68, 68)),
                    )
                    .clicked()
                {
                    self.profile.logged_in = false;
                    let _ = save_profile(&self.profile);
                }
            });
        });
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // If --updater is passed, run the OS-local updater dialog directly
    if args.iter().any(|a| a == "--updater") {
        let should_launch = Arc::new(AtomicBool::new(false));
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([460.0, 160.0])
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_title("telos-updater"),
            ..Default::default()
        };

        return eframe::run_native(
            "telos-updater",
            options,
            Box::new(move |_cc| {
                Ok(Box::new(crate::updater_embed::TelosUpdaterApp::new(should_launch)))
            }),
        )
        .map_err(|e| anyhow::anyhow!("Updater window error: {e:?}"));
    }

    let repo_dir = resolve_repo_dir();
    let should_launch_game = Arc::new(AtomicBool::new(false));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 537.0])
            .with_min_inner_size([1024.0, 537.0])
            .with_decorations(false)
            .with_title("Telos Launcher"),
        ..Default::default()
    };

    let flag = Arc::clone(&should_launch_game);
    let _ = eframe::run_native(
        "Telos Launcher",
        options,
        Box::new(move |_cc| Ok(Box::new(TelosLauncherApp::new(repo_dir, flag)))),
    );

    Ok(())
}

#[doc(hidden)]
mod updater_embed {
    pub use super::updater_impl::TelosUpdaterApp;
}

#[doc(hidden)]
#[path = "updater.rs"]
mod updater_impl;
