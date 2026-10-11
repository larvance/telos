//! Telos developer build progress launcher and concurrency coordinator.
//!
//! Provides a centered, custom-framed developer splash window displaying real-time
//! crate compilation progress, and coordinates multi-instance build locks so that
//! secondary instances wait for active builds and skip redundant rebuilds.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

/// Event sent from the background compilation/lock worker to the egui UI.
#[derive(Debug)]
enum BuildEvent {
    /// Update status message with current activity.
    Status(String),
    /// A crate has started compiling.
    CrateStarted(String),
    /// A crate artifact has completed.
    ArtifactFinished {
        crate_name: String,
        is_target_bin: bool,
    },
    /// Build lock is currently held by another process; waiting.
    LockHeld { message: String },
    /// Compilation or lock wait succeeded.
    Success { already_built: bool },
    /// Build failed with compiler error output.
    Failed(String),
}

/// Internal state of the dev launcher.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LauncherState {
    Starting,
    Building,
    WaitingForLock,
    Success,
    Failed,
}

/// State container shared across frames in the egui event loop.
struct TelosDevApp {
    rx: Receiver<BuildEvent>,
    state: LauncherState,
    status_text: String,
    sub_text: String,
    current_crate: String,
    completed_crates: usize,
    total_crates: usize,
    start_time: Instant,
    success_time: Option<Instant>,
    error_output: String,
    centered: bool,
    is_debug: bool,
    should_launch: Arc<AtomicBool>,
}

impl TelosDevApp {
    fn new(rx: Receiver<BuildEvent>, is_debug: bool, should_launch: Arc<AtomicBool>) -> Self {
        Self {
            rx,
            state: LauncherState::Starting,
            status_text: "Initializing Telos build coordinator...".to_string(),
            sub_text: "Checking build directory lock".to_string(),
            current_crate: String::new(),
            completed_crates: 0,
            total_crates: 28, // Typical workspace crate count
            start_time: Instant::now(),
            success_time: None,
            error_output: String::new(),
            centered: false,
            is_debug,
            should_launch,
        }
    }
}

