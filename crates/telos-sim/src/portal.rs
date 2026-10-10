//! Nether portal frame detection, activation topology, dimensional coordinate translation,
//! and contiguous portal deactivation.

use glam::DVec3;
use hashbrown::HashSet;
use std::collections::VecDeque;
use telos_core::coords::BlockPos;

/// Horizontal orientation axis of a vertical Nether portal plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PortalAxis {
    /// Portal plane lies along the X-axis (faces North/South, width spans X).
    #[default]
    X,
    /// Portal plane lies along the Z-axis (faces East/West, width spans Z).
    Z,
}

/// Bounding structure of a validated rectangular Nether portal frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalFrame {
    /// Orientation axis of the portal.
    pub axis: PortalAxis,
    /// Minimum interior world coordinates (bottom-left corner of the portal interior).
    pub bottom_left: BlockPos,
    /// Interior width in blocks ($2 \le \text{width} \le 21$).
    pub width: i32,
    /// Interior height in blocks ($3 \le \text{height} \le 21$).
    pub height: i32,
    /// All interior block positions that must be filled with `nether_portal`.
    pub interior_blocks: Vec<BlockPos>,
}

/// Ratio of horizontal coordinate scaling between the Overworld and the Nether (8:1).
pub const OVERWORLD_TO_NETHER_RATIO: f64 = 8.0;

/// Translates an Overworld world-space coordinate into the corresponding Nether coordinate.
#[must_use]
pub fn overworld_to_nether_coords(pos: DVec3) -> DVec3 {
    DVec3::new(
        pos.x / OVERWORLD_TO_NETHER_RATIO,
        pos.y.clamp(32.0, 118.0),
        pos.z / OVERWORLD_TO_NETHER_RATIO,
    )
}

/// Translates a Nether world-space coordinate into the corresponding Overworld coordinate.
#[must_use]
pub fn nether_to_overworld_coords(pos: DVec3) -> DVec3 {
    DVec3::new(
        pos.x * OVERWORLD_TO_NETHER_RATIO,
        pos.y.clamp(4.0, 250.0),
        pos.z * OVERWORLD_TO_NETHER_RATIO,
    )
}

/// Classification of a block position during Nether portal frame detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBlockKind {
    /// Obsidian frame boundary block.
    Obsidian,
    /// Air or fire interior cell.
    AirOrFire,
    /// Any other solid or obstructing block.
    Other,
}

/// Attempts to detect a valid enclosing rectangular obsidian frame from an ignition point.
///
/// Scans both the X and Z axes. The frame must satisfy:
/// - Interior width between 2 and 21 blocks inclusive.
/// - Interior height between 3 and 21 blocks inclusive.
/// - Complete bottom sill of obsidian directly beneath the interior floor.
/// - Complete top lintel of obsidian directly above the interior ceiling.
/// - Complete vertical obsidian jambs on both lateral sides.
/// - All interior cells must satisfy `FrameBlockKind::AirOrFire`.
pub fn detect_portal_frame<F>(ignition_pos: BlockPos, mut check_cell: F) -> Option<PortalFrame>
where
    F: FnMut(BlockPos) -> FrameBlockKind,
{
    // Try X-axis orientation first, then Z-axis
    if let Some(frame) = try_detect_axis(ignition_pos, PortalAxis::X, &mut check_cell) {
        return Some(frame);
    }
    try_detect_axis(ignition_pos, PortalAxis::Z, &mut check_cell)
}

