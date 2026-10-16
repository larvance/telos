//! High-fidelity ECS physics system for entities, players, and projectiles.
//!
//! Implements the physics simulation architecture from modern voxel ECS:
//! - [`PhysicsConstants`]: Gravitational acceleration (32.0 blocks/s^2), fluid and air densities.
//! - [`PhysicsValues`]: Component storing mass, drag coefficient, friction, and buoyancy properties.
//! - [`PhysicsBodyState`]: Kinematic position and velocity container.
//! - [`ForceAccumulator`]: Per-tick force summation with resistance force limit clipping.
//! - [`ForceProvider`]: Extensible trait for force sources.
//! - [`ForceProviderStandard`]: Standard physical forces (aerodynamic drag, gravity, surface friction, buoyancy).
//! - [`PhysicsBodyStateUpdaterSymplecticEuler`]: Symplectic Euler numerical integrator preserving energy.
//! - [`MovementStates`]: Locomotion state flags (idle, sprinting, crouching, sliding, mantling, swimming, etc.).
//! - [`PhysicsMath`]: Projected area and terminal velocity calculations.

use bevy_ecs::component::Component;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Global physical constants defined by the engine specification.
pub struct PhysicsConstants;

impl PhysicsConstants {
    /// Gravitational acceleration in blocks/sec^2 (32.0 m/s^2).
    pub const GRAVITY_ACCELERATION: f64 = 32.0;
    /// Ambient air density in kg/m^3.
    pub const DENSITY_AIR: f64 = 1.2;
    /// Water density in kg/m^3.
    pub const DENSITY_WATER: f64 = 998.0;
    /// Terminal falling velocity limit in blocks/s.
    pub const TERMINAL_VELOCITY: f64 = 78.4;
}

/// Physical collision flags indicating interaction categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsFlags;

impl PhysicsFlags {
    /// No collision checks performed.
    pub const NO_COLLISIONS: u8 = 0;
    /// Collide only with other dynamic entities.
    pub const ENTITY_COLLISIONS: u8 = 1;
    /// Collide only with solid voxel blocks.
    pub const BLOCK_COLLISIONS: u8 = 2;
    /// Full collision with both blocks and entities.
    pub const ALL_COLLISIONS: u8 = 3;
}

/// Kinematic state of a physics body holding position and velocity vectors.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicsBodyState {
    /// 3D position in world space coordinates.
    pub position: DVec3,
    /// 3D velocity vector in blocks per second.
    pub velocity: DVec3,
}

impl PhysicsBodyState {
    /// Creates a new `PhysicsBodyState` with given position and velocity.
    #[must_use]
    pub const fn new(position: DVec3, velocity: DVec3) -> Self {
        Self { position, velocity }
    }

    /// Resets velocity to zero.
    pub fn set_zero(&mut self) {
        self.velocity = DVec3::ZERO;
    }

    /// Returns the current scalar speed (velocity magnitude).
    #[must_use]
    pub fn speed(&self) -> f64 {
        self.velocity.length()
    }
}

/// Component defining the physical properties of an entity.
#[derive(Debug, Clone, Component)]
pub struct PhysicsValues {
    /// Mass in kilograms or normalized units (default 1.0).
    pub mass: f64,
    /// Aerodynamic drag coefficient (default 0.5).
    pub drag_coefficient: f64,
    /// Surface friction coefficient against solid terrain (default 0.6).
    pub friction_coefficient: f64,
    /// Whether gravity is inverted along the Y axis.
    pub inverted_gravity: bool,
    /// Displaced fluid mass for buoyancy simulation.
    pub displaced_mass: f64,
    /// Bounding box width (X axis extent).
    pub width: f64,
    /// Bounding box height (Y axis extent).
    pub height: f64,
    /// Bounding box depth (Z axis extent).
    pub depth: f64,
}

impl Default for PhysicsValues {
    fn default() -> Self {
        Self {
            mass: 1.0,
            drag_coefficient: 0.5,
            friction_coefficient: 0.6,
            inverted_gravity: false,
            displaced_mass: 0.0,
            width: 0.6,
            height: 1.8,
            depth: 0.6,
        }
    }
}

