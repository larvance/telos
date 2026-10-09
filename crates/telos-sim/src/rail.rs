//! Rail network topology, slope curves, track routing, and minecart momentum physics.

use crate::attributes::Health;
use crate::entity::{Position, Rotation, SimulationFrozen, Velocity};
use bevy_ecs::prelude::*;
use glam::{DVec3, Vec3};
use telos_core::coords::BlockPos;

/// Maximum speed of a minecart on rails in blocks per tick (0.4 blocks/tick = 8 m/s).
pub const MINECART_MAX_SPEED: f32 = 0.40;
/// Rolling friction factor on standard rails with passenger (momentum decay per tick).
pub const MINECART_PASSENGER_RAIL_FRICTION: f32 = 0.998;
/// Rolling friction factor on standard rails when empty.
pub const MINECART_EMPTY_RAIL_FRICTION: f32 = 0.995;
/// Braking deceleration multiplier on unpowered booster rails.
pub const MINECART_UNPOWERED_BRAKE_FACTOR: f32 = 0.35;
/// Acceleration boost per tick on active powered rails.
pub const MINECART_POWERED_ACCEL: f32 = 0.06;
/// Gravity acceleration component along a 1:1 slope track.
pub const MINECART_SLOPE_GRAVITY: f32 = 0.007_812_5;

/// High-level classification of rail track types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum RailKind {
    /// Standard iron rail track (supports curves, straight lines, and slopes).
    #[default]
    Normal = 0,
    /// Gold powered booster / brake rail.
    Powered = 1,
    /// Redstone pressure switch detector rail.
    Detector = 2,
    /// Redstone passenger-ejecting activator rail.
    Activator = 3,
}

impl RailKind {
    /// Whether this rail kind supports curved track corners.
    #[must_use]
    pub const fn can_curve(self) -> bool {
        matches!(self, Self::Normal)
    }
}

/// Geometric shape and orientation of a rail block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum RailShape {
    /// Straight line connecting North (-Z) and South (+Z).
    #[default]
    NorthSouth = 0,
    /// Straight line connecting East (+X) and West (-X).
    EastWest = 1,
    /// Sloped ramp ascending toward East (+X).
    AscendingEast = 2,
    /// Sloped ramp ascending toward West (-X).
    AscendingWest = 3,
    /// Sloped ramp ascending toward North (-Z).
    AscendingNorth = 4,
    /// Sloped ramp ascending toward South (+Z).
    AscendingSouth = 5,
    /// 90-degree corner curve connecting South (+Z) and East (+X).
    SouthEast = 6,
    /// 90-degree corner curve connecting South (+Z) and West (-X).
    SouthWest = 7,
    /// 90-degree corner curve connecting North (-Z) and West (-X).
    NorthWest = 8,
    /// 90-degree corner curve connecting North (-Z) and East (+X).
    NorthEast = 9,
}

impl RailShape {
    /// Returns true if this shape represents an ascending slope ramp.
    #[must_use]
    pub const fn is_ascending(self) -> bool {
        matches!(
            self,
            Self::AscendingEast | Self::AscendingWest | Self::AscendingNorth | Self::AscendingSouth
        )
    }

    /// Returns true if this shape represents a 90-degree corner curve.
    #[must_use]
    pub const fn is_curve(self) -> bool {
        matches!(
            self,
            Self::SouthEast | Self::SouthWest | Self::NorthWest | Self::NorthEast
        )
    }

    /// Returns true if this shape represents a straight flat track.
    #[must_use]
    pub const fn is_straight(self) -> bool {
        matches!(self, Self::NorthSouth | Self::EastWest)
    }
}

