//! Virtual on-screen touch controls for mobile (Android / iOS) and touchscreen devices.
//!
//! Provides a responsive dual-zone virtual joystick (movement) and camera swipe/tap controls,
//! accompanied by virtual HUD action buttons (Jump, Sneak, Break, Place, Pause, Inventory).

use std::collections::HashMap;
use telos_ui::scale::snap_to_physical;
use telos_ui::{BitmapFont, UiQuad};
use winit::event::{Touch, TouchPhase};

use crate::physics::PlayerInputState;

/// Touch button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchButton {
    /// Jump action.
    Jump,
    /// Sneak action.
    Sneak,
    /// Hit / mine block action.
    Break,
    /// Use / place block action.
    Place,
    /// Pause menu action.
    Pause,
    /// Inventory action.
    Inventory,
}

/// Virtual on-screen touch controller.
#[derive(Debug, Clone)]
pub struct TouchControls {
    /// Whether virtual touch controls are currently active/visible.
    pub enabled: bool,

    /// Active touch ID for the left movement joystick.
    left_stick_id: Option<u64>,
    /// Base center of the virtual joystick in GUI coordinates.
    pub left_stick_center: [f32; 2],
    /// Current knob position of the virtual joystick in GUI coordinates.
    pub left_stick_pos: [f32; 2],
    /// Normalized directional vector [-1.0..1.0] for movement (x: strafe, y: forward/back).
    pub joystick_vec: [f32; 2],

    /// Active touch ID for camera look/swipe.
    look_touch_id: Option<u64>,
    /// Last recorded position of the look touch.
    last_look_pos: [f32; 2],
    /// Accumulated look delta in pixels to apply to camera yaw/pitch.
    pub look_delta: [f32; 2],

    /// Map from touch ID to active virtual button pressed.
    button_touches: HashMap<u64, TouchButton>,

    /// State flag for virtual jump button.
    pub jump_pressed: bool,
    /// State flag for virtual sneak button.
    pub sneak_pressed: bool,
    /// State flag for virtual break button.
    pub break_pressed: bool,
    /// State flag for virtual place button.
    pub place_pressed: bool,
    /// Pending single-shot break/attack trigger.
    pub break_requested: bool,
    /// Pending single-shot place/use trigger.
    pub place_requested: bool,
    /// Pending pause menu toggle request.
    pub pause_requested: bool,
    /// Pending inventory toggle request.
    pub inventory_requested: bool,
    /// Pending hotbar slot selection request.
    pub hotbar_slot_requested: Option<usize>,
}

impl Default for TouchControls {
    fn default() -> Self {
        Self {
            enabled: std::env::var("TELOS_TOUCH").is_ok()
                || cfg!(target_os = "android")
                || cfg!(target_os = "ios"),
            left_stick_id: None,
            left_stick_center: [70.0, 0.0], // Y resolved dynamically relative to screen height
            left_stick_pos: [70.0, 0.0],
            joystick_vec: [0.0, 0.0],
            look_touch_id: None,
            last_look_pos: [0.0, 0.0],
            look_delta: [0.0, 0.0],
            button_touches: HashMap::new(),
            jump_pressed: false,
            sneak_pressed: false,
            break_pressed: false,
            place_pressed: false,
            break_requested: false,
            place_requested: false,
            pause_requested: false,
            inventory_requested: false,
            hotbar_slot_requested: None,
        }
    }
}

impl TouchControls {
    /// Maximum radius of the virtual joystick in GUI pixels.
    pub const JOYSTICK_RADIUS: f32 = 48.0;
    /// Deadzone radius before movement is triggered.
    pub const DEADZONE_RADIUS: f32 = 8.0;