impl PhysicsValues {
    /// Creates a new `PhysicsValues` for a humanoid player character.
    #[must_use]
    pub fn player() -> Self {
        Self::default()
    }

    /// Sets the bounding box dimensions.
    #[must_use]
    pub const fn with_dimensions(mut self, width: f64, height: f64, depth: f64) -> Self {
        self.width = width;
        self.height = height;
        self.depth = depth;
        self
    }
}

/// Accumulates forces acting upon a body during a single simulation tick.
#[derive(Debug, Clone, Default)]
pub struct ForceAccumulator {
    /// Current scalar speed at the beginning of the tick.
    pub speed: f64,
    /// Net accumulated force vector in Newtons.
    pub force: DVec3,
    /// Maximum allowable opposing resistance force before motion reverses in `dt`.
    pub resistance_force_limit: DVec3,
}

impl ForceAccumulator {
    /// Initializes the accumulator for the given body state, mass, and time step.
    pub fn initialize(&mut self, state: &PhysicsBodyState, mass: f64, time_step: f64) {
        self.force = DVec3::ZERO;
        self.speed = state.velocity.length();
        if time_step > 0.0 {
            self.resistance_force_limit = state.velocity * (-mass / time_step);
        } else {
            self.resistance_force_limit = DVec3::ZERO;
        }
    }

    /// Clips an opposing resistance force (such as drag or friction) so it does not reverse motion.
    pub fn clip_force(force: &mut DVec3, limit: DVec3) {
        if limit.x >= 0.0 {
            force.x = force.x.clamp(0.0, limit.x);
        } else {
            force.x = force.x.clamp(limit.x, 0.0);
        }

        if limit.y >= 0.0 {
            force.y = force.y.clamp(0.0, limit.y);
        } else {
            force.y = force.y.clamp(limit.y, 0.0);
        }

        if limit.z >= 0.0 {
            force.z = force.z.clamp(0.0, limit.z);
        } else {
            force.z = force.z.clamp(limit.z, 0.0);
        }
    }
}

/// Extensible interface for physical force sources (gravity, drag, wind, explosions).
pub trait ForceProvider: Send + Sync {
    /// Contributes forces into the [`ForceAccumulator`] for the current tick.
    fn update(&self, state: &PhysicsBodyState, accumulator: &mut ForceAccumulator, on_ground: bool);
}

/// Standard configuration state for [`ForceProviderStandard`].
#[derive(Debug, Clone)]
pub struct ForceProviderStandardState {
    /// External directional forces (e.g. player movement impulse, wind).
    pub external_force: DVec3,
    /// Drag coefficient.
    pub drag_coefficient: f64,
    /// Downward gravity acceleration.
    pub gravity: f64,
    /// Surface friction coefficient.
    pub friction_coefficient: f64,
    /// Displaced fluid mass for buoyancy.
    pub displaced_mass: f64,
    /// Bounding box width.
    pub width: f64,
    /// Bounding box height.
    pub height: f64,
    /// Bounding box depth.
    pub depth: f64,
    /// Body mass.
    pub mass: f64,
}

impl Default for ForceProviderStandardState {
    fn default() -> Self {
        Self {
            external_force: DVec3::ZERO,
            drag_coefficient: 0.5,
            gravity: PhysicsConstants::GRAVITY_ACCELERATION,
            friction_coefficient: 0.6,
            displaced_mass: 0.0,
            width: 0.6,
            height: 1.8,
            depth: 0.6,
            mass: 1.0,
        }
    }
}

/// Standard physics force provider implementing aerodynamic drag, gravity, surface friction, and buoyancy.
#[derive(Debug, Clone, Default)]
pub struct ForceProviderStandard {
    /// Physical parameters and active external forces.
    pub state: ForceProviderStandardState,
}

impl ForceProviderStandard {
    /// Creates a new standard force provider with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the external driving force vector.
    pub fn set_external_force(&mut self, force: DVec3) {
        self.state.external_force = force;
    }
}