/// Evaluates neighbor track topology around `pos` to determine the best-fit rail shape.
pub fn determine_rail_shape<F>(pos: BlockPos, can_curve: bool, mut has_rail: F) -> RailShape
where
    F: FnMut(BlockPos) -> bool,
{
    let has_north_up = has_rail(pos.north(1).up(1));
    let has_south_up = has_rail(pos.south(1).up(1));
    let has_east_up = has_rail(pos.east(1).up(1));
    let has_west_up = has_rail(pos.west(1).up(1));

    // 1. Check ascending slopes first
    if has_north_up {
        return RailShape::AscendingNorth;
    }
    if has_south_up {
        return RailShape::AscendingSouth;
    }
    if has_east_up {
        return RailShape::AscendingEast;
    }
    if has_west_up {
        return RailShape::AscendingWest;
    }

    // Check adjacent rails at same level or downward step
    let north = has_rail(pos.north(1)) || has_rail(pos.north(1).down(1));
    let south = has_rail(pos.south(1)) || has_rail(pos.south(1).down(1));
    let east = has_rail(pos.east(1)) || has_rail(pos.east(1).down(1));
    let west = has_rail(pos.west(1)) || has_rail(pos.west(1).down(1));

    // 2. Straight tracks
    if north && south {
        return RailShape::NorthSouth;
    }
    if east && west {
        return RailShape::EastWest;
    }

    // 3. Curves (if rail type permits)
    if can_curve {
        if south && east {
            return RailShape::SouthEast;
        }
        if south && west {
            return RailShape::SouthWest;
        }
        if north && west {
            return RailShape::NorthWest;
        }
        if north && east {
            return RailShape::NorthEast;
        }
    }

    // 4. Fallback single neighbor orientation
    if north || south {
        RailShape::NorthSouth
    } else if east || west {
        RailShape::EastWest
    } else {
        RailShape::NorthSouth
    }
}

/// Component attached to Minecart entities tracking rail routing, momentum, and passenger state.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MinecartEntity {
    /// Current track speed magnitude in blocks per tick.
    pub speed: f32,
    /// 3D momentum vector.
    pub momentum: Vec3,
    /// Whether the cart is currently snapped onto a rail track.
    pub on_rail: bool,
    /// Active track shape if currently on rail.
    pub rail_shape: RailShape,
    /// Active rail track kind if currently on rail.
    pub rail_kind: RailKind,
    /// Redstone activation state for powered or activator rails.
    pub powered: bool,
    /// World block coordinate of the rail block currently being traversed.
    pub rail_pos: Option<BlockPos>,
    /// Passenger entity net ID, if mounted.
    pub passenger_id: Option<u32>,
    /// Passenger forward throttle input in [-1.0, 1.0].
    pub forward_input: f32,
    /// Passenger turning input in [-1.0, 1.0].
    pub turn_input: f32,
    /// Visual rocking / impact timer in ticks.
    pub rock_ticks: u16,
}

impl MinecartEntity {
    /// Creates a new default unoccupied `MinecartEntity`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            speed: 0.0,
            momentum: Vec3::ZERO,
            on_rail: false,
            rail_shape: RailShape::NorthSouth,
            rail_kind: RailKind::Normal,
            powered: false,
            rail_pos: None,
            passenger_id: None,
            forward_input: 0.0,
            turn_input: 0.0,
            rock_ticks: 0,
        }
    }
}

impl Default for MinecartEntity {
    fn default() -> Self {
        Self::new()
    }
}