impl eframe::App for TelosDevApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Center the window on screen on the first frame once monitor size is known
        if !self.centered {
            if let Some(mon_size) = ctx.input(|i| i.viewport().monitor_size) {
                let win_w = 400.0;
                let win_h = if self.state == LauncherState::Failed { 260.0 } else { 135.0 };
                let x = ((mon_size.x - win_w) / 2.0).max(0.0);
                let y = ((mon_size.y - win_h) / 2.0).max(0.0);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                self.centered = true;
            }
        }

        // 2. Process incoming build events
        while let Ok(event) = self.rx.try_recv() {
            match event {
                BuildEvent::Status(msg) => {
                    self.status_text = msg;
                }
                BuildEvent::CrateStarted(name) => {
                    self.state = LauncherState::Building;
                    self.current_crate = name.clone();
                    self.status_text = format!("Compiling {name}...");
                }
                BuildEvent::ArtifactFinished {
                    crate_name,
                    is_target_bin,
                } => {
                    self.state = LauncherState::Building;
                    self.completed_crates += 1;
                    if self.completed_crates > self.total_crates {
                        self.total_crates = self.completed_crates + 1;
                    }
                    if is_target_bin {
                        self.status_text = "Finalizing release executable (LTO)...".to_string();
                    } else {
                        self.status_text = format!(
                            "Built {crate_name} ({}/{})",
                            self.completed_crates, self.total_crates
                        );
                    }
                }
                BuildEvent::LockHeld { message } => {
                    self.state = LauncherState::WaitingForLock;
                    self.status_text = "Waiting for existing build to finish...".to_string();
                    self.sub_text = message;
                }
                BuildEvent::Success { already_built } => {
                    self.state = LauncherState::Success;
                    self.success_time = Some(Instant::now());
                    self.should_launch.store(true, Ordering::SeqCst);
                    if already_built {
                        self.status_text = "Build complete from other instance! Launching...".to_string();
                        self.sub_text = "Skipped duplicate build".to_string();
                    } else {
                        self.status_text = "Build successful! Launching Telos...".to_string();
                        self.sub_text = "Launching game client".to_string();
                    }
                }
                BuildEvent::Failed(err) => {
                    self.state = LauncherState::Failed;
                    self.status_text = "Build Failed".to_string();
                    self.error_output = err;
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(500.0, 280.0)));
                }
            }
        }

        // 3. Auto-close once success timer completes (brief visual feedback)
        if let Some(t) = self.success_time {
            if t.elapsed() >= Duration::from_millis(180) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            ctx.request_repaint();
        } else if self.state != LauncherState::Failed {
            // Keep UI animating smoothly at ~60fps while building or waiting
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // 4. Render custom frame and UI controls
        let panel_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(18, 20, 28))
            .stroke(egui::Stroke::new(
                1.5_f32,
                if self.state == LauncherState::Failed {
                    egui::Color32::from_rgb(239, 68, 68) // Red border on error
                } else if self.state == LauncherState::Success {
                    egui::Color32::from_rgb(16, 185, 129) // Emerald border on success
                } else if self.state == LauncherState::WaitingForLock {
                    egui::Color32::from_rgb(245, 158, 11) // Amber border on lock wait
                } else {
                    egui::Color32::from_rgb(59, 130, 246) // Blue accent border
                },
            ))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(12));

        egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
            // --- Custom Title Bar ---
            ui.horizontal(|ui| {
                // Drag area across title
                let title_resp = ui.scope(|ui| {
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
                        egui::RichText::new(if self.is_debug { "DEBUG DEV" } else { "DEV BUILD" })
                            .monospace()
                            .size(11.0)
                            .color(badge_color),
                    );
                }).response;

                if title_resp.interact(egui::Sense::click_and_drag()).dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

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

            // --- Status Message & Current Action ---
            ui.label(
                egui::RichText::new(&self.status_text)
                    .size(13.0)
                    .strong()
                    .color(egui::Color32::from_rgb(229, 231, 235)),
            );

            ui.add_space(6.0);

            // --- Progress Bar / Animation ---
            let bar_height = 8.0;
            let bar_rect = ui.allocate_space(egui::vec2(ui.available_width(), bar_height)).1;
            let painter = ui.painter();

            // Background rail
            painter.rect_filled(
                bar_rect,
                4.0,
                egui::Color32::from_rgb(31, 41, 55),
            );

            if self.state == LauncherState::Success {
                // 100% Emerald Green
                painter.rect_filled(
                    bar_rect,
                    4.0,
                    egui::Color32::from_rgb(16, 185, 129),
                );
            } else if self.state == LauncherState::WaitingForLock {
                // Animated amber wave/pulse across the bar
                let time = ctx.input(|i| i.time) as f32;
                let wave_pos = (time * 1.5).fract();
                let pulse_w = bar_rect.width() * 0.35;
                let start_x = bar_rect.min.x + (bar_rect.width() + pulse_w) * wave_pos - pulse_w;
                let clip_min = start_x.max(bar_rect.min.x);
                let clip_max = (start_x + pulse_w).min(bar_rect.max.x);
                if clip_max > clip_min {
                    let pulse_rect = egui::Rect::from_min_max(
                        egui::pos2(clip_min, bar_rect.min.y),
                        egui::pos2(clip_max, bar_rect.max.y),
                    );
                    painter.rect_filled(pulse_rect, 4.0, egui::Color32::from_rgb(245, 158, 11));
                }
            } else if self.state == LauncherState::Building {
                // Progress based on completed crates
                let frac = if self.total_crates == 0 {
                    0.1
                } else {
                    ((self.completed_crates as f32) / (self.total_crates as f32)).clamp(0.05, 0.95)
                };
                let filled_w = bar_rect.width() * frac;
                let fill_rect = egui::Rect::from_min_max(
                    bar_rect.min,
                    egui::pos2(bar_rect.min.x + filled_w, bar_rect.max.y),
                );
                painter.rect_filled(fill_rect, 4.0, egui::Color32::from_rgb(59, 130, 246));
            }

            ui.add_space(8.0);

            // --- Footer: Elapsed time & concurrency role ---
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
                        LauncherState::WaitingForLock => "Waiting on Lock (Instance #2)",
                        LauncherState::Building => "Builder (Lock Held)",
                        LauncherState::Success => "Ready to Launch",
                        LauncherState::Failed => "Compilation Error",
                        LauncherState::Starting => "Acquiring Lock",
                    };
                    ui.label(
                        egui::RichText::new(role_str)
                            .size(11.0)
                            .color(egui::Color32::from_rgb(156, 163, 175)),
                    );
                });
            });

            // Error display area (if failed)
            if self.state == LauncherState::Failed && !self.error_output.is_empty() {
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
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

/// Attempts to acquire an exclusive lock non-blockingly using advisory file locking.
fn try_lock_exclusive(file: &std::fs::File) -> bool {
    rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_ok()
}

/// Spawns the background build worker that coordinates locking and builds.
fn start_build_worker(
    repo_dir: PathBuf,
    is_debug: bool,
    tx: Sender<BuildEvent>,
    start_time: SystemTime,
) {
    std::thread::Builder::new()
        .name("telos-build-worker".into())
        .spawn(move || {
            let target_bin = if is_debug {
                repo_dir.join("target/debug/telos")
            } else {
                repo_dir.join("target/release/telos")
            };

            let lock_dir = repo_dir.join("target");
            let _ = std::fs::create_dir_all(&lock_dir);
            let lock_path = lock_dir.join(".telos-dev.lock");

            let lock_file = match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
            {
                Ok(f) => f,
                Err(err) => {
                    let _ = tx.send(BuildEvent::Failed(format!("Failed to open lockfile: {err}")));
                    return;
                }
            };

            // 1. Check if another instance is already building
            let initial_lock = try_lock_exclusive(&lock_file);
            let mut waited_for_lock = false;

            if !initial_lock {
                waited_for_lock = true;
                let _ = tx.send(BuildEvent::LockHeld {
                    message: "Another Telos build is in progress. Auto-launching when finished.".into(),
                });

                // Wait for the active build to complete and release the lock
                loop {
                    std::thread::sleep(Duration::from_millis(120));
                    if try_lock_exclusive(&lock_file) {
                        break;
                    }
                }
            }

            // Lock is now held exclusively by this process!
            // 2. Multi-instance rule: If we waited for the lock, check if the other instance built it!
            if waited_for_lock {
                let bin_is_fresh = target_bin.metadata().and_then(|m| m.modified()).map_or(
                    false,
                    |mtime| {
                        // Binary was created/modified during or after our wait start
                        mtime >= start_time.checked_sub(Duration::from_secs(5)).unwrap_or(start_time)
                    },
                );

                if bin_is_fresh && target_bin.exists() {
                    // DO NOT build again! The other instance already built it once!
                    let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);
                    let _ = tx.send(BuildEvent::Success {
                        already_built: true,
                    });
                    return;
                }
            }

            // 3. We are the builder or binary is missing: execute `cargo build`
            let _ = tx.send(BuildEvent::Status("Starting Cargo build...".into()));

            let mut cmd = Command::new("cargo");
            cmd.current_dir(&repo_dir);
            cmd.arg("build");
            if !is_debug {
                cmd.arg("--release");
            }
            cmd.args(["--bin", "telos", "--message-format=json"]);
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(err) => {
                    let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);
                    let _ = tx.send(BuildEvent::Failed(format!("Failed to spawn cargo: {err}")));
                    return;
                }
            };

            let stdout = child.stdout.take().expect("piped stdout");
            let stderr = child.stderr.take().expect("piped stderr");

            let tx_err = tx.clone();
            let stderr_handle = std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                let mut accumulated_errors = Vec::new();
                for line in reader.lines().map_while(Result::ok) {
                    if line.trim().starts_with("Compiling") {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.len() >= 2 {
                            let _ = tx_err.send(BuildEvent::CrateStarted(parts[1].to_string()));
                        }
                    } else if line.contains("error:") || line.contains("Error") {
                        accumulated_errors.push(line);
                    }
                }
                accumulated_errors.join("\n")
            });

            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if val.get("reason").and_then(|r| r.as_str()) == Some("compiler-artifact") {
                        let target_name = val
                            .get("target")
                            .and_then(|t| t.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("crate");
                        let is_bin = val
                            .get("target")
                            .and_then(|t| t.get("kind"))
                            .and_then(|k| k.as_array())
                            .map_or(false, |kinds| {
                                kinds.iter().any(|k| k.as_str() == Some("bin"))
                            });

                        let _ = tx.send(BuildEvent::ArtifactFinished {
                            crate_name: target_name.to_string(),
                            is_target_bin: is_bin && target_name == "telos",
                        });
                    }
                }
            }

            let status = child.wait();
            let accumulated_err = stderr_handle.join().unwrap_or_default();

            // Release file lock immediately upon build completion
            let _ = rustix::fs::flock(&lock_file, rustix::fs::FlockOperation::Unlock);

            match status {
                Ok(exit) if exit.success() => {
                    let _ = tx.send(BuildEvent::Success {
                        already_built: false,
                    });
                }
                Ok(exit) => {
                    let _ = tx.send(BuildEvent::Failed(format!(
                        "Cargo exited with code {}\n{}",
                        exit.code().unwrap_or(1),
                        accumulated_err
                    )));
                }
                Err(err) => {
                    let _ = tx.send(BuildEvent::Failed(format!("Cargo process error: {err}")));
                }
            }
        })
        .expect("spawn build worker thread");
}