impl ForceProvider for ForceProviderStandard {
    fn update(&self, body_state: &PhysicsBodyState, accumulator: &mut ForceAccumulator, on_ground: bool) {
        let ext_force = self.state.external_force;
        let ext_force_y = ext_force.y;

        // 1. External forces (impulses, walking force)
        accumulator.force += ext_force;

        // 2. Drag force (air resistance)
        let speed = accumulator.speed;
        if speed > 1e-4 {
            let projected_area = PhysicsMath::compute_projected_area(
                body_state.velocity,
                self.state.width,
                self.state.height,
                self.state.depth,
            );
            let drag_force_div_speed = self.state.drag_coefficient * projected_area * speed;
            let mut drag_force = -body_state.velocity * drag_force_div_speed;
            ForceAccumulator::clip_force(&mut drag_force, accumulator.resistance_force_limit);
            accumulator.force += drag_force;
        }

        // 3. Gravity and surface friction
        let gravity_force = -self.state.gravity * self.state.mass;
        if on_ground {
            // Normal force on ground
            let normal_force = (gravity_force.abs() + ext_force_y.abs()).max(0.0);
            let friction_force = normal_force * self.state.friction_coefficient;
            let h_speed = (body_state.velocity.x * body_state.velocity.x
                + body_state.velocity.z * body_state.velocity.z)
                .sqrt();

            if h_speed > 1e-4 && friction_force > 0.0 {
                let mut fric = DVec3::new(
                    -body_state.velocity.x * (friction_force / h_speed),
                    0.0,
                    -body_state.velocity.z * (friction_force / h_speed),
                );
                ForceAccumulator::clip_force(&mut fric, accumulator.resistance_force_limit);
                accumulator.force += fric;
            }
        } else {
            accumulator.force.y += gravity_force;
        }

        // 4. Fluid buoyancy
        if self.state.displaced_mass > 0.0 {
            accumulator.force.y += self.state.displaced_mass * self.state.gravity;
        }
    }
}

/// Symplectic Euler numerical integrator.
///
/// Updates velocity first using net acceleration:
/// `v' = v + a * dt`
/// Then uses the updated velocity to advance position:
/// `x' = x + v' * dt`
///
/// This preserves energy in harmonic and orbital systems far better than explicit Euler.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicsBodyStateUpdaterSymplecticEuler;

impl PhysicsBodyStateUpdaterSymplecticEuler {
    /// Advances physical simulation from `before` into `after`.
    pub fn update(
        &self,
        before: &PhysicsBodyState,
        after: &mut PhysicsBodyState,
        mass: f64,
        dt: f64,
        on_ground: bool,
        force_providers: &[&dyn ForceProvider],
    ) {
        let mut accumulator = ForceAccumulator::default();
        accumulator.initialize(before, mass, dt);

        for provider in force_providers {
            provider.update(before, &mut accumulator, on_ground);
        }

        let mass = mass.max(1e-4);
        let acceleration = accumulator.force / mass;

        // v' = v + a * dt
        after.velocity = before.velocity + acceleration * dt;

        // Velocity clamping against numerical instability
        let max_speed = PhysicsConstants::TERMINAL_VELOCITY;
        if after.velocity.length() > max_speed {
            after.velocity = after.velocity.normalize() * max_speed;
        }

        // x' = x + v' * dt (Symplectic Euler: updates position after velocity)
        after.position = before.position + after.velocity * dt;
    }
}

/// Mathematics utilities for physics calculation.
pub struct PhysicsMath;

impl PhysicsMath {
    /// Computes the projected cross-sectional area of an AABB aligned with the velocity direction.
    #[must_use]
    pub fn compute_projected_area(velocity: DVec3, width: f64, height: f64, depth: f64) -> f64 {
        let speed = velocity.length();
        if speed < 1e-6 {
            return width * height;
        }
        let dir = velocity / speed;
        let mut area = 0.0;
        if dir.x != 0.0 {
            area += dir.x.abs() * depth * height;
        }
        if dir.y != 0.0 {
            area += dir.y.abs() * depth * width;
        }
        if dir.z != 0.0 {
            area += dir.z.abs() * width * height;
        }
        area
    }

