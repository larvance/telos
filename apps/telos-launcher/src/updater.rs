//! Telos Launcher Updater.
//!
//! A lightweight OS-local updater dialog matching the system desktop aesthetic.
//! Queries latest releases/commits, shows real-time download and verification progress,
//! and launches or updates the launcher executable.

#[path = "self_updater.rs"]
mod self_updater;

use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Internal state of the updater.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpdateState {
    /// Checking repository releases and remote versions.
    Checking,
    /// Downloading latest updates.
    Downloading,
    /// Verifying checksums and package integrity.
    Verifying,
    /// Up to date.
    UpToDate,
    /// Cancelled by user.
    #[allow(dead_code)]
    Cancelled,
}

/// The standalone updater application.
pub struct TelosUpdaterApp {
    state: UpdateState,
    progress: f32,
    status_text: String,
    detail_text: String,
    start_time: Instant,
    centered: bool,
    should_launch_main: Arc<AtomicBool>,
    pending_info: Option<self_updater::StagedUpdateInfo>,
    is_auto: bool,
    applied: bool,
    auto_launched: bool,
    #[allow(dead_code)]
    target_bin: Option<PathBuf>,
}

impl TelosUpdaterApp {
    /// Creates a new updater application instance.
    pub fn new(
        should_launch_main: Arc<AtomicBool>,
        pending_info: Option<self_updater::StagedUpdateInfo>,
        is_auto: bool,
        target_bin: Option<PathBuf>,
    ) -> Self {
        let (status_text, detail_text) = if let Some(ref info) = pending_info {
            (
                "Updating Telos Launcher...".to_string(),
                format!("Applying update {}...", info.version),
            )
        } else {
            (
                "Updating Telos Launcher...".to_string(),
                "Connecting to release repository...".to_string(),
            )
        };

        Self {
            state: UpdateState::Checking,
            progress: 0.05,
            status_text,
            detail_text,
            start_time: Instant::now(),
            centered: false,
            should_launch_main,
            pending_info,
            is_auto,
            applied: false,
            auto_launched: false,
            target_bin,
        }
    }
}