fn resolve_repo_dir() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = Path::new(&manifest_dir);
        if let Some(parent) = p.parent().and_then(Path::parent) {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
        }
    }
    let default_path = PathBuf::from("/home/usr/Projects/larvance/telos");
    if default_path.join("Cargo.toml").exists() {
        return default_path;
    }
    PathBuf::from("/home/usr/Projects/voxel")
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_debug = std::env::var("TELOS_DEBUG").map_or(false, |v| v == "1");
    let repo_dir = resolve_repo_dir();
    let start_time = SystemTime::now();

    let target_bin = if is_debug {
        repo_dir.join("target/debug/telos")
    } else {
        repo_dir.join("target/release/telos")
    };

    let (tx, rx) = channel::<BuildEvent>();
    let should_launch = Arc::new(AtomicBool::new(false));

    start_build_worker(
        repo_dir.clone(),
        is_debug,
        tx,
        start_time,
    );

    // If headless environment (no X11 / Wayland), don't crash on GUI: wait for worker
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        println!("Telos Dev: Running in headless mode (waiting for build)...");
        while let Ok(evt) = rx.recv() {
            match evt {
                BuildEvent::Status(s) | BuildEvent::LockHeld { message: s } => {
                    println!("[Telos Dev] {s}");
                }
                BuildEvent::CrateStarted(c) => {
                    println!("[Telos Dev] Compiling {c}...");
                }
                BuildEvent::Success { .. } => {
                    should_launch.store(true, Ordering::SeqCst);
                    break;
                }
                BuildEvent::Failed(err) => {
                    eprintln!("[Telos Dev Error] {err}");
                    std::process::exit(1);
                }
                BuildEvent::ArtifactFinished { .. } => {}
            }
        }
    } else {
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
            Box::new(move |_cc| {
                Ok(Box::new(TelosDevApp::new(rx, is_debug, app_launch_flag)))
            }),
        );
    }

    // After the egui splash window cleanly closes: launch the compiled Telos binary!
    if should_launch.load(Ordering::SeqCst) {
        if !target_bin.exists() {
            anyhow::bail!("Target binary not found at {:?}", target_bin);
        }

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