    /// Computes the theoretical terminal velocity for a falling body.
    #[must_use]
    pub fn get_terminal_velocity(mass: f64, density: f64, area_m2: f64, drag_coeff: f64) -> f64 {
        let denom = density * area_m2 * drag_coeff;
        if denom <= 0.0 {
            return PhysicsConstants::TERMINAL_VELOCITY;
        }
        (2.0 * mass * PhysicsConstants::GRAVITY_ACCELERATION / denom).sqrt()
    }
}

/// Comprehensive locomotion state flags tracked per entity.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component, Serialize, Deserialize)]
pub struct MovementStates {
    /// Entity is completely stationary without input or movement.
    pub idle: bool,
    /// Entity has no horizontal movement velocity.
    pub horizontal_idle: bool,
    /// Entity is currently in an upward jump arc.
    pub jumping: bool,
    /// Entity is actively flying (creative or spectator).
    pub flying: bool,
    /// Entity is moving at standard walking speed.
    pub walking: bool,
    /// Entity is moving at running speed.
    pub running: bool,
    /// Entity is sprinting at maximum speed.
    pub sprinting: bool,
    /// Entity is crouching (reduces hitbox height).
    pub crouching: bool,
    /// Entity is forced into a crouch due to a low ceiling obstacle.
    pub forced_crouching: bool,
    /// Entity is falling through the air under gravity.
    pub falling: bool,
    /// Entity is climbing a ladder or climbable surface.
    pub climbing: bool,
    /// Entity is submerged within a fluid volume.
    pub in_fluid: bool,
    /// Entity is actively swimming through fluid.
    pub swimming: bool,
    /// Entity is leaping out of fluid.
    pub swim_jumping: bool,
    /// Entity has firm ground contact with a solid block surface.
    pub on_ground: bool,
    /// Entity is mantling over a ledge obstacle.
    pub mantling: bool,
    /// Entity is performing a high-speed crouch slide.
    pub sliding: bool,
    /// Entity is mounted on another entity or vehicle.
    pub mounting: bool,
    /// Entity is performing an evasive roll.
    pub rolling: bool,
    /// Entity is seated.
    pub sitting: bool,
    /// Entity is gliding through the air.
    pub gliding: bool,
    /// Entity is sleeping in a bed.
    pub sleeping: bool,
}

impl MovementStates {
    /// Creates a default standing-on-ground state.
    #[must_use]
    pub fn standing_on_ground() -> Self {
        Self {
            idle: true,
            horizontal_idle: true,
            on_ground: true,
            ..Default::default()
        }
    }

