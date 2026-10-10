//! Procedural Nether dimension cavern chunk generation.
//!
//! Generates cubic chunks for the Nether dimension ($Y \in [0, 4)$, world $Y \in [0, 128)$).
//! Features:
//! - Out of bounds chunks ($Y < 0$ or $Y \ge 4$) generate uniform air chunks.
//! - Solid and probabilistic bedrock floor at world $Y \in [0, 4)$.
//! - Solid and probabilistic bedrock ceiling at world $Y \in [124, 128)$.
//! - 3D coarse density noise forming massive cavern halls, overhangs, pillars, and ledges.
//! - Lava ocean sea level at world $Y < 32$.
//! - Soul sand shores and sediment patches around lava sea level and cavern formations.
//! - Ceiling glowstone clusters hanging down into air pockets.

use crate::density::{CELLS_XZ, CELLS_Y, CORNERS_XZ, CORNERS_Y, TOTAL_CORNERS};
use crate::noise::fbm3d;
use crate::surface::ResolvedBlocks;
use telos_core::coords::{CHUNK_VOLUME, ChunkPos};
use telos_voxel::chunk::Chunk;
use telos_voxel::state::BlockStateId;
use telos_voxel::storage::bulk;

/// Fast deterministic 64-bit integer coordinate hash.
#[inline]
fn pos_hash(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x517c_c1b7_2722_0a95)
        ^ (y as u64).wrapping_mul(0x6c62_272e_07bb_0142)
        ^ (z as u64).wrapping_mul(0x62b8_2175_6295_c58d);
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 30;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 27;
    h
}

/// Evaluates Nether density at world-space coordinate $(x, y, z)$.
#[inline]
fn sample_nether_density(seed: u64, x: f32, y: f32, z: f32) -> f32 {
    let base_bias = if y < 32.0 {
        let t = (32.0 - y) / 24.0;
        t * t * 1.5 - 0.35
    } else if y > 96.0 {
        let t = (y - 96.0) / 24.0;
        t * t * 1.5 - 0.35
    } else {
        -0.35
    };

    let large_caverns = fbm3d(
        seed.wrapping_add(1337),
        x,
        y * 0.7,
        z,
        2,
        1.0 / 80.0,
        2.0,
        0.5,
    ) * 1.6;
    let med_detail = fbm3d(seed.wrapping_add(7331), x, y, z, 2, 1.0 / 40.0, 2.0, 0.5) * 0.9;

    base_bias + large_caverns + med_detail
}