/// Physical simulation step for a minecart snapped onto a rail track.
#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::float_cmp)]
pub fn step_minecart_rail_physics(
    pos: &mut DVec3,
    vel: &mut Vec3,
    rot: &mut Rotation,
    cart: &mut MinecartEntity,
) {
    let bx = pos.x.floor() as i32;
    let by = pos.y.floor() as i32;
    let bz = pos.z.floor() as i32;

    let mut speed = (vel.x * vel.x + vel.z * vel.z).sqrt();

    // 1. Passenger propulsion
    if cart.passenger_id.is_some() && cart.forward_input > 0.05 && speed < 0.15 {
        speed = (speed + 0.02).min(0.15);
    }

    // 2. Track power & friction mechanics
    match cart.rail_kind {
        RailKind::Powered => {
            if cart.powered {
                speed = (speed + MINECART_POWERED_ACCEL).min(MINECART_MAX_SPEED);
                if speed < 0.01 {
                    speed = 0.05;
                }
            } else {
                speed *= MINECART_UNPOWERED_BRAKE_FACTOR;
                if speed < 0.02 {
                    speed = 0.0;
                }
            }
        }
        RailKind::Activator => {
            if cart.powered {
                // Eject passenger
                cart.passenger_id = None;
            }
            let friction = if cart.passenger_id.is_some() {
                MINECART_PASSENGER_RAIL_FRICTION
            } else {
                MINECART_EMPTY_RAIL_FRICTION
            };
            speed *= friction;
        }
        RailKind::Normal | RailKind::Detector => {
            let friction = if cart.passenger_id.is_some() {
                MINECART_PASSENGER_RAIL_FRICTION
            } else {
                MINECART_EMPTY_RAIL_FRICTION
            };
            speed *= friction;
        }
    }

    // 3. Trajectory snapping & slope/curve momentum integration
    match cart.rail_shape {
        RailShape::NorthSouth => {
            pos.x = f64::from(bx) + 0.5;
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.z > 0.0 || (vel.z == 0.0 && (rot.yaw.abs() < 45.0 || rot.yaw > 315.0)) {
                vel.z = speed;
                vel.x = 0.0;
                rot.yaw = 0.0;
            } else {
                vel.z = -speed;
                vel.x = 0.0;
                rot.yaw = 180.0;
            }
            pos.z += f64::from(vel.z);
        }
        RailShape::EastWest => {
            pos.z = f64::from(bz) + 0.5;
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.x > 0.0 || (vel.x == 0.0 && rot.yaw >= 225.0 && rot.yaw <= 315.0) {
                vel.x = speed;
                vel.z = 0.0;
                rot.yaw = 270.0;
            } else {
                vel.x = -speed;
                vel.z = 0.0;
                rot.yaw = 90.0;
            }
            pos.x += f64::from(vel.x);
        }
        RailShape::AscendingEast => {
            pos.z = f64::from(bz) + 0.5;
            vel.x -= MINECART_SLOPE_GRAVITY;
            let moving_east = vel.x >= 0.0;
            let track_speed = vel.x.abs().min(MINECART_MAX_SPEED);
            vel.x = if moving_east {
                track_speed
            } else {
                -track_speed
            };
            vel.z = 0.0;
            rot.yaw = if moving_east { 270.0 } else { 90.0 };
            rot.pitch = if moving_east { 35.0 } else { -35.0 };
            pos.x += f64::from(vel.x);
            let frac = (pos.x - f64::from(bx)).clamp(0.0, 1.0);
            pos.y = f64::from(by) + frac + 0.0625;
        }
        RailShape::AscendingWest => {
            pos.z = f64::from(bz) + 0.5;
            vel.x += MINECART_SLOPE_GRAVITY;
            let moving_west = vel.x <= 0.0;
            let track_speed = vel.x.abs().min(MINECART_MAX_SPEED);
            vel.x = if moving_west {
                -track_speed
            } else {
                track_speed
            };
            vel.z = 0.0;
            rot.yaw = if moving_west { 90.0 } else { 270.0 };
            rot.pitch = if moving_west { 35.0 } else { -35.0 };
            pos.x += f64::from(vel.x);
            let frac = 1.0 - (pos.x - f64::from(bx)).clamp(0.0, 1.0);
            pos.y = f64::from(by) + frac + 0.0625;
        }
        RailShape::AscendingNorth => {
            pos.x = f64::from(bx) + 0.5;
            vel.z += MINECART_SLOPE_GRAVITY;
            let moving_north = vel.z <= 0.0;
            let track_speed = vel.z.abs().min(MINECART_MAX_SPEED);
            vel.z = if moving_north {
                -track_speed
            } else {
                track_speed
            };
            vel.x = 0.0;
            rot.yaw = if moving_north { 180.0 } else { 0.0 };
            rot.pitch = if moving_north { 35.0 } else { -35.0 };
            pos.z += f64::from(vel.z);
            let frac = 1.0 - (pos.z - f64::from(bz)).clamp(0.0, 1.0);
            pos.y = f64::from(by) + frac + 0.0625;
        }
        RailShape::AscendingSouth => {
            pos.x = f64::from(bx) + 0.5;
            vel.z -= MINECART_SLOPE_GRAVITY;
            let moving_south = vel.z >= 0.0;
            let track_speed = vel.z.abs().min(MINECART_MAX_SPEED);
            vel.z = if moving_south {
                track_speed
            } else {
                -track_speed
            };
            vel.x = 0.0;
            rot.yaw = if moving_south { 0.0 } else { 180.0 };
            rot.pitch = if moving_south { 35.0 } else { -35.0 };
            pos.z += f64::from(vel.z);
            let frac = (pos.z - f64::from(bz)).clamp(0.0, 1.0);
            pos.y = f64::from(by) + frac + 0.0625;
        }
        RailShape::SouthEast => {
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.z < 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 180.0) {
                vel.x = speed;
                vel.z = 0.0;
                rot.yaw = 270.0;
            } else if vel.x < 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 90.0) {
                vel.x = 0.0;
                vel.z = speed;
                rot.yaw = 0.0;
            }
            pos.x += f64::from(vel.x);
            pos.z += f64::from(vel.z);
        }
        RailShape::SouthWest => {
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.z < 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 180.0) {
                vel.x = -speed;
                vel.z = 0.0;
                rot.yaw = 90.0;
            } else if vel.x > 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 270.0) {
                vel.x = 0.0;
                vel.z = speed;
                rot.yaw = 0.0;
            }
            pos.x += f64::from(vel.x);
            pos.z += f64::from(vel.z);
        }
        RailShape::NorthWest => {
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.z > 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 0.0) {
                vel.x = -speed;
                vel.z = 0.0;
                rot.yaw = 90.0;
            } else if vel.x > 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 270.0) {
                vel.x = 0.0;
                vel.z = -speed;
                rot.yaw = 180.0;
            }
            pos.x += f64::from(vel.x);
            pos.z += f64::from(vel.z);
        }
        RailShape::NorthEast => {
            pos.y = f64::from(by) + 0.0625;
            rot.pitch = 0.0;
            if vel.z > 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 0.0) {
                vel.x = speed;
                vel.z = 0.0;
                rot.yaw = 270.0;
            } else if vel.x < 0.0 || (vel.x == 0.0 && vel.z == 0.0 && rot.yaw == 90.0) {
                vel.x = 0.0;
                vel.z = -speed;
                rot.yaw = 180.0;
            }
            pos.x += f64::from(vel.x);
            pos.z += f64::from(vel.z);
        }
    }

    cart.on_rail = true;
    cart.speed = (vel.x * vel.x + vel.z * vel.z).sqrt();
    cart.momentum = *vel;
    if cart.rock_ticks > 0 {
        cart.rock_ticks -= 1;
    }
}