    /// Evaluates locomotion states based on current kinematic values and inputs.
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    pub fn update(
        &mut self,
        velocity: DVec3,
        on_ground: bool,
        flying: bool,
        in_fluid: bool,
        input_forward: bool,
        input_backward: bool,
        input_left: bool,
        input_right: bool,
        input_sprint: bool,
        input_crouch: bool,
        low_ceiling: bool,
    ) {
        self.on_ground = on_ground;
        self.flying = flying;
        self.in_fluid = in_fluid;

        let has_input = input_forward || input_backward || input_left || input_right;
        let h_speed = (velocity.x * velocity.x + velocity.z * velocity.z).sqrt();

        self.horizontal_idle = h_speed < 0.05 && !has_input;
        self.idle = self.horizontal_idle && velocity.y.abs() < 0.05;

        // Forced crouching takes precedence if headroom is restricted
        self.forced_crouching = low_ceiling;
        self.crouching = input_crouch || low_ceiling;

        // Crouch sliding: high speed + ground + crouching
        if on_ground && self.crouching && (self.sprinting || h_speed > 4.5) {
            self.sliding = true;
        } else if h_speed < 1.0 || !self.crouching || !on_ground {
            self.sliding = false;
        }

        // Locomotion categories
        if on_ground && !self.sliding {
            self.sprinting = input_sprint && has_input && !self.crouching;
            self.running = has_input && !self.sprinting && !self.crouching && h_speed > 3.0;
            self.walking = has_input && !self.sprinting && !self.running && !self.crouching;
        } else if !on_ground {
            self.sprinting = input_sprint && has_input;
        }

        // Vertical states
        self.jumping = !on_ground && velocity.y > 0.1 && !flying;
        self.falling = !on_ground && velocity.y < -0.1 && !flying;

        if in_fluid {
            self.swimming = has_input || velocity.y.abs() > 0.1;
        } else {
            self.swimming = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_body_state_initialization() {
        let mut state = PhysicsBodyState::new(DVec3::new(10.0, 64.0, 20.0), DVec3::new(3.0, 4.0, 0.0));
        assert!((state.speed() - 5.0).abs() < 1e-6);

        state.set_zero();
        assert_eq!(state.velocity, DVec3::ZERO);
        assert_eq!(state.speed(), 0.0);
    }

    #[test]
    fn test_force_accumulator_resistance_clipping() {
        let state = PhysicsBodyState::new(DVec3::ZERO, DVec3::new(10.0, 0.0, -5.0));
        let mut accumulator = ForceAccumulator::default();
        accumulator.initialize(&state, 1.0, 0.05);

        // Limit = -velocity * mass / dt = (-10 / 0.05, 0, 5 / 0.05) = (-200, 0, 100)
        assert!((accumulator.resistance_force_limit.x - -200.0).abs() < 1e-3);
        assert!((accumulator.resistance_force_limit.z - 100.0).abs() < 1e-3);

        // A resistance force exceeding the limit should be clipped
        let mut excessive_force = DVec3::new(-350.0, 0.0, 150.0);
        ForceAccumulator::clip_force(&mut excessive_force, accumulator.resistance_force_limit);
        assert!((excessive_force.x - -200.0).abs() < 1e-3);
        assert!((excessive_force.z - 100.0).abs() < 1e-3);
    }

    #[test]
    fn test_symplectic_euler_gravity_fall() {
        let updater = PhysicsBodyStateUpdaterSymplecticEuler;
        let mut before = PhysicsBodyState::new(DVec3::new(0.0, 100.0, 0.0), DVec3::ZERO);
        let mut after = before;

        let provider = ForceProviderStandard::new();
        let providers: [&dyn ForceProvider; 1] = [&provider];

        // Step 1 tick (0.05s)
        updater.update(&before, &mut after, 1.0, 0.05, false, &providers);

        // Under gravity 32.0: v' = 0 + (-32.0 * 0.05) = -1.6
        assert!((after.velocity.y - -1.6).abs() < 0.05);
        // x' = x + v' * dt = 100 + (-1.6 * 0.05) = 99.92
        assert!((after.position.y - 99.92).abs() < 0.05);

        before = after;
        updater.update(&before, &mut after, 1.0, 0.05, false, &providers);
        assert!(after.velocity.y < -1.6);
        assert!(after.position.y < 99.92);
    }

    #[test]
    fn test_projected_area_calculation() {
        let area_z = PhysicsMath::compute_projected_area(DVec3::new(0.0, 0.0, 5.0), 0.6, 1.8, 0.6);
        assert!((area_z - (0.6 * 1.8)).abs() < 1e-4);

        let area_y = PhysicsMath::compute_projected_area(DVec3::new(0.0, -10.0, 0.0), 0.6, 1.8, 0.6);
        assert!((area_y - (0.6 * 0.6)).abs() < 1e-4);
    }

    #[test]
    fn test_movement_states_transition_to_sliding() {
        let mut states = MovementStates::standing_on_ground();
        assert!(states.idle);

        // Sprinting forward on ground
        states.update(
            DVec3::new(5.6, 0.0, 0.0),
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            false,
            false,
        );
        assert!(states.sprinting);
        assert!(!states.sliding);

        // Press crouch while sprinting -> triggers crouch slide!
        states.update(
            DVec3::new(5.4, 0.0, 0.0),
            true,
            false,
            false,
            true,
            false,
            false,
            false,
            true,
            true,
            false,
        );
        assert!(states.sliding);
        assert!(states.crouching);
    }

    #[test]
    fn test_forced_crouching_under_low_ceiling() {
        let mut states = MovementStates::standing_on_ground();
        states.update(
            DVec3::ZERO,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            true, // Low ceiling detected
        );
        assert!(states.forced_crouching);
        assert!(states.crouching);
    }
}
