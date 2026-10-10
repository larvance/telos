//! Procedural Nether dimension cavern chunk generation.
//!
//! Generates cubic chunks for the Nether dimension ($Y \in [0, 4)$, world $Y \in [0, 128)$).
//! Features:
//! - Out of bounds chunks ($Y < 0$ or $Y \ge 4$) generate uniform air chunks.
//! - Solid and probabilistic bedrock floor at world $Y \in [0, 4)$.
//! - Solid and probabilistic bedrock ceiling at world $Y \in [124, 128)$.
//! - Multi-noise Nether biome classification (Nether Wastes, Soul Sand Valley, Basalt Deltas, Crimson Forest, Warped Forest).
//! - 3D coarse density noise forming massive cavern halls, overhangs, pillars, and ledges.
//! - Lava ocean sea level at world $Y < 32$.
//! - Basalt Deltas cellular basalt column spires, blackstone terraces, and incandescent magma pools.
//! - Soul Sand Valley soul sand and soul soil strata.
//! - Ceiling glowstone multi-bulb branching stalactite chandeliers.
//! - Nether ore veins: Nether Quartz (16 veins/chunk), Nether Gold (10 veins/chunk), and deeply buried Ancient Debris (air-exposure discard).
//! - Subterranean fortress corridor reservation hooks.

use crate::density::{CELLS_XZ, CELLS_Y, CORNERS_XZ, CORNERS_Y, TOTAL_CORNERS};
use crate::noise::fbm3d;
use crate::noise::simplex::noise2;
use crate::surface::ResolvedBlocks;
use telos_core::coords::{CHUNK_VOLUME, ChunkPos};
use telos_voxel::chunk::Chunk;
use telos_voxel::state::BlockStateId;
use telos_voxel::storage::bulk;

/// Multi-noise procedural biomes in the Nether dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetherBiome {
    /// Classic cavernous netherrack terrain, gravel/soul sand shores, hanging glowstone.
    NetherWastes,
    /// Wide cavernous hollows lined with soul sand, soul soil, and basalt strata.
    SoulSandValley,
    /// Chaotic jagged terrain dominated by basalt columns, blackstone terraces, and magma pools.
    BasaltDeltas,
    /// Dense crimson nylium terrain with large fungal overhangs.
    CrimsonForest,
    /// Sprawling cyan warped nylium caverns.
    WarpedForest,
}

/// Evaluates 2D climate noise to sample the active Nether biome at horizontal coordinates $(wx, wz)$.
#[must_use]
pub fn sample_nether_biome(seed: u64, wx: i32, wz: i32) -> NetherBiome {
    let temp = noise2(seed.wrapping_add(101), wx as f32 * 0.003, wz as f32 * 0.003);
    let hum = noise2(seed.wrapping_add(202), wx as f32 * 0.003, wz as f32 * 0.003);

    if temp < -0.2 {
        if hum < 0.0 {
            NetherBiome::SoulSandValley
        } else {
            NetherBiome::WarpedForest
        }
    } else if temp > 0.2 {
        if hum < -0.1 {
            NetherBiome::BasaltDeltas
        } else {
            NetherBiome::CrimsonForest
        }
    } else {
        NetherBiome::NetherWastes
    }
}

/// Returns `true` if the coordinate $(wx, wy, wz)$ falls inside a subterranean fortress corridor reservation volume.
#[must_use]
pub fn is_fortress_reserved(seed: u64, wx: i32, wy: i32, wz: i32) -> bool {
    if !(48..=68).contains(&wy) {
        return false;
    }
    let region_chunk_x = wx.div_euclid(256);
    let region_chunk_z = wz.div_euclid(256);
    let h = pos_hash(
        seed.wrapping_add(0xF087_1E55),
        region_chunk_x,
        0,
        region_chunk_z,
    );
    if !h.is_multiple_of(3) {
        return false;
    }
    let center_x = region_chunk_x * 256 + 100 + ((h >> 8) % 56) as i32;
    let center_z = region_chunk_z * 256 + 100 + ((h >> 16) % 56) as i32;
    (wx - center_x).abs() <= 12 && (wz - center_z).abs() <= 12
}

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