#[allow(clippy::too_many_lines)]
fn try_detect_axis<F>(
    ignition_pos: BlockPos,
    axis: PortalAxis,
    check_cell: &mut F,
) -> Option<PortalFrame>
where
    F: FnMut(BlockPos) -> FrameBlockKind,
{
    // 1. Scan downwards to find the bottom obsidian sill
    let mut cur_y = ignition_pos.y();
    let max_y_drop = 22;
    let mut steps_down = 0;
    while steps_down < max_y_drop {
        let p = BlockPos::new(ignition_pos.x(), cur_y, ignition_pos.z());
        match check_cell(p) {
            FrameBlockKind::Obsidian => {
                cur_y += 1;
                break;
            }
            FrameBlockKind::AirOrFire => {}
            FrameBlockKind::Other => return None,
        }
        cur_y -= 1;
        steps_down += 1;
    }

    let floor_y = cur_y;

    // 2. Scan in negative lateral direction to find the left obsidian jamb
    let (mut lateral_neg, lateral_step) = match axis {
        PortalAxis::X => (ignition_pos.x(), 1),
        PortalAxis::Z => (ignition_pos.z(), 1),
    };

    let mut steps_left = 0;
    while steps_left < 22 {
        let test_pos = match axis {
            PortalAxis::X => BlockPos::new(lateral_neg, floor_y, ignition_pos.z()),
            PortalAxis::Z => BlockPos::new(ignition_pos.x(), floor_y, lateral_neg),
        };
        let kind = check_cell(test_pos);
        if kind == FrameBlockKind::Obsidian {
            lateral_neg += lateral_step;
            break;
        }
        if kind != FrameBlockKind::AirOrFire
            || check_cell(test_pos.down(1)) != FrameBlockKind::Obsidian
        {
            return None;
        }
        lateral_neg -= lateral_step;
        steps_left += 1;
    }

    let min_lateral = lateral_neg;

    // 3. Scan in positive lateral direction to find the right obsidian jamb and measure width
    let mut width = 0;
    let max_lateral_span = 22;
    while width < max_lateral_span {
        let test_pos = match axis {
            PortalAxis::X => BlockPos::new(min_lateral + width, floor_y, ignition_pos.z()),
            PortalAxis::Z => BlockPos::new(ignition_pos.x(), floor_y, min_lateral + width),
        };

        let kind = check_cell(test_pos);
        if kind == FrameBlockKind::Obsidian {
            // Reached right jamb
            break;
        }

        // Interior floor must be air/fire and must have obsidian underneath
        if kind != FrameBlockKind::AirOrFire
            || check_cell(test_pos.down(1)) != FrameBlockKind::Obsidian
        {
            return None;
        }

        width += 1;
    }

    if !(2..=21).contains(&width) {
        return None;
    }

    // 4. Measure interior height and verify side jambs and top lintel
    let mut height = 0;
    let max_height_span = 22;
    'height_loop: while height < max_height_span {
        let current_y = floor_y + height;

        // Check left jamb
        let left_jamb = match axis {
            PortalAxis::X => BlockPos::new(min_lateral - 1, current_y, ignition_pos.z()),
            PortalAxis::Z => BlockPos::new(ignition_pos.x(), current_y, min_lateral - 1),
        };
        // Check right jamb
        let right_jamb = match axis {
            PortalAxis::X => BlockPos::new(min_lateral + width, current_y, ignition_pos.z()),
            PortalAxis::Z => BlockPos::new(ignition_pos.x(), current_y, min_lateral + width),
        };

        // Check if we hit top lintel across the entire width
        let mut top_obsidian_count = 0;
        for dx in 0..width {
            let p = match axis {
                PortalAxis::X => BlockPos::new(min_lateral + dx, current_y, ignition_pos.z()),
                PortalAxis::Z => BlockPos::new(ignition_pos.x(), current_y, min_lateral + dx),
            };
            if check_cell(p) == FrameBlockKind::Obsidian {
                top_obsidian_count += 1;
            }
        }

        if top_obsidian_count == width {
            // All top blocks are obsidian: top lintel found!
            break 'height_loop;
        }

        if top_obsidian_count > 0 {
            // Partial obsidian row: invalid frame
            return None;
        }

        if check_cell(left_jamb) != FrameBlockKind::Obsidian
            || check_cell(right_jamb) != FrameBlockKind::Obsidian
        {
            return None;
        }

        // Verify interior row is air/fire
        for dx in 0..width {
            let p = match axis {
                PortalAxis::X => BlockPos::new(min_lateral + dx, current_y, ignition_pos.z()),
                PortalAxis::Z => BlockPos::new(ignition_pos.x(), current_y, min_lateral + dx),
            };
            if check_cell(p) != FrameBlockKind::AirOrFire {
                return None;
            }
        }

        height += 1;
    }

    if !(3..=21).contains(&height) {
        return None;
    }

    // Collect all interior block positions
    let bottom_left = match axis {
        PortalAxis::X => BlockPos::new(min_lateral, floor_y, ignition_pos.z()),
        PortalAxis::Z => BlockPos::new(ignition_pos.x(), floor_y, min_lateral),
    };

    let mut interior_blocks = Vec::with_capacity((width * height) as usize);
    for dy in 0..height {
        for dw in 0..width {
            let p = match axis {
                PortalAxis::X => BlockPos::new(min_lateral + dw, floor_y + dy, ignition_pos.z()),
                PortalAxis::Z => BlockPos::new(ignition_pos.x(), floor_y + dy, min_lateral + dw),
            };
            interior_blocks.push(p);
        }
    }

    Some(PortalFrame {
        axis,
        bottom_left,
        width,
        height,
        interior_blocks,
    })
}

