//! Server-side downsampling from level $L-1$ to level $L$ (2×2×2 → 1 voxel).

use telos_voxel::state::BlockStateId;

use crate::coords::LodNodeKey;
use crate::node::{LodNode, LodVoxelSource};

/// Downsamples 8 child octants of level $L-1$ into a single level $L$ `LodNode`.
///
/// Children are provided as an array of 8 optional references indexed by octant:
/// `octant = ox | (oy << 1) | (oz << 2)`.
/// Missing child nodes are treated as empty air.
#[must_use]
#[allow(clippy::similar_names)]
pub fn downsample_octants<S: LodVoxelSource>(
    parent_key: LodNodeKey,
    children: &[Option<&S>; 8],
) -> LodNode {
    let mut node = LodNode::empty(parent_key);

    for (octant, child_opt) in children.iter().enumerate() {
        let Some(child) = child_opt else {
            continue;
        };
        if child.is_empty() {
            continue;
        }

        let ox = (octant & 1) as u32;
        let oy = ((octant >> 1) & 1) as u32;
        let oz = ((octant >> 2) & 1) as u32;

        let base_vx = ox * 16;
        let base_vy = oy * 16;
        let base_vz = oz * 16;

        for lz in 0..16u32 {
            let vz = base_vz + lz;
            let cz = lz * 2;
            for ly in 0..16u32 {
                let vy = base_vy + ly;
                let cy = ly * 2;
                for lx in 0..16u32 {
                    let vx = base_vx + lx;
                    let cx = lx * 2;

                    let state = vote_representative(*child, cx, cy, cz);
                    if !state.is_air() {
                        node.set_state(vx, vy, vz, state);
                    }
                }
            }
        }
    }

    node
}

#[derive(Clone, Copy)]
struct StateVote {
    state: BlockStateId,
    weight: f32,
    count: u8,
}

/// Evaluates 8 child voxels in a 2×2×2 cube and votes for the representative `BlockStateId`.
#[inline]
#[allow(clippy::float_cmp)]
fn vote_representative<S: LodVoxelSource>(child: &S, cx: u32, cy: u32, cz: u32) -> BlockStateId {
    let mut votes = [StateVote {
        state: BlockStateId::AIR,
        weight: 0.0,
        count: 0,
    }; 8];
    let mut vote_len = 0usize;

    for dz in 0..=1u32 {
        for dy in 0..=1u32 {
            for dx in 0..=1u32 {
                let x = cx + dx;
                let y = cy + dy;
                let z = cz + dz;

                if !child.is_solid(x, y, z) {
                    continue;
                }

                let state = child.get_state(x, y, z);

                // Evaluate directional exposures to air
                let mut voxel_weight = 0.0f32;

                // Up (+Y)
                if y + 1 >= 32 || !child.is_solid(x, y + 1, z) {
                    voxel_weight += 4.0;
                }
                // Down (-Y)
                if y == 0 || !child.is_solid(x, y - 1, z) {
                    voxel_weight += 0.25;
                }
                // Sides (+X, -X, +Z, -Z)
                if x + 1 >= 32 || !child.is_solid(x + 1, y, z) {
                    voxel_weight += 1.0;
                }
                if x == 0 || !child.is_solid(x - 1, y, z) {
                    voxel_weight += 1.0;
                }
                if z + 1 >= 32 || !child.is_solid(x, y, z + 1) {
                    voxel_weight += 1.0;
                }
                if z == 0 || !child.is_solid(x, y, z - 1) {
                    voxel_weight += 1.0;
                }

                // Insert or update state in stack-allocated votes
                let mut found = false;
                for vote in &mut votes[..vote_len] {
                    if vote.state == state {
                        vote.count += 1;
                        vote.weight += voxel_weight;
                        found = true;
                        break;
                    }
                }
                if !found {
                    votes[vote_len] = StateVote {
                        state,
                        weight: voxel_weight,
                        count: 1,
                    };
                    vote_len += 1;
                }
            }
        }
    }

    if vote_len == 0 {
        return BlockStateId::AIR;
    }

    // 1. Pick maximum weighted exposed state (if any has weight > 0)
    let mut best_weighted: Option<(BlockStateId, f32)> = None;
    let mut best_count: Option<(BlockStateId, u8)> = None;

    for vote in &votes[..vote_len] {
        if vote.weight > 0.0 {
            match best_weighted {
                None => best_weighted = Some((vote.state, vote.weight)),
                Some((best_s, best_w)) => {
                    if vote.weight > best_w || (vote.weight == best_w && vote.state.0 < best_s.0) {
                        best_weighted = Some((vote.state, vote.weight));
                    }
                }
            }
        }
        match best_count {
            None => best_count = Some((vote.state, vote.count)),
            Some((best_s, best_c)) => {
                if vote.count > best_c || (vote.count == best_c && vote.state.0 < best_s.0) {
                    best_count = Some((vote.state, vote.count));
                }
            }
        }
    }

    if let Some((best_s, _)) = best_weighted {
        return best_s;
    }

    best_count.map_or(BlockStateId::AIR, |(s, _)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservative_occupancy() {
        let child_key = LodNodeKey::new(0, 0, 0, 0);
        let mut child = LodNode::empty(child_key);

        // Place a single solid stone block in child octant 0 at (0, 0, 0)
        let stone = BlockStateId::new(1);
        child.set_state(0, 0, 0, stone);

        let parent_key = LodNodeKey::new(1, 0, 0, 0);
        let children: [Option<&LodNode>; 8] =
            [Some(&child), None, None, None, None, None, None, None];

        let parent = downsample_octants(parent_key, &children);
        assert!(parent.occupancy.is_solid(0, 0, 0));
        assert_eq!(parent.get_state(0, 0, 0), stone);

        // Surrounding parent voxels must remain air
        assert!(!parent.occupancy.is_solid(1, 0, 0));
        assert!(!parent.occupancy.is_solid(0, 1, 0));
        assert!(!parent.occupancy.is_solid(0, 0, 1));
    }

    #[test]
    fn test_top_face_weight_precedence() {
        let child_key = LodNodeKey::new(0, 0, 0, 0);
        let mut child = LodNode::empty(child_key);

        let dirt = BlockStateId::new(2);
        let grass = BlockStateId::new(3);

        // Top voxel at (0, 1, 0) is grass (exposed to top), bottom at (0, 0, 0) is dirt
        child.set_state(0, 0, 0, dirt);
        child.set_state(0, 1, 0, grass);

        let parent_key = LodNodeKey::new(1, 0, 0, 0);
        let children: [Option<&LodNode>; 8] =
            [Some(&child), None, None, None, None, None, None, None];

        let parent = downsample_octants(parent_key, &children);
        // Grass has higher top-facing weight (4.0) than side dirt
        assert_eq!(parent.get_state(0, 0, 0), grass);
    }
}
