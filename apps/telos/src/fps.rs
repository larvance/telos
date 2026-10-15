//! High-precision sliding-window FPS and frame-time tracker.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Sliding-window frame duration and frame rate tracker.
#[derive(Debug)]
pub struct FpsTracker {
    last_frame: Instant,
    recent_durations: VecDeque<(Instant, f64)>,
    current_fps: u32,
    current_frame_time_ms: f32,
    last_display_update: Instant,
    has_first_frame: bool,
}

impl Default for FpsTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsTracker {
    /// Creates a new `FpsTracker` initialized with standard 60 FPS defaults.
    #[must_use]
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_frame: now,
            recent_durations: VecDeque::with_capacity(128),
            current_fps: 60,
            current_frame_time_ms: 16.67,
            last_display_update: now,
            has_first_frame: false,
        }
    }

    /// Records the tick of a frame and updates sliding window metrics.
    ///
    /// Returns `(fps, frame_time_ms)`.
    pub fn tick(&mut self) -> (u32, f32) {
        let now = Instant::now();
        if !self.has_first_frame {
            self.has_first_frame = true;
            self.last_frame = now;
            self.last_display_update = now;
            return (self.current_fps, self.current_frame_time_ms);
        }

        let frame_dur = (now - self.last_frame).as_secs_f64();
        self.last_frame = now;

        if frame_dur > 0.0 {
            self.recent_durations.push_back((now, frame_dur));
        }

        // Evict samples older than 500 ms, keeping at least 1 recent sample
        let cutoff = now.checked_sub(Duration::from_millis(500)).unwrap_or(now);
        while let Some(&(ts, _)) = self.recent_durations.front() {
            if ts < cutoff && self.recent_durations.len() > 1 {
                self.recent_durations.pop_front();
            } else {
                break;
            }
        }

        // Update displayed metrics at 10 Hz (every 100ms) or when first starting
        if now.duration_since(self.last_display_update) >= Duration::from_millis(100)
            || self.recent_durations.len() <= 2
        {
            self.last_display_update = now;
            let count = self.recent_durations.len();
            if count > 0 {
                let total_time: f64 = self.recent_durations.iter().map(|(_, d)| *d).sum();
                let avg_dur = total_time / count as f64;
                self.current_fps = if avg_dur > 0.0 {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let fps = (1.0 / avg_dur).round().clamp(0.0, 9999.0) as u32;
                    fps
                } else {
                    0
                };
                self.current_frame_time_ms = ((avg_dur * 1000.0) as f32).max(0.01);
            }
        }

        (self.current_fps, self.current_frame_time_ms)
    }

    /// Explicitly records a synthetic frame duration (useful for deterministic tests).
    pub fn record_duration(&mut self, now: Instant, frame_dur_secs: f64) {
        if !self.has_first_frame {
            self.has_first_frame = true;
            self.last_frame = now;
            self.last_display_update = now;
        }

        if frame_dur_secs > 0.0 {
            self.recent_durations.push_back((now, frame_dur_secs));
        }

        let cutoff = now.checked_sub(Duration::from_millis(500)).unwrap_or(now);
        while let Some(&(ts, _)) = self.recent_durations.front() {
            if ts < cutoff && self.recent_durations.len() > 1 {
                self.recent_durations.pop_front();
            } else {
                break;
            }
        }

        if now.duration_since(self.last_display_update) >= Duration::from_millis(100)
            || self.recent_durations.len() <= 2
        {
            self.last_display_update = now;
            let count = self.recent_durations.len();
            if count > 0 {
                let total_time: f64 = self.recent_durations.iter().map(|(_, d)| *d).sum();
                let avg_dur = total_time / count as f64;
                self.current_fps = if avg_dur > 0.0 {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let fps = (1.0 / avg_dur).round().clamp(0.0, 9999.0) as u32;
                    fps
                } else {
                    0
                };
                self.current_frame_time_ms = ((avg_dur * 1000.0) as f32).max(0.01);
            }
        }
    }

    /// Current calculated frames per second.
    #[must_use]
    pub fn fps(&self) -> u32 {
        self.current_fps
    }

    /// Current rolling average frame time in milliseconds.
    #[must_use]
    pub fn frame_time_ms(&self) -> f32 {
        self.current_frame_time_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_tracker_initial_values() {
        let tracker = FpsTracker::new();
        assert_eq!(tracker.fps(), 60);
        assert!((tracker.frame_time_ms() - 16.67).abs() < 0.01);
    }

    #[test]
    fn test_fps_tracker_steady_60_fps() {
        let mut tracker = FpsTracker::new();
        let mut t = Instant::now();

        // 30 frames at 16.666 ms (~60 FPS)
        for _ in 0..30 {
            t += Duration::from_micros(16_667);
            tracker.record_duration(t, 0.016_667);
        }

        assert_eq!(tracker.fps(), 60);
        assert!((tracker.frame_time_ms() - 16.67).abs() < 0.1);
    }

    #[test]
    fn test_fps_tracker_high_refresh_144_fps() {
        let mut tracker = FpsTracker::new();
        let mut t = Instant::now();

        // 60 frames at 6.944 ms (~144 FPS)
        for _ in 0..60 {
            t += Duration::from_micros(6_944);
            tracker.record_duration(t, 0.006_944);
        }

        assert_eq!(tracker.fps(), 144);
        assert!((tracker.frame_time_ms() - 6.94).abs() < 0.1);
    }

    #[test]
    fn test_fps_tracker_lag_spike_drops_fps() {
        let mut tracker = FpsTracker::new();
        let mut t = Instant::now();

        // 10 normal frames at 60 FPS
        for _ in 0..10 {
            t += Duration::from_millis(16);
            tracker.record_duration(t, 0.016);
        }

        // Huge lag spike of 200ms
        t += Duration::from_millis(200);
        tracker.record_duration(t, 0.200);

        // Average duration over the sliding window climbs and FPS drops significantly
        assert!(
            tracker.fps() < 60,
            "FPS should drop below 60 on a lag spike, got {}",
            tracker.fps()
        );
        assert!(
            tracker.frame_time_ms() > 20.0,
            "Frame time should increase on lag spike, got {}",
            tracker.frame_time_ms()
        );
    }

    #[test]
    fn test_fps_tracker_recovers_after_lag() {
        let mut tracker = FpsTracker::new();
        let mut t = Instant::now();

        // Lag spike of 300ms
        t += Duration::from_millis(300);
        tracker.record_duration(t, 0.300);

        // Then 45 steady frames at 60 FPS (exceeding 500ms sliding window and 100ms display update)
        for _ in 0..45 {
            t += Duration::from_micros(16_667);
            tracker.record_duration(t, 0.016_667);
        }

        // Window eviction removes the 300ms lag spike, returning to steady 60 FPS
        assert_eq!(tracker.fps(), 60);
        assert!((tracker.frame_time_ms() - 16.67).abs() < 0.1);
    }
}