/// Breadth-first search finding all contiguous `nether_portal` blocks connected to `start_pos`.
pub fn find_contiguous_portal_blocks<FIsPortal>(
    start_pos: BlockPos,
    mut is_portal: FIsPortal,
) -> Vec<BlockPos>
where
    FIsPortal: FnMut(BlockPos) -> bool,
{
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    if is_portal(start_pos) {
        visited.insert(start_pos);
        queue.push_back(start_pos);
    }

    // Check immediate neighbors of start_pos in case start_pos was a broken frame block
    for neighbor in [
        start_pos.up(1),
        start_pos.down(1),
        start_pos.north(1),
        start_pos.south(1),
        start_pos.east(1),
        start_pos.west(1),
    ] {
        if is_portal(neighbor) && visited.insert(neighbor) {
            queue.push_back(neighbor);
        }
    }

    while let Some(current) = queue.pop_front() {
        result.push(current);

        for next in [
            current.up(1),
            current.down(1),
            current.north(1),
            current.south(1),
            current.east(1),
            current.west(1),
        ] {
            if is_portal(next) && visited.insert(next) {
                queue.push_back(next);
            }
        }
    }

    result
}

/// Generates block coordinates for constructing a standard 4x5 obsidian Nether portal structure.
///
/// Returns `(frame_obsidian_blocks, interior_portal_blocks, landing_platform_blocks)`.
pub fn generate_portal_structure_blocks(
    bottom_left_interior: BlockPos,
    axis: PortalAxis,
) -> (Vec<BlockPos>, Vec<BlockPos>, Vec<BlockPos>) {
    let mut frame = Vec::with_capacity(14);
    let mut interior = Vec::with_capacity(6);
    let mut platform = Vec::with_capacity(16);

    let bx = bottom_left_interior.x();
    let by = bottom_left_interior.y();
    let bz = bottom_left_interior.z();

    // Interior: 2 wide (0..2) x 3 high (0..3)
    for dy in 0..3 {
        for dw in 0..2 {
            let p = match axis {
                PortalAxis::X => BlockPos::new(bx + dw, by + dy, bz),
                PortalAxis::Z => BlockPos::new(bx, by + dy, bz + dw),
            };
            interior.push(p);
        }
    }

    // Frame: 4 wide (-1..=2) x 5 high (-1..=3)
    // Bottom sill and Top lintel
    for dw in -1..=2 {
        let bottom = match axis {
            PortalAxis::X => BlockPos::new(bx + dw, by - 1, bz),
            PortalAxis::Z => BlockPos::new(bx, by - 1, bz + dw),
        };
        let top = match axis {
            PortalAxis::X => BlockPos::new(bx + dw, by + 3, bz),
            PortalAxis::Z => BlockPos::new(bx, by + 3, bz + dw),
        };
        frame.push(bottom);
        frame.push(top);
    }

    // Side jambs (dy = 0..3)
    for dy in 0..3 {
        let left = match axis {
            PortalAxis::X => BlockPos::new(bx - 1, by + dy, bz),
            PortalAxis::Z => BlockPos::new(bx, by + dy, bz - 1),
        };
        let right = match axis {
            PortalAxis::X => BlockPos::new(bx + 2, by + dy, bz),
            PortalAxis::Z => BlockPos::new(bx, by + dy, bz + 2),
        };
        frame.push(left);
        frame.push(right);
    }

    // Landing platform 4x4 beneath portal floor to ensure safe stepping
    for dw in -1..=2 {
        for dz in -1..=1 {
            let p = match axis {
                PortalAxis::X => BlockPos::new(bx + dw, by - 1, bz + dz),
                PortalAxis::Z => BlockPos::new(bx + dz, by - 1, bz + dw),
            };
            platform.push(p);
        }
    }

    (frame, interior, platform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_coordinate_scaling_roundtrip() {
        let overworld_pos = DVec3::new(800.0, 64.0, -1600.0);
        let nether_pos = overworld_to_nether_coords(overworld_pos);

        assert_eq!(nether_pos.x, 100.0);
        assert_eq!(nether_pos.y, 64.0);
        assert_eq!(nether_pos.z, -200.0);

        let returned_overworld = nether_to_overworld_coords(nether_pos);
        assert_eq!(returned_overworld.x, 800.0);
        assert_eq!(returned_overworld.y, 64.0);
        assert_eq!(returned_overworld.z, -1600.0);

        // Clamping bounds
        let high_pos = DVec3::new(0.0, 300.0, 0.0);
        assert_eq!(overworld_to_nether_coords(high_pos).y, 118.0);

        let low_pos = DVec3::new(0.0, 2.0, 0.0);
        assert_eq!(overworld_to_nether_coords(low_pos).y, 32.0);
    }

    #[test]
    fn test_standard_4x5_portal_frame_detection_x_axis() {
        // Construct a standard 4x5 obsidian frame along X axis:
        // Inside: x = [10, 11], y = [64, 65, 66], z = 20
        let mut obs_set = HashSet::new();

        // Bottom sill: y = 63, x = 9..=12
        for x in 9..=12 {
            obs_set.insert(BlockPos::new(x, 63, 20));
        }
        // Top lintel: y = 67, x = 9..=12
        for x in 9..=12 {
            obs_set.insert(BlockPos::new(x, 67, 20));
        }
        // Left jamb: x = 9, y = 64..=66
        for y in 64..=66 {
            obs_set.insert(BlockPos::new(9, y, 20));
        }
        // Right jamb: x = 12, y = 64..=66
        for y in 64..=66 {
            obs_set.insert(BlockPos::new(12, y, 20));
        }

        let check_cell = |p: BlockPos| {
            if obs_set.contains(&p) {
                FrameBlockKind::Obsidian
            } else {
                FrameBlockKind::AirOrFire
            }
        };

        // Ignite inside at (10, 64, 20)
        let frame = detect_portal_frame(BlockPos::new(10, 64, 20), check_cell)
            .expect("frame should be detected");

        assert_eq!(frame.axis, PortalAxis::X);
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 3);
        assert_eq!(frame.bottom_left, BlockPos::new(10, 64, 20));
        assert_eq!(frame.interior_blocks.len(), 6);
    }

    #[test]
    fn test_standard_4x5_portal_frame_detection_z_axis() {
        // Construct a standard 4x5 obsidian frame along Z axis:
        // Inside: z = [30, 31], y = [70, 71, 72], x = 15
        let mut obs_set = HashSet::new();

        for z in 29..=32 {
            obs_set.insert(BlockPos::new(15, 69, z));
            obs_set.insert(BlockPos::new(15, 73, z));
        }
        for y in 70..=72 {
            obs_set.insert(BlockPos::new(15, y, 29));
            obs_set.insert(BlockPos::new(15, y, 32));
        }

        let check_cell = |p: BlockPos| {
            if obs_set.contains(&p) {
                FrameBlockKind::Obsidian
            } else {
                FrameBlockKind::AirOrFire
            }
        };

        let frame = detect_portal_frame(BlockPos::new(15, 70, 31), check_cell)
            .expect("Z-axis frame should be detected");

        assert_eq!(frame.axis, PortalAxis::Z);
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 3);
        assert_eq!(frame.bottom_left, BlockPos::new(15, 70, 30));
        assert_eq!(frame.interior_blocks.len(), 6);
    }

    #[test]
    fn test_broken_frame_rejected() {
        // Frame missing top lintel
        let mut obs_set = HashSet::new();
        for x in 9..=12 {
            obs_set.insert(BlockPos::new(x, 63, 20));
        }
        for y in 64..=66 {
            obs_set.insert(BlockPos::new(9, y, 20));
            obs_set.insert(BlockPos::new(12, y, 20));
        }

        let check_cell = |p: BlockPos| {
            if obs_set.contains(&p) {
                FrameBlockKind::Obsidian
            } else {
                FrameBlockKind::AirOrFire
            }
        };

        assert!(detect_portal_frame(BlockPos::new(10, 64, 20), check_cell).is_none());
    }

    #[test]
    fn test_find_contiguous_portal_blocks() {
        let mut portal_blocks = HashSet::new();
        portal_blocks.insert(BlockPos::new(10, 64, 20));
        portal_blocks.insert(BlockPos::new(11, 64, 20));
        portal_blocks.insert(BlockPos::new(10, 65, 20));
        portal_blocks.insert(BlockPos::new(11, 65, 20));
        portal_blocks.insert(BlockPos::new(10, 66, 20));
        portal_blocks.insert(BlockPos::new(11, 66, 20));

        let is_portal = |p: BlockPos| portal_blocks.contains(&p);

        let connected = find_contiguous_portal_blocks(BlockPos::new(10, 64, 20), is_portal);
        assert_eq!(connected.len(), 6);

        // Broken frame block neighbor triggers full sweep
        let connected_from_neighbor =
            find_contiguous_portal_blocks(BlockPos::new(9, 64, 20), is_portal);
        assert_eq!(connected_from_neighbor.len(), 6);
    }
}