/// Physical simulation step for a derailed minecart falling or sliding off-track.
pub fn step_minecart_offrail_physics(
    pos: &mut DVec3,
    vel: &mut Vec3,
    rot: &mut Rotation,
    cart: &mut MinecartEntity,
) {
    pos.x += f64::from(vel.x);
    pos.y += f64::from(vel.y);
    pos.z += f64::from(vel.z);

    vel.y = (vel.y - 0.08).max(-2.0);
    vel.x *= 0.60;
    vel.z *= 0.60;

    rot.pitch = 0.0;
    cart.on_rail = false;
    cart.speed = (vel.x * vel.x + vel.z * vel.z).sqrt();
    cart.momentum = *vel;
    if cart.rock_ticks > 0 {
        cart.rock_ticks -= 1;
    }
}

/// Movement, track constraint, and momentum physics system for Minecart entities.
pub fn minecart_movement_system(
    mut query: Query<
        (
            &mut Position,
            &mut Velocity,
            &mut Rotation,
            &mut MinecartEntity,
            &Health,
        ),
        Without<SimulationFrozen>,
    >,
) {
    for (mut pos, mut vel, mut rot, mut cart, _health) in &mut query {
        if cart.on_rail {
            step_minecart_rail_physics(&mut pos.0, &mut vel.0, &mut rot, &mut cart);
        } else {
            step_minecart_offrail_physics(&mut pos.0, &mut vel.0, &mut rot, &mut cart);
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::similar_names)]
mod tests {
    use super::*;

    #[test]
    fn test_rail_shape_predicates() {
        assert!(RailShape::NorthSouth.is_straight());
        assert!(RailShape::EastWest.is_straight());
        assert!(!RailShape::NorthSouth.is_curve());
        assert!(!RailShape::NorthSouth.is_ascending());

        assert!(RailShape::AscendingNorth.is_ascending());
        assert!(RailShape::AscendingEast.is_ascending());
        assert!(!RailShape::AscendingNorth.is_curve());

        assert!(RailShape::SouthEast.is_curve());
        assert!(RailShape::NorthWest.is_curve());
        assert!(!RailShape::SouthEast.is_straight());
    }

    #[test]
    fn test_determine_rail_shape_straight_and_curves() {
        let center = BlockPos::new(10, 64, 10);

        // 1. North and South neighbors -> NorthSouth
        let shape_ns = determine_rail_shape(center, true, |p| {
            p == center.north(1) || p == center.south(1)
        });
        assert_eq!(shape_ns, RailShape::NorthSouth);

        // 2. East and West neighbors -> EastWest
        let shape_ew =
            determine_rail_shape(center, true, |p| p == center.east(1) || p == center.west(1));
        assert_eq!(shape_ew, RailShape::EastWest);

        // 3. South and East neighbors -> SouthEast
        let shape_se = determine_rail_shape(center, true, |p| {
            p == center.south(1) || p == center.east(1)
        });
        assert_eq!(shape_se, RailShape::SouthEast);

        // 4. North and West neighbors -> NorthWest
        let shape_nw = determine_rail_shape(center, true, |p| {
            p == center.north(1) || p == center.west(1)
        });
        assert_eq!(shape_nw, RailShape::NorthWest);
    }

    #[test]
    fn test_determine_rail_shape_ascending_slopes() {
        let center = BlockPos::new(10, 64, 10);

        // North neighbor 1 block up -> AscendingNorth
        let shape_an = determine_rail_shape(center, true, |p| p == center.north(1).up(1));
        assert_eq!(shape_an, RailShape::AscendingNorth);

        // East neighbor 1 block up -> AscendingEast
        let shape_ae = determine_rail_shape(center, true, |p| p == center.east(1).up(1));
        assert_eq!(shape_ae, RailShape::AscendingEast);
    }

    #[test]
    fn test_minecart_straight_rail_physics() {
        let mut pos = DVec3::new(10.2, 64.0, 10.0);
        let mut vel = Vec3::new(0.0, 0.0, 0.2);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = true;
        cart.rail_shape = RailShape::NorthSouth;

        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);

        // Snapped to X center 10.5
        assert_eq!(pos.x, 10.5);
        assert_eq!(pos.y, 64.0625);
        // Moved along Z
        assert!(pos.z > 10.0);
        // Low rail friction applied (empty cart: 0.995)
        assert!((cart.speed - 0.2 * 0.995).abs() < 1e-4);
        assert_eq!(rot.yaw, 0.0);
    }

    #[test]
    fn test_minecart_curve_turning_and_momentum_conservation() {
        let mut pos = DVec3::new(10.0, 64.0, 10.5);
        // Moving north (-Z) into a SouthEast curve
        let mut vel = Vec3::new(0.0, 0.0, -0.3);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = true;
        cart.rail_shape = RailShape::SouthEast;

        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);

        // Momentum smoothly turned from -Z to +X
        assert!(vel.x > 0.29);
        assert_eq!(vel.z, 0.0);
        assert_eq!(rot.yaw, 270.0);
        assert!((cart.speed - 0.3 * 0.995).abs() < 1e-3);
    }

    #[test]
    fn test_minecart_powered_rail_acceleration_and_brake() {
        let mut pos = DVec3::new(10.5, 64.0, 10.0);
        let mut vel = Vec3::new(0.0, 0.0, 0.1);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = true;
        cart.rail_shape = RailShape::NorthSouth;
        cart.rail_kind = RailKind::Powered;
        cart.powered = true;

        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);
        // Accelerated by 0.06
        assert!((cart.speed - 0.16).abs() < 1e-4);

        // Unpowered brake test
        cart.powered = false;
        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);
        // Strongly braked
        assert!(cart.speed < 0.16 * 0.4);
    }

    #[test]
    fn test_minecart_slope_physics_and_elevation() {
        let mut pos = DVec3::new(10.0, 64.0, 10.5);
        let mut vel = Vec3::new(0.2, 0.0, 0.0);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = true;
        cart.rail_shape = RailShape::AscendingEast;

        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);
        // Y rises with X advancement
        assert!(pos.y > 64.0625);
        assert_eq!(rot.pitch, 35.0);
        // Gravity slowed it down slightly climbing uphill
        assert!(vel.x < 0.2);
    }

    #[test]
    fn test_minecart_offrail_physics() {
        let mut pos = DVec3::new(10.0, 64.0, 10.0);
        let mut vel = Vec3::new(0.2, 0.0, 0.2);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = false;

        step_minecart_offrail_physics(&mut pos, &mut vel, &mut rot, &mut cart);
        // Standard gravity
        assert_eq!(vel.y, -0.08);
        // Ground friction 0.6
        assert!((vel.x - 0.12).abs() < 1e-4);
        assert!(!cart.on_rail);
    }

    #[test]
    fn test_minecart_activator_rail_ejects_passenger() {
        let mut pos = DVec3::new(10.5, 64.0, 10.0);
        let mut vel = Vec3::new(0.0, 0.0, 0.2);
        let mut rot = Rotation::default();
        let mut cart = MinecartEntity::new();
        cart.on_rail = true;
        cart.rail_shape = RailShape::NorthSouth;
        cart.rail_kind = RailKind::Activator;
        cart.powered = true;
        cart.passenger_id = Some(42);

        step_minecart_rail_physics(&mut pos, &mut vel, &mut rot, &mut cart);
        assert_eq!(cart.passenger_id, None);
    }
}