    /// Processes an incoming Winit touch event.
    pub fn handle_touch(&mut self, touch: &Touch, win_w: f32, win_h: f32, gui_scale: u32) {
        // Automatically enable touch controls when touch events arrive
        self.enabled = true;

        let scale = gui_scale.max(1) as f32;
        let gx = touch.location.x as f32 / scale;
        let gy = touch.location.y as f32 / scale;
        let id = touch.id;

        match touch.phase {
            TouchPhase::Started => {
                // 1. Check if touch landed on any virtual buttons
                if let Some(btn) = self.hit_test_buttons(gx, gy, win_w, win_h) {
                    self.button_touches.insert(id, btn);
                    self.set_button_state(btn, true);
                    return;
                }

                // 2. Check if touch landed on the hotbar (centered at bottom)
                let hotbar_w = 182.0;
                let hotbar_h = 24.0;
                let hotbar_x = (win_w - hotbar_w) * 0.5;
                let hotbar_y = win_h - hotbar_h;
                if gx >= hotbar_x && gx <= hotbar_x + hotbar_w && gy >= hotbar_y && gy <= win_h {
                    let slot = ((gx - hotbar_x) / 20.2).floor() as usize;
                    if slot < 9 {
                        self.hotbar_slot_requested = Some(slot);
                        return;
                    }
                }

                // 3. Left side: Movement Joystick
                if gx < win_w * 0.5 {
                    if self.left_stick_id.is_none() {
                        self.left_stick_id = Some(id);
                        self.left_stick_center = [gx, gy];
                        self.left_stick_pos = [gx, gy];
                        self.joystick_vec = [0.0, 0.0];
                    }
                } else {
                    // 4. Right side: Camera Look
                    if self.look_touch_id.is_none() {
                        self.look_touch_id = Some(id);
                        self.last_look_pos = [gx, gy];
                    }
                }
            }

            TouchPhase::Moved => {
                // If it's a button touch, verify it hasn't slid off
                if let Some(&btn) = self.button_touches.get(&id) {
                    let still_on = self.hit_test_buttons(gx, gy, win_w, win_h) == Some(btn);
                    self.set_button_state(btn, still_on);
                    return;
                }

                // Movement Joystick motion
                if Some(id) == self.left_stick_id {
                    let dx = gx - self.left_stick_center[0];
                    let dy = gy - self.left_stick_center[1];
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist > Self::JOYSTICK_RADIUS {
                        let ratio = Self::JOYSTICK_RADIUS / dist;
                        self.left_stick_pos = [
                            self.left_stick_center[0] + dx * ratio,
                            self.left_stick_center[1] + dy * ratio,
                        ];
                    } else {
                        self.left_stick_pos = [gx, gy];
                    }

                    if dist > Self::DEADZONE_RADIUS {
                        let norm = ((dist - Self::DEADZONE_RADIUS)
                            / (Self::JOYSTICK_RADIUS - Self::DEADZONE_RADIUS))
                            .min(1.0);
                        self.joystick_vec = [(dx / dist) * norm, (dy / dist) * norm];
                    } else {
                        self.joystick_vec = [0.0, 0.0];
                    }
                }
                // Camera Look motion
                else if Some(id) == self.look_touch_id {
                    let dx = gx - self.last_look_pos[0];
                    let dy = gy - self.last_look_pos[1];
                    self.look_delta[0] += dx;
                    self.look_delta[1] += dy;
                    self.last_look_pos = [gx, gy];
                }
            }

            TouchPhase::Ended | TouchPhase::Cancelled => {
                if let Some(btn) = self.button_touches.remove(&id) {
                    self.set_button_state(btn, false);
                }

                if Some(id) == self.left_stick_id {
                    self.left_stick_id = None;
                    self.joystick_vec = [0.0, 0.0];
                    self.left_stick_pos = self.left_stick_center;
                }

                if Some(id) == self.look_touch_id {
                    self.look_touch_id = None;
                }
            }
        }
    }

    /// Hit-tests on-screen buttons in GUI coordinates.
    fn hit_test_buttons(&self, gx: f32, gy: f32, win_w: f32, win_h: f32) -> Option<TouchButton> {
        // Pause Button: top-right corner
        if gx >= win_w - 44.0 && gx <= win_w - 8.0 && gy >= 8.0 && gy <= 44.0 {
            return Some(TouchButton::Pause);
        }

        // Inventory Button: top-left corner
        if gx >= 8.0 && gx <= 44.0 && gy >= 8.0 && gy <= 44.0 {
            return Some(TouchButton::Inventory);
        }

        // Jump Button: bottom-right (center: win_w - 55.0, win_h - 55.0, radius 26.0)
        let jmp_c = [win_w - 55.0, win_h - 55.0];
        if (gx - jmp_c[0]).hypot(gy - jmp_c[1]) <= 26.0 {
            return Some(TouchButton::Jump);
        }

        // Sneak Button: bottom-right inner (center: win_w - 105.0, win_h - 45.0, radius 20.0)
        let snk_c = [win_w - 105.0, win_h - 45.0];
        if (gx - snk_c[0]).hypot(gy - snk_c[1]) <= 20.0 {
            return Some(TouchButton::Sneak);
        }

        // Break/Attack Button: bottom-right upper (center: win_w - 55.0, win_h - 110.0, radius 22.0)
        let brk_c = [win_w - 55.0, win_h - 110.0];
        if (gx - brk_c[0]).hypot(gy - brk_c[1]) <= 22.0 {
            return Some(TouchButton::Break);
        }

        // Place/Use Button: bottom-right upper inner (center: win_w - 105.0, win_h - 95.0, radius 20.0)
        let plc_c = [win_w - 105.0, win_h - 95.0];
        if (gx - plc_c[0]).hypot(gy - plc_c[1]) <= 20.0 {
            return Some(TouchButton::Place);
        }

        None
    }