/// Generates a Nether dimension cubic chunk at the given chunk coordinates.
#[must_use]
#[allow(
    clippy::large_stack_arrays,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::cast_possible_wrap
)]
pub fn generate_nether_chunk(seed: u64, pos: ChunkPos, blocks: &ResolvedBlocks) -> Chunk {
    // Out of bounds vertical chunks are pure air
    if !(0..4).contains(&pos.y()) {
        return Chunk::new_uniform(pos, BlockStateId::AIR, false);
    }

    let origin_x = (pos.x() * 32) as f32;
    let origin_y = (pos.y() * 32) as f32;
    let origin_z = (pos.z() * 32) as f32;

    // 1. Evaluate 405 corner samples on coarse 4x8x4 grid
    let mut corner_densities = [0.0f32; TOTAL_CORNERS];
    for cz in 0..CORNERS_XZ {
        let z = origin_z + (cz * 4) as f32;
        for cy in 0..CORNERS_Y {
            let y = origin_y + (cy * 8) as f32;
            for cx in 0..CORNERS_XZ {
                let x = origin_x + (cx * 4) as f32;
                let idx = cz * (CORNERS_Y * CORNERS_XZ) + cy * CORNERS_XZ + cx;
                corner_densities[idx] = sample_nether_density(seed, x, y, z);
            }
        }
    }

    // 2. Trilinear interpolation into 32³ occupancy
    let mut occupancy = [false; CHUNK_VOLUME];
    for cell_z in 0..CELLS_XZ {
        let cz0 = cell_z;
        let cz1 = cell_z + 1;
        for cell_y in 0..CELLS_Y {
            let cy0 = cell_y;
            let cy1 = cell_y + 1;
            for cell_x in 0..CELLS_XZ {
                let cx0 = cell_x;
                let cx1 = cell_x + 1;

                let c000 =
                    corner_densities[cz0 * (CORNERS_Y * CORNERS_XZ) + cy0 * CORNERS_XZ + cx0];
                let c100 =
                    corner_densities[cz0 * (CORNERS_Y * CORNERS_XZ) + cy0 * CORNERS_XZ + cx1];
                let c010 =
                    corner_densities[cz0 * (CORNERS_Y * CORNERS_XZ) + cy1 * CORNERS_XZ + cx0];
                let c110 =
                    corner_densities[cz0 * (CORNERS_Y * CORNERS_XZ) + cy1 * CORNERS_XZ + cx1];
                let c001 =
                    corner_densities[cz1 * (CORNERS_Y * CORNERS_XZ) + cy0 * CORNERS_XZ + cx0];
                let c101 =
                    corner_densities[cz1 * (CORNERS_Y * CORNERS_XZ) + cy0 * CORNERS_XZ + cx1];
                let c011 =
                    corner_densities[cz1 * (CORNERS_Y * CORNERS_XZ) + cy1 * CORNERS_XZ + cx0];
                let c111 =
                    corner_densities[cz1 * (CORNERS_Y * CORNERS_XZ) + cy1 * CORNERS_XZ + cx1];

                // Fast sign culling
                let all_positive = c000 > 0.0
                    && c100 > 0.0
                    && c010 > 0.0
                    && c110 > 0.0
                    && c001 > 0.0
                    && c101 > 0.0
                    && c011 > 0.0
                    && c111 > 0.0;
                let all_negative = c000 <= 0.0
                    && c100 <= 0.0
                    && c010 <= 0.0
                    && c110 <= 0.0
                    && c001 <= 0.0
                    && c101 <= 0.0
                    && c011 <= 0.0
                    && c111 <= 0.0;

                if all_positive {
                    for dy in 0..8 {
                        let y = cell_y * 8 + dy;
                        let y_offset = y << 10;
                        for dz in 0..4 {
                            let z = cell_z * 4 + dz;
                            let z_offset = y_offset | (z << 5);
                            for dx in 0..4 {
                                let x = cell_x * 4 + dx;
                                occupancy[z_offset | x] = true;
                            }
                        }
                    }
                    continue;
                }

                if all_negative {
                    continue;
                }

                for dy in 0..8 {
                    let fy = dy as f32 / 8.0;
                    let y = cell_y * 8 + dy;
                    let y_offset = y << 10;

                    let c00 = c000 * (1.0 - fy) + c010 * fy;
                    let c10 = c100 * (1.0 - fy) + c110 * fy;
                    let c01 = c001 * (1.0 - fy) + c011 * fy;
                    let c11 = c101 * (1.0 - fy) + c111 * fy;

                    for dz in 0..4 {
                        let fz = dz as f32 / 4.0;
                        let z = cell_z * 4 + dz;
                        let z_offset = y_offset | (z << 5);

                        let c0 = c00 * (1.0 - fz) + c01 * fz;
                        let c1 = c10 * (1.0 - fz) + c11 * fz;

                        for dx in 0..4 {
                            let fx = dx as f32 / 4.0;
                            let x = cell_x * 4 + dx;
                            let d = c0 * (1.0 - fx) + c1 * fx;
                            occupancy[z_offset | x] = d > 0.0;
                        }
                    }
                }
            }
        }
    }

    // 3. Populate block IDs into dense array
    let mut dense = [BlockStateId::AIR; CHUNK_VOLUME];
    for y in 0..32 {
        let wy = pos.y() * 32 + (y as i32);
        let y_offset = y << 10;
        for z in 0..32 {
            let wz = pos.z() * 32 + (z as i32);
            let z_offset = y_offset | (z << 5);
            for x in 0..32 {
                let wx = pos.x() * 32 + (x as i32);
                let idx = z_offset | x;

                if wy == 0 {
                    dense[idx] = blocks.bedrock;
                } else if wy < 4 {
                    if (pos_hash(seed, wx, wy, wz) % 4) as i32 >= wy {
                        dense[idx] = blocks.bedrock;
                    } else {
                        dense[idx] = blocks.netherrack;
                    }
                } else if wy == 127 {
                    dense[idx] = blocks.bedrock;
                } else if wy >= 124 {
                    if (pos_hash(seed.wrapping_add(1), wx, wy, wz) % 4) as i32 <= (127 - wy) {
                        dense[idx] = blocks.bedrock;
                    } else {
                        dense[idx] = blocks.netherrack;
                    }
                } else if occupancy[idx] {
                    // Soul sand shores around lava level
                    let soul_noise = crate::noise::simplex::noise2(
                        seed.wrapping_add(444),
                        wx as f32 * 0.04,
                        wz as f32 * 0.04,
                    );
                    if (27..=35).contains(&wy) && soul_noise > 0.2 {
                        dense[idx] = blocks.soul_sand;
                    } else {
                        dense[idx] = blocks.netherrack;
                    }
                } else if wy < 32 {
                    dense[idx] = blocks.lava;
                } else {
                    dense[idx] = blocks.air;
                }
            }
        }
    }

    // 4. Ceiling Glowstone cluster decoration
    for z in 2..30 {
        let wz = pos.z() * 32 + (z as i32);
        for x in 2..30 {
            let wx = pos.x() * 32 + (x as i32);
            for y in (1..32).rev() {
                let wy = pos.y() * 32 + (y as i32);
                let idx = (y << 10) | (z << 5) | x;
                let below_idx = idx - (1 << 10);

                if dense[idx] == blocks.netherrack
                    && dense[below_idx] == blocks.air
                    && (36..120).contains(&wy)
                {
                    let cluster_h = pos_hash(seed.wrapping_add(9999), wx, wy, wz);
                    if cluster_h.is_multiple_of(48) {
                        let len =
                            ((pos_hash(seed.wrapping_add(8888), wx, wy, wz) % 3) + 1) as usize;
                        for k in 1..=len {
                            if y >= k {
                                let drop_idx = idx - (k << 10);
                                if dense[drop_idx] == blocks.air {
                                    dense[drop_idx] = blocks.glowstone;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let packed_blocks = bulk::from_dense(&dense);
    let water_id = blocks.water;
    let lava_id = blocks.lava;
    Chunk::from_blocks(pos, packed_blocks, move |state| {
        state != BlockStateId::AIR && state != water_id && state != lava_id
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use telos_voxel::coords::LocalIdx;
    use telos_voxel::registry::BlockRegistry;

    #[test]
    fn test_nether_chunk_vertical_bounds() {
        let reg = BlockRegistry::standard();
        let blocks = ResolvedBlocks::resolve(&reg);
        let seed = 42;

        // Y < 0 chunk is all air
        let sub_chunk = generate_nether_chunk(seed, ChunkPos::new(0, -1, 0), &blocks);
        assert!(sub_chunk.occupancy().is_empty());

        // Y >= 4 chunk is all air
        let super_chunk = generate_nether_chunk(seed, ChunkPos::new(0, 4, 0), &blocks);
        assert!(super_chunk.occupancy().is_empty());

        // Y = 0 chunk has bedrock at bottom y=0 and lava ocean
        let floor_chunk = generate_nether_chunk(seed, ChunkPos::new(0, 0, 0), &blocks);
        assert!(!floor_chunk.occupancy().is_empty());
        assert_eq!(
            floor_chunk.get(LocalIdx::from_coords(0, 0, 0).unwrap()),
            blocks.bedrock
        );

        // Y = 3 chunk has bedrock at ceiling y=31 (world y=127)
        let ceiling_chunk = generate_nether_chunk(seed, ChunkPos::new(0, 3, 0), &blocks);
        assert!(!ceiling_chunk.occupancy().is_empty());
        assert_eq!(
            ceiling_chunk.get(LocalIdx::from_coords(0, 31, 0).unwrap()),
            blocks.bedrock
        );
    }

    #[test]
    fn test_nether_chunk_features() {
        let reg = BlockRegistry::standard();
        let blocks = ResolvedBlocks::resolve(&reg);
        let seed = 12345;

        let chunk = generate_nether_chunk(seed, ChunkPos::new(0, 1, 0), &blocks);
        assert!(!chunk.occupancy().is_empty());

        let mut has_netherrack = false;
        let mut has_air = false;
        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    let b = chunk.get(LocalIdx::from_coords(x, y, z).unwrap());
                    if b == blocks.netherrack {
                        has_netherrack = true;
                    } else if b == blocks.air {
                        has_air = true;
                    }
                }
            }
        }
        assert!(has_netherrack, "Nether chunk must contain netherrack");
        assert!(has_air, "Nether chunk must have cavern air space");
    }
}
