//! Telos developer build progress splash and concurrency coordinator binary (`telos-dev`).
//!
//! Provides the standalone developer splash window that monitors build locks and compilations.

#[path = "build_coordinator.rs"]
mod build_coordinator;

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant, SystemTime};

use build_coordinator::{BuildEvent, start_build_worker};
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DevState {
    Starting,
    Building,
    WaitingForLock,
    Success,
    Failed,
}

struct DevApp {
    rx: Receiver<BuildEvent>,
    state: DevState,
    status_text: String,
    completed_crates: usize,
    total_crates: usize,
    start_time: Instant,
    success_time: Option<Instant>,
    error_output: String,
    centered: bool,
    is_debug: bool,
    should_launch: Arc<AtomicBool>,
}

impl DevApp {
    fn new(rx: Receiver<BuildEvent>, is_debug: bool, should_launch: Arc<AtomicBool>) -> Self {
        Self {
            rx,
            state: DevState::Starting,
            status_text: "Initializing Telos build coordinator...".to_string(),
            completed_crates: 0,
            total_crates: 28,
            start_time: Instant::now(),
            success_time: None,
            error_output: String::new(),
            centered: false,
            is_debug,
            should_launch,
        }
    }
}

impl eframe::App for DevApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.centered {
            if let Some(mon_size) = ctx.input(|i| i.viewport().monitor_size) {
                let win_w = 400.0;
                let win_h = if self.state == DevState::Failed {
                    260.0
                } else {
                    135.0
                };
                let x = ((mon_size.x - win_w) / 2.0).max(0.0);
                let y = ((mon_size.y - win_h) / 2.0).max(0.0);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                self.centered = true;
            }
        }

        while let Ok(event) = self.rx.try_recv() {
            match event {
                BuildEvent::Status(msg) => {
                    self.status_text = msg;
                }
                BuildEvent::CrateStarted(name) => {
                    self.state = DevState::Building;
                    self.status_text = format!("Compiling {name}...");
                }
                BuildEvent::ArtifactFinished {
                    crate_name,
                    is_target_bin,
                } => {
                    self.state = DevState::Building;
                    self.completed_crates += 1;
                    if self.completed_crates > self.total_crates {
                        self.total_crates = self.completed_crates + 1;
                    }
                    if is_target_bin {
                        self.status_text = "Finalizing executable...".to_string();
                    } else {
                        self.status_text = format!(
                            "Built {crate_name} ({}/{})",
                            self.completed_crates, self.total_crates
                        );
                    }
                }
                BuildEvent::LockHeld { message } => {
                    self.state = DevState::WaitingForLock;
                    self.status_text = message;
                }
                BuildEvent::Success { already_built } => {
                    self.state = DevState::Success;
                    self.success_time = Some(Instant::now());
                    self.should_launch.store(true, Ordering::SeqCst);
                    if already_built {
                        self.status_text =
                            "Code unchanged! Launching Telos...".to_string();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.status_text = "Build successful! Launching Telos...".to_string();
                    }
                }
                BuildEvent::Failed(err) => {
                    self.state = DevState::Failed;
                    self.status_text = "Build Failed".to_string();
                    self.error_output = err;
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        500.0, 280.0,
                    )));
                }
            }
        }

        if let Some(t) = self.success_time {
            if t.elapsed() >= Duration::from_millis(180) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            ctx.request_repaint();
        } else if self.state != DevState::Failed {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        let panel_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(18, 20, 28))
            .stroke(egui::Stroke::new(
                1.5_f32,
                if self.state == DevState::Failed {
                    egui::Color32::from_rgb(239, 68, 68)
                } else if self.state == DevState::Success {
                    egui::Color32::from_rgb(16, 185, 129)
                } else if self.state == DevState::WaitingForLock {
                    egui::Color32::from_rgb(245, 158, 11)
                } else {
                    egui::Color32::from_rgb(59, 130, 246)
                },
            ))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(12));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TELOS")
                            .monospace()
                            .strong()
                            .size(15.0)
                            .color(egui::Color32::from_rgb(240, 243, 246)),
                    );
                    let badge_color = if self.is_debug {
                        egui::Color32::from_rgb(168, 85, 247)
                    } else {
                        egui::Color32::from_rgb(56, 189, 248)
                    };
                    ui.label(
                        egui::RichText::new(if self.is_debug {
                            "DEBUG DEV"
                        } else {
                            "DEV BUILD"
                        })
                        .monospace()
                        .size(11.0)
                        .color(badge_color),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new("✕")
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(156, 163, 175)),
                            )
                            .frame(false),
                        );
                        if close_btn.clicked() {
                            self.should_launch.store(false, Ordering::SeqCst);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&self.status_text)
                        .size(13.0)
                        .strong()
                        .color(egui::Color32::from_rgb(229, 231, 235)),
                );

                ui.add_space(6.0);
                let bar_height = 8.0;
                let bar_rect = ui
                    .allocate_space(egui::vec2(ui.available_width(), bar_height))
                    .1;
                let painter = ui.painter();
                painter.rect_filled(bar_rect, 4.0, egui::Color32::from_rgb(31, 41, 55));

                if self.state == DevState::Success {
                    painter.rect_filled(bar_rect, 4.0, egui::Color32::from_rgb(16, 185, 129));
                } else if self.state == DevState::WaitingForLock {
                    let time = ctx.input(|i| i.time) as f32;
                    let wave_pos = (time * 1.5).fract();
                    let pulse_w = bar_rect.width() * 0.35;
                    let start_x =
                        bar_rect.min.x + (bar_rect.width() + pulse_w) * wave_pos - pulse_w;
                    let clip_min = start_x.max(bar_rect.min.x);
                    let clip_max = (start_x + pulse_w).min(bar_rect.max.x);
                    if clip_max > clip_min {
                        let pulse_rect = egui::Rect::from_min_max(
                            egui::pos2(clip_min, bar_rect.min.y),
                            egui::pos2(clip_max, bar_rect.max.y),
                        );
                        painter.rect_filled(pulse_rect, 4.0, egui::Color32::from_rgb(245, 158, 11));
                    }
                } else if self.state == DevState::Building {
                    let frac = if self.total_crates == 0 {
                        0.1
                    } else {
                        ((self.completed_crates as f32) / (self.total_crates as f32))
                            .clamp(0.05, 0.95)
                    };
                    let filled_w = bar_rect.width() * frac;
                    let fill_rect = egui::Rect::from_min_max(
                        bar_rect.min,
                        egui::pos2(bar_rect.min.x + filled_w, bar_rect.max.y),
                    );
                    painter.rect_filled(fill_rect, 4.0, egui::Color32::from_rgb(59, 130, 246));
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let elapsed = self.start_time.elapsed().as_secs();
                    let time_str = format!("{}:{:02}s", elapsed / 60, elapsed % 60);
                    ui.label(
                        egui::RichText::new(time_str)
                            .monospace()
                            .size(11.0)
                            .color(egui::Color32::from_rgb(156, 163, 175)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let role_str = match self.state {
                            DevState::WaitingForLock => "Waiting on Lock (Instance #2)",
                            DevState::Building => "Builder (Lock Held)",
                            DevState::Success => "Ready to Launch",
                            DevState::Failed => "Compilation Error",
                            DevState::Starting => "Acquiring Lock",
                        };
                        ui.label(
                            egui::RichText::new(role_str)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                    });
                });

                if self.state == DevState::Failed && !self.error_output.is_empty() {
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.error_output.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .text_color(egui::Color32::from_rgb(248, 113, 113)),
                            );
                        });
                }
            });
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_debug = std::env::var("TELOS_DEBUG").map_or(false, |v| v == "1");
    let repo_dir = build_coordinator::resolve_repo_dir();
    let start_time = SystemTime::now();

    let target_bin = if is_debug {
        repo_dir.join("target/debug/telos")
    } else {
        repo_dir.join("target/release/telos")
    };

    let (tx, rx) = channel::<BuildEvent>();
    let should_launch = Arc::new(AtomicBool::new(false));

    start_build_worker(repo_dir.clone(), is_debug, tx, start_time);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 135.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_title("Telos [Dev]")
            .with_always_on_top(),
        ..Default::default()
    };

    let app_launch_flag = Arc::clone(&should_launch);
    let _ = eframe::run_native(
        "Telos Dev Build",
        options,
        Box::new(move |_cc| Ok(Box::new(DevApp::new(rx, is_debug, app_launch_flag)))),
    );

    if should_launch.load(Ordering::SeqCst) && target_bin.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let mut cmd = Command::new(&target_bin);
            cmd.args(&args);
            let err = cmd.exec();
            anyhow::bail!("Failed to exec telos binary: {err}");
        }
        #[cfg(not(unix))]
        {
            let mut child = Command::new(&target_bin).args(&args).spawn()?;
            child.wait()?;
        }
    }

    Ok(())
}