    fn set_button_state(&mut self, btn: TouchButton, active: bool) {
        match btn {
            TouchButton::Jump => self.jump_pressed = active,
            TouchButton::Sneak => self.sneak_pressed = active,
            TouchButton::Break => {
                self.break_pressed = active;
                if active {
                    self.break_requested = true;
                }
            }
            TouchButton::Place => {
                self.place_pressed = active;
                if active {
                    self.place_requested = true;
                }
            }
            TouchButton::Pause => {
                if active {
                    self.pause_requested = true;
                }
            }
            TouchButton::Inventory => {
                if active {
                    self.inventory_requested = true;
                }
            }
        }
    }

    /// Feeds active virtual touch inputs into the player physical input state.
    pub fn apply_to_player_input(&self, input: &mut PlayerInputState) {
        if !self.enabled {
            return;
        }

        let [jx, jy] = self.joystick_vec;
        if jy < -0.3 {
            input.forward = true;
        }
        if jy > 0.3 {
            input.backward = true;
        }
        if jx < -0.3 {
            input.left = true;
        }
        if jx > 0.3 {
            input.right = true;
        }

        // Sprinting when pushing joystick towards max threshold
        if (jx * jx + jy * jy) > 0.70 * 0.70 && jy < -0.4 {
            input.sprint = true;
        }

        if self.jump_pressed {
            input.jump = true;
        }
        if self.sneak_pressed {
            input.sneak = true;
        }
    }

    /// Consumes the accumulated look delta.
    pub fn consume_look_delta(&mut self) -> [f32; 2] {
        let delta = self.look_delta;
        self.look_delta = [0.0, 0.0];
        delta
    }