/// Samples cellular Voronoi basalt column parameters for Basalt Deltas at $(wx, wz)$.
///
/// Returns `Some((base_y, top_y, dist_sq))` if horizontal coordinates lie within a column spire.
#[inline]
#[allow(clippy::similar_names)]
fn basalt_delta_column_sample(seed: u64, wx: i32, wz: i32) -> Option<(i32, i32, i32)> {
    let cell_size = 7;
    let cell_x = wx.div_euclid(cell_size);
    let cell_z = wz.div_euclid(cell_size);

    for dz in -1..=1 {
        for dx in -1..=1 {
            let cx_cell = cell_x + dx;
            let cz_cell = cell_z + dz;
            let h = pos_hash(seed.wrapping_add(5555), cx_cell, 0, cz_cell);
            // 70% of cells contain a basalt spire
            if (h % 10) >= 7 {
                continue;
            }
            let center_x = cx_cell * cell_size + ((h >> 4) % 5 + 1) as i32;
            let center_z = cz_cell * cell_size + ((h >> 8) % 5 + 1) as i32;
            let dist_sq = (wx - center_x) * (wx - center_x) + (wz - center_z) * (wz - center_z);
            let wide = (h >> 12).is_multiple_of(3);
            let radius_sq = if wide { 3 } else { 1 };
            if dist_sq <= radius_sq {
                let base_y = 20;
                let height = 12 + ((h >> 16) % 18) as i32; // up to 30 blocks high
                let top_y = (base_y + height).min(115);
                return Some((base_y, top_y, dist_sq));
            }
        }
    }
    None
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

/// Deposits a Nether ore vein cluster into `dense` using a bounded 3D random walk.
#[allow(clippy::too_many_arguments, clippy::cast_possible_wrap)]
fn apply_nether_ore_vein(
    dense: &mut [BlockStateId; CHUNK_VOLUME],
    pos: ChunkPos,
    ore_block: BlockStateId,
    vein_seed: u64,
    size: usize,
    min_wy: i32,
    max_wy: i32,
    must_be_buried: bool,
    blocks: &ResolvedBlocks,
) {
    let vx = (vein_seed & 31) as i32;
    let vy = ((vein_seed >> 5) & 31) as i32;
    let vz = ((vein_seed >> 10) & 31) as i32;
    let wy = pos.y() * 32 + vy;

    if wy < min_wy || wy > max_wy {
        return;
    }

    let mut cur_x = vx;
    let mut cur_y = vy;
    let mut cur_z = vz;

    for step in 0..size {
        let step_h = pos_hash(vein_seed, cur_x, cur_y + step as i32, cur_z);
        let ox = match (step_h >> 4) % 3 {
            0 => -1,
            1 => 0,
            _ => 1,
        };
        let oy = match (step_h >> 6) % 3 {
            0 => -1,
            1 => 0,
            _ => 1,
        };
        let oz = match (step_h >> 8) % 3 {
            0 => -1,
            1 => 0,
            _ => 1,
        };

        cur_x = (cur_x + ox).clamp(0, 31);
        cur_y = (cur_y + oy).clamp(0, 31);
        cur_z = (cur_z + oz).clamp(0, 31);

        let idx = (cur_y as usize) << 10 | (cur_z as usize) << 5 | (cur_x as usize);
        let current_id = dense[idx];

        let is_host = current_id == blocks.netherrack
            || current_id == blocks.basalt
            || current_id == blocks.blackstone;

        if is_host {
            if must_be_buried {
                let is_exposed = (cur_x > 0 && dense[idx - 1] == blocks.air)
                    || (cur_x < 31 && dense[idx + 1] == blocks.air)
                    || (cur_z > 0 && dense[idx - 32] == blocks.air)
                    || (cur_z < 31 && dense[idx + 32] == blocks.air)
                    || (cur_y > 0 && dense[idx - 1024] == blocks.air)
                    || (cur_y < 31 && dense[idx + 1024] == blocks.air);
                if is_exposed {
                    continue;
                }
            }
            dense[idx] = ore_block;
        }
    }
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

    // 3. Populate block IDs into dense array with Nether biomes and formations
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
                } else {
                    let biome = sample_nether_biome(seed, wx, wz);

                    if occupancy[idx] {
                        match biome {
                            NetherBiome::BasaltDeltas => {
                                let h = pos_hash(seed.wrapping_add(333), wx, wy, wz);
                                if (h % 10) < 5 {
                                    dense[idx] = blocks.basalt;
                                } else if (h % 10) < 8 {
                                    dense[idx] = blocks.blackstone;
                                } else if wy <= 35 && (h % 10) == 9 {
                                    dense[idx] = blocks.magma_block;
                                } else {
                                    dense[idx] = blocks.blackstone;
                                }
                            }
                            NetherBiome::SoulSandValley => {
                                let soul_noise = noise2(
                                    seed.wrapping_add(444),
                                    wx as f32 * 0.05,
                                    wz as f32 * 0.05,
                                );
                                if (24..=45).contains(&wy) || soul_noise > -0.1 {
                                    if wy >= 31 {
                                        dense[idx] = blocks.soul_sand;
                                    } else {
                                        dense[idx] = blocks.soul_soil;
                                    }
                                } else {
                                    dense[idx] = blocks.netherrack;
                                }
                            }
                            NetherBiome::NetherWastes => {
                                let soul_noise = noise2(
                                    seed.wrapping_add(444),
                                    wx as f32 * 0.04,
                                    wz as f32 * 0.04,
                                );
                                if (27..=35).contains(&wy) && soul_noise > 0.2 {
                                    dense[idx] = blocks.soul_sand;
                                } else {
                                    dense[idx] = blocks.netherrack;
                                }
                            }
                            NetherBiome::CrimsonForest | NetherBiome::WarpedForest => {
                                dense[idx] = blocks.netherrack;
                            }
                        }
                    } else {
                        // Open space (cavern hollows or ocean)
                        if biome == NetherBiome::BasaltDeltas
                            && let Some((base_y, top_y, _)) =
                                basalt_delta_column_sample(seed, wx, wz)
                            && wy >= base_y
                            && wy <= top_y
                        {
                            dense[idx] = blocks.basalt;
                            continue;
                        }

                        if wy < 32 {
                            dense[idx] = blocks.lava;
                        } else {
                            dense[idx] = blocks.air;
                        }
                    }
                }
            }
        }
    }

    // 4. Ceiling Glowstone stalactite chandelier decoration
    for z in 2..30 {
        let wz = pos.z() * 32 + (z as i32);
        for x in 2..30 {
            let wx = pos.x() * 32 + (x as i32);
            for y in (1..32).rev() {
                let wy = pos.y() * 32 + (y as i32);
                let idx = (y << 10) | (z << 5) | x;
                let below_idx = idx - (1 << 10);

                let is_ceiling_rock = dense[idx] == blocks.netherrack
                    || dense[idx] == blocks.basalt
                    || dense[idx] == blocks.blackstone;

                if is_ceiling_rock && dense[below_idx] == blocks.air && (36..120).contains(&wy) {
                    let cluster_h = pos_hash(seed.wrapping_add(9999), wx, wy, wz);
                    if cluster_h.is_multiple_of(32) {
                        let len = (((cluster_h >> 8) % 5) + 2) as usize; // Length 2..=6
                        for k in 1..=len {
                            if y >= k {
                                let drop_idx = idx - (k << 10);
                                if dense[drop_idx] == blocks.air {
                                    dense[drop_idx] = blocks.glowstone;
                                }

                                // Branching horizontal bulbs
                                for (bx, bz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                    let nx = x as i32 + bx;
                                    let nz = z as i32 + bz;
                                    if (1..31).contains(&nx) && (1..31).contains(&nz) {
                                        let bulb_h = pos_hash(
                                            cluster_h.wrapping_add(k as u64),
                                            nx,
                                            wy - k as i32,
                                            nz,
                                        );
                                        if bulb_h.is_multiple_of(3) {
                                            let bulb_idx = ((y - k) << 10)
                                                | ((nz as usize) << 5)
                                                | (nx as usize);
                                            if dense[bulb_idx] == blocks.air {
                                                dense[bulb_idx] = blocks.glowstone;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Nether Ore Vein Strata: Quartz, Gold, and Ancient Debris
    // Nether Quartz: 16 clusters per chunk between Y in [10, 118]
    for v in 0..16 {
        let v_seed = pos_hash(
            seed.wrapping_add(0xCAFE_0001),
            pos.x(),
            pos.y() * 32 + v,
            pos.z(),
        );
        let size = 4 + ((v_seed >> 16) % 10) as usize;
        apply_nether_ore_vein(
            &mut dense,
            pos,
            blocks.nether_quartz_ore,
            v_seed,
            size,
            10,
            118,
            false,
            blocks,
        );
    }

    // Nether Gold: 10 clusters per chunk between Y in [10, 118]
    for v in 0..10 {
        let v_seed = pos_hash(
            seed.wrapping_add(0xCAFE_0002),
            pos.x(),
            pos.y() * 32 + v,
            pos.z(),
        );
        let size = 4 + ((v_seed >> 16) % 6) as usize;
        apply_nether_ore_vein(
            &mut dense,
            pos,
            blocks.nether_gold_ore,
            v_seed,
            size,
            10,
            118,
            false,
            blocks,
        );
    }

    // Ancient Debris: 2 clusters per chunk deep Y in [8, 22] (completely buried, air-exposure discard)
    for v in 0..2 {
        let v_seed = pos_hash(
            seed.wrapping_add(0xCAFE_0003),
            pos.x(),
            pos.y() * 32 + v,
            pos.z(),
        );
        let size = 1 + ((v_seed >> 16) % 3) as usize;
        apply_nether_ore_vein(
            &mut dense,
            pos,
            blocks.ancient_debris,
            v_seed,
            size,
            8,
            22,
            true,
            blocks,
        );
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

    #[test]
    fn test_nether_biome_sampling() {
        let seed = 987_654_321;
        let mut found_wastes = false;
        let mut found_valley = false;
        let mut found_deltas = false;
        let mut found_crimson = false;
        let mut found_warped = false;

        for z in (-400..400).step_by(25) {
            for x in (-400..400).step_by(25) {
                let biome = sample_nether_biome(seed, x, z);
                let biome2 = sample_nether_biome(seed, x, z);
                assert_eq!(biome, biome2, "Biome sampling must be deterministic");

                match biome {
                    NetherBiome::NetherWastes => found_wastes = true,
                    NetherBiome::SoulSandValley => found_valley = true,
                    NetherBiome::BasaltDeltas => found_deltas = true,
                    NetherBiome::CrimsonForest => found_crimson = true,
                    NetherBiome::WarpedForest => found_warped = true,
                }
            }
        }

        assert!(found_wastes, "Should generate Nether Wastes");
        assert!(found_valley, "Should generate Soul Sand Valley");
        assert!(found_deltas, "Should generate Basalt Deltas");
        assert!(found_crimson, "Should generate Crimson Forest");
        assert!(found_warped, "Should generate Warped Forest");
    }

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_nether_ore_distribution() {
        let reg = BlockRegistry::standard();
        let blocks = ResolvedBlocks::resolve(&reg);
        let seed = 77_8899;

        // In deep chunk Y=0, ancient debris can generate
        let deep_chunk = generate_nether_chunk(seed, ChunkPos::new(0, 0, 0), &blocks);
        let mut quartz_count = 0;
        let mut gold_count = 0;
        let mut debris_count = 0;

        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    let b = deep_chunk.get(LocalIdx::from_coords(x, y, z).unwrap());
                    if b == blocks.nether_quartz_ore {
                        quartz_count += 1;
                    } else if b == blocks.nether_gold_ore {
                        gold_count += 1;
                    } else if b == blocks.ancient_debris {
                        debris_count += 1;
                        assert!((8..=22).contains(&y), "Ancient debris must be in [8, 22]");

                        // Verify ancient debris has zero air exposure
                        for (dx, dy, dz) in [
                            (-1, 0, 0),
                            (1, 0, 0),
                            (0, -1, 0),
                            (0, 1, 0),
                            (0, 0, -1),
                            (0, 0, 1),
                        ] {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            let nz = z as i32 + dz;
                            if (0..32).contains(&nx)
                                && (0..32).contains(&ny)
                                && (0..32).contains(&nz)
                            {
                                let neighbor = deep_chunk.get(
                                    LocalIdx::from_coords(nx as u32, ny as u32, nz as u32).unwrap(),
                                );
                                assert_ne!(
                                    neighbor, blocks.air,
                                    "Ancient debris must never be exposed to air!"
                                );
                            }
                        }
                    }
                }
            }
        }

        assert!(
            quartz_count > 0 || gold_count > 0,
            "Ore veins must spawn in Nether chunk"
        );
        let _ = debris_count;
    }

    #[test]
    fn test_basalt_delta_column_generation() {
        let reg = BlockRegistry::standard();
        let blocks = ResolvedBlocks::resolve(&reg);
        let seed = 42;

        // Find a chunk that contains Basalt Deltas
        let mut found_deltas_chunk = false;
        for cz in -10..10 {
            for cx in -10..10 {
                if sample_nether_biome(seed, cx * 32 + 16, cz * 32 + 16)
                    == NetherBiome::BasaltDeltas
                {
                    let chunk = generate_nether_chunk(seed, ChunkPos::new(cx, 1, cz), &blocks);
                    let mut basalt = 0;
                    let mut blackstone = 0;
                    for y in 0..32 {
                        for z in 0..32 {
                            for x in 0..32 {
                                let b = chunk.get(LocalIdx::from_coords(x, y, z).unwrap());
                                if b == blocks.basalt {
                                    basalt += 1;
                                } else if b == blocks.blackstone {
                                    blackstone += 1;
                                }
                            }
                        }
                    }
                    if basalt > 0 && blackstone > 0 {
                        found_deltas_chunk = true;
                        break;
                    }
                }
            }
            if found_deltas_chunk {
                break;
            }
        }
        assert!(
            found_deltas_chunk,
            "Basalt Deltas must contain basalt spires and blackstone"
        );
    }

    #[test]
    fn test_glowstone_chandelier_clustering() {
        let reg = BlockRegistry::standard();
        let blocks = ResolvedBlocks::resolve(&reg);
        let seed = 999;

        let mut found_glowstone = false;
        for cz in 0..5 {
            for cx in 0..5 {
                let chunk = generate_nether_chunk(seed, ChunkPos::new(cx, 2, cz), &blocks);
                let mut glowstone_count = 0;
                for y in 0..32 {
                    for z in 0..32 {
                        for x in 0..32 {
                            if chunk.get(LocalIdx::from_coords(x, y, z).unwrap())
                                == blocks.glowstone
                            {
                                glowstone_count += 1;
                            }
                        }
                    }
                }
                if glowstone_count > 1 {
                    found_glowstone = true;
                    break;
                }
            }
            if found_glowstone {
                break;
            }
        }
        assert!(
            found_glowstone,
            "Glowstone chandeliers must generate multi-block clusters"
        );
    }
}