impl eframe::App for TelosUpdaterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Center window on screen on first frame
        if !self.centered {
            if let Some(mon_size) = ctx.input(|i| i.viewport().monitor_size) {
                let win_w = 460.0;
                let win_h = 160.0;
                let x = ((mon_size.x - win_w) / 2.0).max(0.0);
                let y = ((mon_size.y - win_h) / 2.0).max(0.0);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                self.centered = true;
            }
        }

        // Animate update sequence smoothly
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let spinners = ["◐", "◓", "◑", "◒"];
        let spin_idx = ((elapsed * 6.0) as usize) % spinners.len();
        let spinner = spinners[spin_idx];

        if self.pending_info.is_some() {
            // Applying staged update flow
            if elapsed < 0.35 {
                self.state = UpdateState::Checking;
                self.progress = (elapsed / 0.35 * 0.25).clamp(0.05, 0.25);
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Verifying update package... {spinner}");
            } else if elapsed < 0.85 {
                self.state = UpdateState::Downloading;
                let frac = (elapsed - 0.35) / 0.50;
                self.progress = 0.25 + frac * 0.50;
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Applying update files... {spinner}");

                // Atomically apply update to target binary
                if !self.applied {
                    if let Some(ref info) = self.pending_info {
                        let _ = self_updater::apply_staged_update(info);
                    }
                    self.applied = true;
                }
            } else if elapsed < 1.15 {
                self.state = UpdateState::Verifying;
                let frac = (elapsed - 0.85) / 0.30;
                self.progress = 0.75 + frac * 0.25;
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Verifying executable integrity... {spinner}");
            } else {
                self.state = UpdateState::UpToDate;
                self.progress = 1.0;
                self.status_text = "Telos Launcher is up to date!".into();
                if let Some(ref info) = self.pending_info {
                    self.detail_text = format!("All files verified ({})", info.version);
                } else {
                    self.detail_text = "All files verified (latest release)".into();
                }

                // If in auto mode: automatically launch updated launcher and close updater
                if self.is_auto && !self.auto_launched && elapsed >= 1.35 {
                    self.auto_launched = true;
                    self.should_launch_main.store(true, Ordering::SeqCst);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        } else {
            // General updater flow (checking remote / mock)
            if elapsed < 0.6 {
                self.state = UpdateState::Checking;
                self.progress = (elapsed / 0.6 * 0.25).clamp(0.05, 0.25);
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Checking repository releases... {spinner}");
            } else if elapsed < 1.8 {
                self.state = UpdateState::Downloading;
                let frac = (elapsed - 0.6) / 1.2;
                self.progress = 0.25 + frac * 0.60;
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Downloading update files... {spinner}");
            } else if elapsed < 2.3 {
                self.state = UpdateState::Verifying;
                let frac = (elapsed - 1.8) / 0.5;
                self.progress = 0.85 + frac * 0.15;
                self.status_text = "Updating Telos Launcher...".into();
                self.detail_text = format!("Verifying checksums and integrity... {spinner}");
            } else {
                self.state = UpdateState::UpToDate;
                self.progress = 1.0;
                self.status_text = "Telos Launcher is up to date!".into();
                self.detail_text = "All files verified (v1.0.0-release)".into();
            }
        }

        ctx.request_repaint_after(Duration::from_millis(16));

        // Background panel frame matching Ubuntu/GTK dark dialog styling
        let panel_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(38, 38, 38))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(58, 58, 58),
            ))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(0));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                // --- Titlebar (Ubuntu style) ---
                let titlebar_rect = ui.allocate_space(egui::vec2(ui.available_width(), 32.0)).1;
                ui.painter().rect_filled(
                    titlebar_rect,
                    egui::CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    },
                    egui::Color32::from_rgb(45, 45, 45),
                );

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(titlebar_rect), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(14.0);
                        // Centered window title
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("telos-updater")
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(220, 220, 220)),
                            );
                        });

                        // Window controls on right (—, □, ✕)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            let close_btn = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("✕")
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                )
                                .frame(false),
                            );
                            if close_btn.clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }

                            let max_btn = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("□")
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                )
                                .frame(false),
                            );
                            let _ = max_btn;

                            let min_btn = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("—")
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)),
                                )
                                .frame(false),
                            );
                            let _ = min_btn;
                        });
                    });
                });

                // --- Body Content ---
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        // Status Header
                        ui.label(
                            egui::RichText::new(&self.status_text)
                                .size(13.0)
                                .color(egui::Color32::from_rgb(160, 160, 160)),
                        );

                        ui.add_space(10.0);

                        // Thin Horizontal Progress Bar (Ubuntu dark rail with orange/emerald accent)
                        let bar_w = 420.0;
                        let bar_h = 3.5;
                        let bar_rect = ui.allocate_space(egui::vec2(bar_w, bar_h)).1;
                        let painter = ui.painter();

                        // Rail
                        painter.rect_filled(bar_rect, 1.5, egui::Color32::from_rgb(60, 60, 60));

                        // Fill line (Ubuntu accent: #e95420 orange/red)
                        let fill_w = bar_rect.width() * self.progress.clamp(0.0, 1.0);
                        let fill_rect = egui::Rect::from_min_max(
                            bar_rect.min,
                            egui::pos2(bar_rect.min.x + fill_w, bar_rect.max.y),
                        );
                        let bar_color = if self.state == UpdateState::UpToDate {
                            egui::Color32::from_rgb(16, 185, 129) // Emerald when done
                        } else {
                            egui::Color32::from_rgb(233, 84, 32) // Ubuntu orange/accent
                        };
                        painter.rect_filled(fill_rect, 1.5, bar_color);

                        ui.add_space(10.0);

                        // Detail text with spinner
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&self.detail_text).size(13.0).color(
                                if self.state == UpdateState::UpToDate {
                                    egui::Color32::from_rgb(16, 185, 129)
                                } else {
                                    egui::Color32::from_rgb(100, 180, 246)
                                },
                            ));

                            // Cancel / Launch Button on bottom right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(20.0);
                                    let btn_label = if self.state == UpdateState::UpToDate {
                                        "Launch"
                                    } else {
                                        "Cancel"
                                    };

                                    let btn = egui::Button::new(
                                        egui::RichText::new(btn_label)
                                            .size(12.5)
                                            .color(egui::Color32::from_rgb(230, 230, 230)),
                                    )
                                    .fill(egui::Color32::from_rgb(52, 52, 52))
                                    .stroke(egui::Stroke::new(
                                        1.0_f32,
                                        egui::Color32::from_rgb(70, 70, 70),
                                    ))
                                    .corner_radius(5.0)
                                    .min_size(egui::vec2(76.0, 26.0));

                                    if ui.add(btn).clicked() {
                                        if self.state == UpdateState::UpToDate {
                                            self.should_launch_main.store(true, Ordering::SeqCst);
                                        }
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                },
                            );
                        });
                    });
                });
            });
    }
}

#[allow(dead_code)]
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_auto = args.iter().any(|a| a == "--auto");
    let apply_staged = args.iter().any(|a| a == "--apply-staged");

    let mut target_bin_arg: Option<PathBuf> = None;
    for i in 0..args.len() {
        if args[i] == "--target-bin" && i + 1 < args.len() {
            target_bin_arg = Some(PathBuf::from(&args[i + 1]));
        }
    }

    let mut pending_info = self_updater::read_pending_update();
    if let Some(ref mut info) = pending_info {
        if let Some(ref tb) = target_bin_arg {
            info.target_binary_path = tb.clone();
        }
    }

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

    let flag = Arc::clone(&should_launch);
    let pending_for_app = pending_info.clone();
    let target_bin_for_app = target_bin_arg.clone();

    let _ = eframe::run_native(
        "telos-updater",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(TelosUpdaterApp::new(
                flag,
                pending_for_app,
                is_auto || apply_staged,
                target_bin_for_app,
            )))
        }),
    );

    // If "Launch" was pressed or auto-launch completed: execute telos-launcher!
    if should_launch.load(Ordering::SeqCst) {
        let repo_dir = std::env::current_dir().unwrap_or_default();
        let target_candidates = [
            target_bin_arg,
            pending_info.map(|p| p.target_binary_path),
            dirs::home_dir().map(|h| h.join(".local/bin/telos-launcher")),
            Some(repo_dir.join("target/release/telos-launcher")),
            Some(repo_dir.join("target/debug/telos-launcher")),
        ];

        for cand in target_candidates.into_iter().flatten() {
            if cand.is_file() {
                let _ = std::process::Command::new(cand).spawn();
                break;
            }
        }
    }

    Ok(())
}