    /// Renders virtual touch HUD controls (joystick, jump, sneak, break, place, pause buttons).
    pub fn render(
        &self,
        font: &BitmapFont,
        win_w_phys: u32,
        win_h_phys: u32,
        gui_scale: u32,
        quads: &mut Vec<UiQuad>,
    ) {
        if !self.enabled {
            return;
        }

        let scale = gui_scale.max(1) as f32;
        let gw = win_w_phys as f32 / scale;
        let gh = win_h_phys as f32 / scale;

        // Colors
        let bg_color = UiQuad::rgba(0, 0, 0, 90);
        let border_color = UiQuad::rgba(255, 255, 255, 120);
        let active_color = UiQuad::rgba(60, 133, 39, 180);
        let knob_color = UiQuad::rgba(255, 255, 255, 160);

        let snap = |v: f32| snap_to_physical(v, gui_scale);
        let snap_u16 = |v: f32| snap_to_physical(v, gui_scale).max(1) as u16;
        let border_px = snap_u16(1.0);

        // 1. Virtual Joystick
        let center = if self.left_stick_id.is_some() {
            self.left_stick_center
        } else {
            [70.0, gh - 70.0]
        };
        let knob = if self.left_stick_id.is_some() {
            self.left_stick_pos
        } else {
            center
        };

        // Joystick base (box representation with border)
        let r = Self::JOYSTICK_RADIUS;
        let bx = snap(center[0] - r);
        let by = snap(center[1] - r);
        let bw = snap_u16(r * 2.0);
        quads.push(UiQuad::solid([bx, by], [bw, bw], bg_color));
        // 1px border around stick base
        quads.push(UiQuad::solid([bx, by], [bw, border_px], border_color));
        quads.push(UiQuad::solid(
            [bx, by + bw as i32 - border_px as i32],
            [bw, border_px],
            border_color,
        ));
        quads.push(UiQuad::solid([bx, by], [border_px, bw], border_color));
        quads.push(UiQuad::solid(
            [bx + bw as i32 - border_px as i32, by],
            [border_px, bw],
            border_color,
        ));

        // Joystick knob
        let kr = 18.0f32;
        let kx = snap(knob[0] - kr);
        let ky = snap(knob[1] - kr);
        let kw = snap_u16(kr * 2.0);
        quads.push(UiQuad::solid([kx, ky], [kw, kw], knob_color));

        // 2. Action Buttons Helper
        let draw_btn =
            |quads: &mut Vec<UiQuad>, cx: f32, cy: f32, size: f32, label: &str, active: bool| {
                let half = size * 0.5;
                let px = snap(cx - half);
                let py = snap(cy - half);
                let s = snap_u16(size);
                let col = if active { active_color } else { bg_color };

                quads.push(UiQuad::solid([px, py], [s, s], col));
                quads.push(UiQuad::solid([px, py], [s, border_px], border_color));
                quads.push(UiQuad::solid(
                    [px, py + s as i32 - border_px as i32],
                    [s, border_px],
                    border_color,
                ));
                quads.push(UiQuad::solid([px, py], [border_px, s], border_color));
                quads.push(UiQuad::solid(
                    [px + s as i32 - border_px as i32, py],
                    [border_px, s],
                    border_color,
                ));

                // Centered text label
                let (text_w, text_h) = font.measure_text(label);
                let tx = cx - text_w * 0.5;
                let ty = cy - text_h * 0.5;
                font.layout_text(label, tx, ty, 0xFFFFFFFF, true, gui_scale, quads);
            };

        // Jump button
        draw_btn(quads, gw - 55.0, gh - 55.0, 50.0, "JMP", self.jump_pressed);
        // Sneak button
        draw_btn(
            quads,
            gw - 105.0,
            gh - 45.0,
            40.0,
            "SNK",
            self.sneak_pressed,
        );
        // Break button
        draw_btn(
            quads,
            gw - 55.0,
            gh - 110.0,
            44.0,
            "HIT",
            self.break_pressed,
        );
        // Place button
        draw_btn(
            quads,
            gw - 105.0,
            gh - 95.0,
            40.0,
            "USE",
            self.place_pressed,
        );

        // Pause Menu Button (top-right)
        draw_btn(quads, gw - 26.0, 26.0, 36.0, "||", false);
        // Inventory Button (top-left)
        draw_btn(quads, 26.0, 26.0, 36.0, "INV", false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_joystick_vector_normalization() {
        let mut controls = TouchControls::default();
        let center = [100.0, 100.0];
        controls.left_stick_id = Some(1);
        controls.left_stick_center = center;

        // Move within deadzone
        let touch_deadzone = Touch {
            device_id: unsafe { std::mem::zeroed() },
            phase: TouchPhase::Moved,
            location: winit::dpi::PhysicalPosition::new(104.0, 100.0),
            force: None,
            id: 1,
        };
        controls.handle_touch(&touch_deadzone, 800.0, 600.0, 1);
        assert_eq!(controls.joystick_vec, [0.0, 0.0]);

        // Move beyond radius (clamped)
        let touch_far = Touch {
            device_id: unsafe { std::mem::zeroed() },
            phase: TouchPhase::Moved,
            location: winit::dpi::PhysicalPosition::new(200.0, 100.0),
            force: None,
            id: 1,
        };
        controls.handle_touch(&touch_far, 800.0, 600.0, 1);
        assert!((controls.joystick_vec[0] - 1.0).abs() < 1e-3);
        assert_eq!(controls.joystick_vec[1], 0.0);
    }

    #[test]
    fn test_apply_to_player_input() {
        let mut controls = TouchControls::default();
        controls.enabled = true;
        controls.joystick_vec = [0.0, -0.8]; // Forward
        controls.jump_pressed = true;

        let mut input = PlayerInputState::default();
        controls.apply_to_player_input(&mut input);

        assert!(input.forward);
        assert!(!input.backward);
        assert!(input.jump);
        assert!(input.sprint); // High magnitude forward triggers sprint
    }
}
