//! Server-authoritative world management, terrain generation, lighting caching, and persistence.

use glam::DVec3;
use hashbrown::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use telos_core::coords::{BlockPos, ChunkPos};
use telos_lod::{
    color::LodColorTable,
    coords::LodNodeKey,
    mesher::{LodMesh, mesh_lod_node},
    node::LodNode,
    pyramid::LodPyramid,
};
use telos_voxel::{
    chunk::{Chunk, ChunkSnapshot},
    coords::{LocalIdx, split_block_pos},
    light::{ChunkHeightmap, ChunkLight, ColumnHeights, LightBfs},
    registry::BlockRegistry,
    state::{BlockStateId, StateFlags},
};
use telos_worldgen::{GeneratorKind, WorldGenerator};

use crate::multi_world::WorldConfig;
use crate::storage::WorldStorage;

/// Actively tracked chunk containing mutable block storage and its latest published immutable snapshot.
pub struct ServerChunk {
    /// Mutable chunk storage.
    pub chunk: Chunk,
    /// Latest published immutable snapshot shared across network and mesher threads.
    pub snapshot: Arc<ChunkSnapshot>,
}

/// Server-side world storage, procedural generator, lighting cache, and LOD pyramid.
pub struct ServerWorld {
    name: String,
    generator: WorldGenerator,
    registry: BlockRegistry,
    chunks: HashMap<ChunkPos, ServerChunk>,
    columns: HashMap<(i32, i32), ColumnHeights>,
    light_bfs: LightBfs,
    pyramid: LodPyramid,
    lod_color_table: LodColorTable,
    lod_meshes: HashMap<LodNodeKey, Arc<LodMesh>>,
    storage: Option<WorldStorage>,
    dirty_chunks: HashSet<ChunkPos>,
    /// Active furnace block positions tracked in this world.
    pub furnace_positions: HashSet<BlockPos>,
    /// Deterministic logic & signal propagation engine.
    pub logic_engine: telos_sim::logic::LogicEngine,
    /// Real-time cellular automata fluid simulation engine.
    pub fluid_engine: telos_sim::fluid::FluidEngine,
}

impl ServerWorld {
    /// Creates a new in-memory `ServerWorld` with the given seed and block registry.
    #[must_use]
    pub fn new(seed: u64, registry: BlockRegistry) -> Self {
        Self::with_generator("overworld", seed, registry, GeneratorKind::Standard)
    }

    /// Creates a world with a specific name, seed, registry, and generator kind.
    #[must_use]
    pub fn with_generator(
        name: impl Into<String>,
        seed: u64,
        registry: BlockRegistry,
        kind: GeneratorKind,
    ) -> Self {
        let generator = WorldGenerator::with_kind(seed, &registry, kind);
        Self {
            name: name.into(),
            generator,
            registry,
            chunks: HashMap::new(),
            columns: HashMap::new(),
            light_bfs: LightBfs::new(),
            pyramid: LodPyramid::new(),
            lod_color_table: LodColorTable::standard(),
            lod_meshes: HashMap::new(),
            storage: None,
            dirty_chunks: HashSet::new(),
            furnace_positions: HashSet::new(),
            logic_engine: telos_sim::logic::LogicEngine::new(),
            fluid_engine: telos_sim::fluid::FluidEngine::new(),
        }
    }

    /// Creates a persistent `ServerWorld` with `.tlr` region container saving enabled.
    pub fn with_storage(
        seed: u64,
        registry: BlockRegistry,
        save_dir: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        let storage = WorldStorage::new(save_dir)?;
        let generator = WorldGenerator::new(seed, &registry);
        Ok(Self {
            name: "overworld".to_string(),
            generator,
            registry,
            chunks: HashMap::new(),
            columns: HashMap::new(),
            light_bfs: LightBfs::new(),
            pyramid: LodPyramid::new(),
            lod_color_table: LodColorTable::standard(),
            lod_meshes: HashMap::new(),
            storage: Some(storage),
            dirty_chunks: HashSet::new(),
            furnace_positions: HashSet::new(),
            logic_engine: telos_sim::logic::LogicEngine::new(),
            fluid_engine: telos_sim::fluid::FluidEngine::new(),
        })
    }

    /// Creates a persistent `ServerWorld` with `.tlr` region container saving and full save-ID mapping.
    pub fn with_content_storage(
        seed: u64,
        registries: &telos_content::FrozenRegistries,
        save_dir: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        Self::with_content_storage_and_compression(seed, registries, save_dir, true, 3)
    }

    /// Creates a persistent `ServerWorld` with `.tlr` region container saving, full save-ID mapping, and explicit compression settings.
    pub fn with_content_storage_and_compression(
        seed: u64,
        registries: &telos_content::FrozenRegistries,
        save_dir: impl AsRef<Path>,
        compression_enabled: bool,
        compression_level: u32,
    ) -> std::io::Result<Self> {
        let storage = WorldStorage::open_or_create_with_compression(
            save_dir,
            registries,
            compression_enabled,
            compression_level,
        )?;
        let registry = registries.block_registry().clone();
        let generator = WorldGenerator::new(seed, &registry);
        Ok(Self {
            name: "overworld".to_string(),
            generator,
            registry,
            chunks: HashMap::new(),
            columns: HashMap::new(),
            light_bfs: LightBfs::new(),
            pyramid: LodPyramid::new(),
            lod_color_table: LodColorTable::standard(),
            lod_meshes: HashMap::new(),
            storage: Some(storage),
            dirty_chunks: HashSet::new(),
            furnace_positions: HashSet::new(),
            logic_engine: telos_sim::logic::LogicEngine::new(),
            fluid_engine: telos_sim::fluid::FluidEngine::new(),
        })
    }

    /// Creates a `ServerWorld` configured from a `WorldConfig`.
    pub fn with_config(
        config: &WorldConfig,
        registries: &telos_content::FrozenRegistries,
    ) -> std::io::Result<Self> {
        let registry = registries.block_registry().clone();
        let generator = WorldGenerator::with_kind(config.seed, &registry, config.generator);
        let storage = if let Some(ref dir) = config.save_directory {
            Some(WorldStorage::open_or_create_with_compression(
                dir,
                registries,
                config.region_compression,
                config.region_compression_level,
            )?)
        } else {
            None
        };

        Ok(Self {
            name: config.name.clone(),
            generator,
            registry,
            chunks: HashMap::new(),
            columns: HashMap::new(),
            light_bfs: LightBfs::new(),
            pyramid: LodPyramid::new(),
            lod_color_table: LodColorTable::standard(),
            lod_meshes: HashMap::new(),
            storage,
            dirty_chunks: HashSet::new(),
            furnace_positions: HashSet::new(),
            logic_engine: telos_sim::logic::LogicEngine::new(),
            fluid_engine: telos_sim::fluid::FluidEngine::new(),
        })
    }

    /// Returns the world name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the active generator algorithm kind.
    #[must_use]
    pub const fn generator_kind(&self) -> GeneratorKind {
        self.generator.kind()
    }

    /// Returns the world generation seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.generator.seed()
    }

    /// Accesses the server's block registry.
    #[must_use]
    pub fn registry(&self) -> &BlockRegistry {
        &self.registry
    }

    /// Retrieves an existing chunk snapshot or generates, lights, and caches it.
    #[allow(clippy::cast_possible_wrap)]
    pub fn get_or_generate_chunk(&mut self, pos: ChunkPos) -> Arc<ChunkSnapshot> {
        if let Some(sc) = self.chunks.get(&pos) {
            return sc.snapshot.clone();
        }

        // 1. Try loading from persistent storage if configured
        if let Some(storage) = &mut self.storage
            && let Ok(Some(mut chunk)) = storage.load_chunk(pos, &self.registry)
        {
            let col_key = (pos.x(), pos.z());
            let col_heights = self.columns.entry(col_key).or_default();
            let local_h = ChunkHeightmap::from_occupancy(chunk.occupancy());
            col_heights.update_chunk(pos.y(), &local_h);

            if chunk.light().is_none() {
                let mut chunk_light = ChunkLight::default();
                let is_opaque = |idx: usize| -> u8 {
                    let x = (idx & 0x1F) as u32;
                    let z = ((idx >> 5) & 0x1F) as u32;
                    let y = ((idx >> 10) & 0x1F) as u32;
                    if chunk.occupancy().is_solid(x, y, z) {
                        15
                    } else {
                        0
                    }
                };
                self.light_bfs.compute_initial_sky_light(
                    pos.y(),
                    col_heights,
                    &mut chunk_light.sky,
                    is_opaque,
                );
                Self::diffuse_cross_chunk_boundary(
                    &self.chunks,
                    pos,
                    &mut chunk_light,
                    &mut self.light_bfs,
                    is_opaque,
                );
                chunk_light.try_collapse();
                chunk.set_light(Some(chunk_light));
            }

            let snapshot = chunk.publish_snapshot();
            for (idx, be) in chunk.block_entities().iter() {
                if be.kind() == telos_voxel::block_entity::BlockEntityKind::Furnace {
                    self.furnace_positions.insert(BlockPos::new(
                        pos.x() * 32 + idx.x() as i32,
                        pos.y() * 32 + idx.y() as i32,
                        pos.z() * 32 + idx.z() as i32,
                    ));
                }
            }
            self.chunks.insert(
                pos,
                ServerChunk {
                    chunk,
                    snapshot: snapshot.clone(),
                },
            );
            return snapshot;
        }

        // 2. Generate chunk terrain
        let mut chunk = self.generator.generate_chunk(pos);

        // 3. Update column heightmap
        let col_key = (pos.x(), pos.z());
        let col_heights = self.columns.entry(col_key).or_default();
        let local_h = ChunkHeightmap::from_occupancy(chunk.occupancy());
        col_heights.update_chunk(pos.y(), &local_h);

        // 4. Compute initial sky light
        let mut chunk_light = ChunkLight::default();
        let is_opaque = |idx: usize| -> u8 {
            let x = (idx & 0x1F) as u32;
            let z = ((idx >> 5) & 0x1F) as u32;
            let y = ((idx >> 10) & 0x1F) as u32;
            if chunk.occupancy().is_solid(x, y, z) {
                15
            } else {
                0
            }
        };

        self.light_bfs.compute_initial_sky_light(
            pos.y(),
            col_heights,
            &mut chunk_light.sky,
            is_opaque,
        );
        Self::diffuse_cross_chunk_boundary(
            &self.chunks,
            pos,
            &mut chunk_light,
            &mut self.light_bfs,
            is_opaque,
        );
        chunk_light.try_collapse();
        chunk.set_light(Some(chunk_light));

        // 5. Publish snapshot and cache
        let snapshot = chunk.publish_snapshot();
        if self.storage.is_some() {
            self.dirty_chunks.insert(pos);
        }
        self.chunks.insert(
            pos,
            ServerChunk {
                chunk,
                snapshot: snapshot.clone(),
            },
        );

        snapshot
    }

    /// Queries the block state at a world `BlockPos`, generating the chunk if missing.
    pub fn get_block(&mut self, pos: BlockPos) -> BlockStateId {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        let snapshot = self.get_or_generate_chunk(chunk_pos);
        snapshot.blocks().get(local_idx)
    }

    /// Queries the block state at a world `BlockPos` if the chunk is loaded; otherwise returns `BlockStateId::AIR`.
    #[must_use]
    pub fn get_loaded_block(&self, pos: BlockPos) -> BlockStateId {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        if let Some(sc) = self.chunks.get(&chunk_pos) {
            sc.chunk.get(local_idx)
        } else {
            BlockStateId::AIR
        }
    }

    /// Retrieves a block entity at `pos`, if present and loaded.
    #[must_use]
    pub fn get_block_entity(
        &self,
        pos: BlockPos,
    ) -> Option<&telos_voxel::block_entity::BlockEntityData> {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        let sc = self.chunks.get(&chunk_pos)?;
        sc.chunk.get_block_entity(local_idx)
    }

    /// Retrieves a mutable block entity at `pos`, if present and loaded.
    /// Flags the chunk dirty for persistence.
    pub fn get_block_entity_mut(
        &mut self,
        pos: BlockPos,
    ) -> Option<&mut telos_voxel::block_entity::BlockEntityData> {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        self.dirty_chunks.insert(chunk_pos);
        let sc = self.chunks.get_mut(&chunk_pos)?;
        sc.chunk.get_block_entity_mut(local_idx)
    }

    /// Sets or replaces a block entity at `pos`, flagging the chunk dirty for persistence.
    pub fn set_block_entity(
        &mut self,
        pos: BlockPos,
        data: telos_voxel::block_entity::BlockEntityData,
    ) -> Option<telos_voxel::block_entity::BlockEntityData> {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        self.dirty_chunks.insert(chunk_pos);
        if data.kind() == telos_voxel::block_entity::BlockEntityKind::Furnace {
            self.furnace_positions.insert(pos);
        }
        let sc = self.chunks.get_mut(&chunk_pos)?;
        sc.chunk.set_block_entity(local_idx, data)
    }

    /// Removes a block entity at `pos`, flagging the chunk dirty for persistence.
    pub fn remove_block_entity(
        &mut self,
        pos: BlockPos,
    ) -> Option<telos_voxel::block_entity::BlockEntityData> {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        self.dirty_chunks.insert(chunk_pos);
        self.furnace_positions.remove(&pos);
        let sc = self.chunks.get_mut(&chunk_pos)?;
        sc.chunk.remove_block_entity(local_idx)
    }

    /// Sets a block in the world, updating chunk storage, heightmaps, and lighting.
    ///
    /// Invalidates affected LOD pyramid levels and flags the chunk dirty for save.
    /// Returns `Some((snapshot, version))` if state changed, or `None` if unchanged or out of bounds.
    pub fn set_block(
        &mut self,
        pos: BlockPos,
        new_state: BlockStateId,
    ) -> Option<(Arc<ChunkSnapshot>, u64)> {
        let (chunk_pos, local_idx) = split_block_pos(pos);
        self.get_or_generate_chunk(chunk_pos);

        let registry = &self.registry;
        let sc = self.chunks.get_mut(&chunk_pos)?;

        let old_state = sc.chunk.get(local_idx);
        if old_state == new_state {
            return None;
        }

        let old_flags = registry.flags(old_state);
        let new_flags = registry.flags(new_state);

        let changed = sc.chunk.set(local_idx, new_state, old_flags, new_flags, 1);
        if !changed {
            return None;
        }

        let was_furnace = registry.is_furnace(old_state);
        let is_now_furnace = registry.is_furnace(new_state);
        if was_furnace && !is_now_furnace {
            self.furnace_positions.remove(&pos);
        } else if is_now_furnace {
            self.furnace_positions.insert(pos);
        }

        // Update column heightmap
        let col_key = (chunk_pos.x(), chunk_pos.z());
        let col_heights = self.columns.entry(col_key).or_default();
        let local_h = ChunkHeightmap::from_occupancy(sc.chunk.occupancy());
        col_heights.update_chunk(chunk_pos.y(), &local_h);

        // Update lighting
        let mut chunk_light = sc.chunk.light().cloned().unwrap_or_default();
        let is_opaque = |idx: usize| -> u8 {
            let x = (idx & 0x1F) as u32;
            let z = ((idx >> 5) & 0x1F) as u32;
            let y = ((idx >> 10) & 0x1F) as u32;
            if sc.chunk.occupancy().is_solid(x, y, z) {
                15
            } else {
                0
            }
        };

        let was_opaque = old_flags.contains(StateFlags::OPAQUE_FULL);
        let is_now_opaque = new_flags.contains(StateFlags::OPAQUE_FULL);

        if was_opaque != is_now_opaque {
            self.light_bfs.compute_initial_sky_light(
                chunk_pos.y(),
                col_heights,
                &mut chunk_light.sky,
                is_opaque,
            );
        }

        let lx = local_idx.x();
        let ly = local_idx.y();
        let lz = local_idx.z();

        let new_emission = self.registry.light_emission(new_state);
        let old_emission = self.registry.light_emission(old_state);

        if new_emission > 0 {
            self.light_bfs
                .add_source(&mut chunk_light.block, lx, ly, lz, new_emission);
            self.light_bfs
                .propagate_block_add(&mut chunk_light.block, is_opaque);
        } else if old_emission > 0 || old_flags.contains(StateFlags::EMITS_LIGHT) {
            self.light_bfs
                .remove_source(&mut chunk_light.block, lx, ly, lz);
            self.light_bfs
                .propagate_block_remove(&mut chunk_light.block, is_opaque);
        }

        chunk_light.try_collapse();
        sc.chunk.set_light(Some(chunk_light));

        let snapshot = sc.chunk.publish_snapshot();
        let version = snapshot.content_version();
        sc.snapshot = snapshot.clone();

        self.dirty_chunks.insert(chunk_pos);
        self.invalidate_lod_hierarchy(chunk_pos);

        // Logic component update and propagation
        self.update_logic_component(pos, old_flags, new_flags, new_state);

        // Notify fluid simulation engine of block mutation
        let mut engine = std::mem::take(&mut self.fluid_engine);
        engine.on_block_changed(pos, 0, self);
        self.fluid_engine = engine;

        Some((snapshot, version))
    }

    fn update_logic_component(
        &mut self,
        pos: BlockPos,
        old_flags: StateFlags,
        new_flags: StateFlags,
        new_state: BlockStateId,
    ) {
        if new_flags.contains(StateFlags::LOGIC_COMPONENT) {
            if let Some(ident) = self.registry.identifier(new_state) {
                let kind = match ident.path() {
                    "logic_wire" | "logic_wire_powered" => Some(telos_sim::logic::LogicKind::Wire),
                    "logic_power_block" => Some(telos_sim::logic::LogicKind::PowerBlock),
                    "logic_lever" => Some(telos_sim::logic::LogicKind::Lever { powered: false }),
                    "logic_lever_on" => Some(telos_sim::logic::LogicKind::Lever { powered: true }),
                    "logic_lamp" => Some(telos_sim::logic::LogicKind::Lamp { lit: false }),
                    "logic_lamp_lit" => Some(telos_sim::logic::LogicKind::Lamp { lit: true }),
                    "logic_repeater" => Some(telos_sim::logic::LogicKind::Repeater {
                        facing: telos_core::coords::Face::North,
                        delay: 1,
                        powered: false,
                    }),
                    "logic_repeater_powered" => Some(telos_sim::logic::LogicKind::Repeater {
                        facing: telos_core::coords::Face::North,
                        delay: 1,
                        powered: true,
                    }),
                    "logic_inverter" => Some(telos_sim::logic::LogicKind::Inverter {
                        facing: telos_core::coords::Face::North,
                        powered: true,
                    }),
                    "logic_inverter_off" => Some(telos_sim::logic::LogicKind::Inverter {
                        facing: telos_core::coords::Face::North,
                        powered: false,
                    }),
                    "logic_diode" => Some(telos_sim::logic::LogicKind::Diode {
                        facing: telos_core::coords::Face::North,
                    }),
                    _ => None,
                };
                if let Some(k) = kind {
                    self.logic_engine.set_component(pos, k);
                }
            }
        } else if old_flags.contains(StateFlags::LOGIC_COMPONENT) {
            self.logic_engine.remove_component(pos);
        }
    }

    /// Resolves the corresponding `BlockStateId` for a logic component.
    #[must_use]
    pub fn resolve_logic_block_state(
        &self,
        _current: BlockStateId,
        comp: &telos_sim::logic::LogicComponent,
    ) -> Option<BlockStateId> {
        let name = match comp.kind {
            telos_sim::logic::LogicKind::Wire => {
                if comp.power > 0 {
                    "logic_wire_powered"
                } else {
                    "logic_wire"
                }
            }
            telos_sim::logic::LogicKind::PowerBlock => "logic_power_block",
            telos_sim::logic::LogicKind::Lever { powered } => {
                if powered {
                    "logic_lever_on"
                } else {
                    "logic_lever"
                }
            }
            telos_sim::logic::LogicKind::Lamp { lit } => {
                if lit {
                    "logic_lamp_lit"
                } else {
                    "logic_lamp"
                }
            }
            telos_sim::logic::LogicKind::Repeater { powered, .. } => {
                if powered {
                    "logic_repeater_powered"
                } else {
                    "logic_repeater"
                }
            }
            telos_sim::logic::LogicKind::Inverter { powered, .. } => {
                if powered {
                    "logic_inverter"
                } else {
                    "logic_inverter_off"
                }
            }
            telos_sim::logic::LogicKind::Diode { .. } => "logic_diode",
        };

        let ident = telos_core::ident::Identifier::new("telos", name).ok()?;
        self.registry
            .get(&ident)
            .map(telos_voxel::Block::default_state)
    }

    /// Advances the logic simulation engine by one tick, updating component states.
    /// Returns all block positions whose block state changed.
    pub fn tick_logic(&mut self, current_tick: u64) -> Vec<(BlockPos, BlockStateId)> {
        let changes = self.logic_engine.tick(current_tick);
        let mut updated = Vec::new();

        for (pos, comp) in changes {
            let cur_state = self.get_block(pos);
            if let Some(target) = self.resolve_logic_block_state(cur_state, &comp)
                && target != cur_state
            {
                let old_flags = self.registry.flags(cur_state);
                let new_flags = self.registry.flags(target);
                let (chunk_pos, local_idx) = split_block_pos(pos);
                if let Some(sc) = self.chunks.get_mut(&chunk_pos) {
                    sc.chunk
                        .set(local_idx, target, old_flags, new_flags, current_tick.max(1));
                    let snapshot = sc.chunk.publish_snapshot();
                    sc.snapshot = snapshot;
                    self.dirty_chunks.insert(chunk_pos);
                    updated.push((pos, target));
                }
            }
        }

        updated
    }

    /// Advances the real-time cellular automata fluid simulation engine by one tick.
    /// Returns all block positions whose block state changed.
    pub fn tick_fluids(&mut self, current_tick: u64) -> Vec<(BlockPos, BlockStateId)> {
        let mut engine = std::mem::take(&mut self.fluid_engine);
        let changes = engine.tick(current_tick, self);
        self.fluid_engine = engine;

        let mut updated = Vec::new();
        for (pos, target) in changes {
            let cur_state = self.get_block(pos);
            if target != cur_state {
                let old_flags = self.registry.flags(cur_state);
                let new_flags = self.registry.flags(target);
                let (chunk_pos, local_idx) = split_block_pos(pos);
                if let Some(sc) = self.chunks.get_mut(&chunk_pos) {
                    sc.chunk
                        .set(local_idx, target, old_flags, new_flags, current_tick.max(1));
                    let snapshot = sc.chunk.publish_snapshot();
                    sc.snapshot = snapshot;
                    self.dirty_chunks.insert(chunk_pos);
                    self.invalidate_lod_hierarchy(chunk_pos);
                    updated.push((pos, target));
                }
            }
        }

        updated
    }

    /// Advances random block ticks (e.g. leaf decay, plant growth) for active loaded chunks.
    ///
    /// Evaluates `ticks_per_chunk` pseudo-random coordinates per chunk. If an orphaned leaf is
    /// discovered, it is decayed into air and returned for network broadcast and particle emission.
    pub fn tick_random_blocks(
        &mut self,
        current_tick: u64,
        ticks_per_chunk: usize,
    ) -> Vec<(BlockPos, BlockStateId)> {
        if self.chunks.is_empty() || ticks_per_chunk == 0 {
            return Vec::new();
        }

        let mut to_decay = Vec::new();

        // 1. Collect candidate block positions that require decay
        for (&chunk_pos, sc) in &self.chunks {
            for i in 0..ticks_per_chunk {
                // Fast deterministic pseudo-random local index sampling
                let h = telos_worldgen::math::hash3(
                    current_tick.wrapping_add(i as u64),
                    chunk_pos.x(),
                    chunk_pos.y(),
                    chunk_pos.z(),
                );
                let lx = (h & 0x1F) as u32;
                let ly = ((h >> 5) & 0x1F) as u32;
                let lz = ((h >> 10) & 0x1F) as u32;

                let local_idx = telos_voxel::coords::LocalIdx::from_coords(lx, ly, lz).unwrap();
                let state = sc.chunk.get(local_idx);
                let flags = self.registry.flags(state);

                if flags.contains(telos_voxel::state::StateFlags::TICKABLE)
                    && self.registry.is_leaves(state)
                {
                    let pos = BlockPos::new(
                        chunk_pos.x() * 32 + lx.cast_signed(),
                        chunk_pos.y() * 32 + ly.cast_signed(),
                        chunk_pos.z() * 32 + lz.cast_signed(),
                    );
                    if telos_sim::foliage::is_leaf_decaying(pos, 4, self) {
                        to_decay.push(pos);
                    }
                }
            }
        }

        // 2. Apply decay to air
        let mut updated = Vec::new();
        for pos in to_decay {
            let (chunk_pos, local_idx) = split_block_pos(pos);
            if let Some(sc) = self.chunks.get_mut(&chunk_pos) {
                let cur = sc.chunk.get(local_idx);
                if self.registry.is_leaves(cur) {
                    let old_flags = self.registry.flags(cur);
                    let new_flags = telos_voxel::state::StateFlags::AIR;
                    sc.chunk.set(
                        local_idx,
                        BlockStateId::AIR,
                        old_flags,
                        new_flags,
                        current_tick.max(1),
                    );
                    let snapshot = sc.chunk.publish_snapshot();
                    sc.snapshot = snapshot;
                    self.dirty_chunks.insert(chunk_pos);
                    self.invalidate_lod_hierarchy(chunk_pos);
                    updated.push((pos, BlockStateId::AIR));
                }
            }
        }

        updated
    }

    /// Invalidates LOD pyramid nodes and cached meshes containing the given chunk.
    pub fn invalidate_lod_hierarchy(&mut self, chunk_pos: ChunkPos) {
        let mut key = LodNodeKey::from_chunk(chunk_pos).parent();
        for _ in 1..=4 {
            self.pyramid.remove(&key);
            self.lod_meshes.remove(&key);
            key = key.parent();
        }
    }

    /// Saves all dirty chunks to `.tlr` region files if storage is configured.
    /// Returns the number of chunks saved.
    pub fn save_dirty_chunks(&mut self) -> Result<usize, telos_storage::StorageError> {
        let Some(storage) = &mut self.storage else {
            self.dirty_chunks.clear();
            return Ok(0);
        };

        if self.dirty_chunks.is_empty() {
            return Ok(0);
        }

        let chunks_to_save: Vec<(ChunkPos, &Chunk)> = self
            .dirty_chunks
            .iter()
            .filter_map(|pos| self.chunks.get(pos).map(|sc| (*pos, &sc.chunk)))
            .collect();

        let count = storage.save_chunks(chunks_to_save)?;
        self.dirty_chunks.clear();
        Ok(count)
    }

    /// Flushes all open region files to disk.
    pub fn flush_storage(&mut self) -> Result<(), telos_storage::StorageError> {
        if let Some(storage) = &mut self.storage {
            storage.flush()?;
        }
        Ok(())
    }

    /// Returns the number of currently cached chunks.
    #[must_use]
    pub fn cached_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Returns `true` if the chunk at `pos` is already generated or loaded in memory.
    #[must_use]
    pub fn contains_chunk(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    /// Returns the number of currently cached LOD nodes.
    #[must_use]
    pub fn cached_lod_count(&self) -> usize {
        self.pyramid.len()
    }

    /// Ensures that an LOD node is downsampled in the pyramid, generating chunks/children recursively as needed.
    pub fn ensure_lod_node(&mut self, key: LodNodeKey) -> Arc<LodNode> {
        if let Some(node) = self.pyramid.get(&key) {
            return node;
        }

        if key.level == 1 {
            let mut snapshots = Vec::with_capacity(8);
            for oz in 0..2 {
                for oy in 0..2 {
                    for ox in 0..2 {
                        let child_pos =
                            ChunkPos::new(key.x * 2 + ox, key.y * 2 + oy, key.z * 2 + oz);
                        snapshots.push(self.get_or_generate_chunk(child_pos));
                    }
                }
            }
            let children: [Option<&Arc<ChunkSnapshot>>; 8] = [
                Some(&snapshots[0]),
                Some(&snapshots[1]),
                Some(&snapshots[2]),
                Some(&snapshots[3]),
                Some(&snapshots[4]),
                Some(&snapshots[5]),
                Some(&snapshots[6]),
                Some(&snapshots[7]),
            ];
            self.pyramid.downsample_and_store(key, &children)
        } else {
            let mut children_nodes = Vec::with_capacity(8);
            for oz in 0..2 {
                for oy in 0..2 {
                    for ox in 0..2 {
                        let child_key = LodNodeKey::new(
                            key.level - 1,
                            key.x * 2 + ox,
                            key.y * 2 + oy,
                            key.z * 2 + oz,
                        );
                        children_nodes.push(self.ensure_lod_node(child_key));
                    }
                }
            }
            let children: [Option<&Arc<LodNode>>; 8] = [
                Some(&children_nodes[0]),
                Some(&children_nodes[1]),
                Some(&children_nodes[2]),
                Some(&children_nodes[3]),
                Some(&children_nodes[4]),
                Some(&children_nodes[5]),
                Some(&children_nodes[6]),
                Some(&children_nodes[7]),
            ];
            self.pyramid.downsample_and_store(key, &children)
        }
    }

    /// Gets or builds a greedy-meshed LOD mesh for a given LOD node key.
    pub fn get_or_mesh_lod_node(&mut self, key: LodNodeKey) -> Arc<LodMesh> {
        if let Some(mesh) = self.lod_meshes.get(&key) {
            return mesh.clone();
        }

        let node = self.ensure_lod_node(key);
        let neighbors: [Option<&LodNode>; 6] = [None; 6];
        let mesh = mesh_lod_node(node.as_ref(), &neighbors, &self.lod_color_table);
        let arc = Arc::new(mesh);
        self.lod_meshes.insert(key, arc.clone());
        arc
    }

    /// Returns the world Y coordinate of the highest solid block at (x, z), or 64 as fallback.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub fn get_surface_y(&self, x: i32, z: i32) -> i32 {
        let chunk_x = x.div_euclid(32);
        let chunk_z = z.div_euclid(32);
        let local_x = x.rem_euclid(32) as u32;
        let local_z = z.rem_euclid(32) as u32;
        if let Some(col) = self.columns.get(&(chunk_x, chunk_z)) {
            let top = col.get_top(local_x, local_z);
            if top > i16::MIN {
                return i32::from(top);
            }
        }
        64
    }

    /// Discovers the highest safe surface ground at or around `(target_x, target_z)`.
    ///
    /// Searches the column and nearby spiral coordinates, evaluating cubic chunks from mountain
    /// peaks down to bedrock, finding the highest solid, non-hazardous surface block
    /// with at least 2 blocks of clear headroom above it.
    #[allow(clippy::cast_possible_wrap, clippy::cast_precision_loss)]
    pub fn find_safe_spawn(&mut self, target_x: i32, target_z: i32) -> DVec3 {
        // Spiral offsets around target_x, target_z (center first, then expanding outward up to 64 blocks)
        let mut search_offsets = Vec::with_capacity(128);
        search_offsets.push((0, 0));
        for r in (4..=64).step_by(8) {
            search_offsets.push((0, r));
            search_offsets.push((r, 0));
            search_offsets.push((0, -r));
            search_offsets.push((-r, 0));
            search_offsets.push((r, r));
            search_offsets.push((-r, r));
            search_offsets.push((r, -r));
            search_offsets.push((-r, -r));
            search_offsets.push((r / 2, r));
            search_offsets.push((r, r / 2));
            search_offsets.push((-r / 2, -r));
            search_offsets.push((-r, -r / 2));
        }

        'column_search: for (dx, dz) in search_offsets {
            let x = target_x + dx;
            let z = target_z + dz;
            let cx = x.div_euclid(32);
            let cz = z.div_euclid(32);
            let lx = x.rem_euclid(32) as usize;
            let lz = z.rem_euclid(32) as usize;

            // Scan vertically from mountain heights (Y ~ 320, cy = 10) down to sea level/underground (cy = -2, Y = -64)
            for cy in (-2..=10).rev() {
                let snap = self.get_or_generate_chunk(ChunkPos::new(cx, cy, cz));
                let blocks = snap.blocks();

                // Scan within this 32-block chunk from top down to 0
                for ly in (0..32usize).rev() {
                    let idx = LocalIdx::from_coords_unchecked(lx as u32, ly as u32, lz as u32);
                    let block = blocks.get(idx);

                    if block.is_air() {
                        continue;
                    }

                    // If fluid (water or lava) or hazard is encountered at the top of the column, this column isn't dry land
                    if self.registry.is_fluid(block)
                        || self.registry.is_lava(block)
                        || self.registry.is_fire(block)
                    {
                        continue 'column_search;
                    }

                    let wy = cy * 32 + ly as i32;

                    // Must be a solid block (e.g. grass, dirt, stone, sand)
                    let flags = self.registry.flags(block);
                    let is_solid = flags.contains(StateFlags::OPAQUE_FULL)
                        || *self.registry.shape(block) == telos_voxel::shape::BlockShape::Cube;
                    let is_hazardous = self.registry.is_magma_block(block)
                        || self.registry.is_fire(block)
                        || self.registry.is_lava(block);

                    if is_hazardous {
                        continue 'column_search;
                    }

                    if !is_solid {
                        // Passable block on surface (e.g. snow layer, tall grass, poppy)
                        let below_block = self.get_block(BlockPos::new(x, wy - 1, z));
                        let below_flags = self.registry.flags(below_block);
                        let below_solid = below_flags.contains(StateFlags::OPAQUE_FULL)
                            || *self.registry.shape(below_block) == telos_voxel::shape::BlockShape::Cube;
                        if below_solid && !self.registry.is_magma_block(below_block) {
                            let headroom_2 = self.get_block(BlockPos::new(x, wy + 1, z));
                            let h2_passable = headroom_2.is_air()
                                || (!self.registry.is_fluid(headroom_2)
                                    && !self.registry.flags(headroom_2).contains(StateFlags::OPAQUE_FULL));
                            if h2_passable {
                                let spawn_y = f64::from(wy);
                                return DVec3::new(f64::from(x) + 0.5, spawn_y, f64::from(z) + 0.5);
                            }
                        }
                        continue 'column_search;
                    }

                    // Verify 2 blocks of clear headroom above (wy + 1, wy + 2)
                    let headroom_1 = self.get_block(BlockPos::new(x, wy + 1, z));
                    let headroom_2 = self.get_block(BlockPos::new(x, wy + 2, z));

                    let h1_passable = headroom_1.is_air()
                        || (!self.registry.is_fluid(headroom_1)
                            && !self
                                .registry
                                .flags(headroom_1)
                                .contains(StateFlags::OPAQUE_FULL));
                    let h2_passable = headroom_2.is_air()
                        || (!self.registry.is_fluid(headroom_2)
                            && !self
                                .registry
                                .flags(headroom_2)
                                .contains(StateFlags::OPAQUE_FULL));

                    if h1_passable && h2_passable {
                        let spawn_y = f64::from(wy) + 1.0;
                        return DVec3::new(f64::from(x) + 0.5, spawn_y, f64::from(z) + 0.5);
                    }

                    // Top-most solid block lacked safe headroom; do not descend into caves
                    continue 'column_search;
                }
            }
        }

        // Fallback for void or empty worlds where search did not find standard terrain
        let fallback_y = self.get_surface_y(target_x, target_z).max(65);
        DVec3::new(f64::from(target_x) + 0.5, f64::from(fallback_y) + 1.0, f64::from(target_z) + 0.5)
    }

    /// Returns (`sky_light`, `block_light`) at pos.
    #[must_use]
    pub fn get_light(&self, pos: BlockPos) -> (u8, u8) {
        let (cpos, lpos) = split_block_pos(pos);
        if let Some(sc) = self.chunks.get(&cpos)
            && let Some(light) = sc.snapshot.light()
        {
            return (
                light.get_sky(lpos.as_usize()),
                light.get_block(lpos.as_usize()),
            );
        }
        (15, 0)
    }

    /// Propagates sky light from already generated cardinal neighbor chunks across boundaries.
    #[allow(clippy::too_many_lines, clippy::cast_possible_wrap)]
    fn diffuse_cross_chunk_boundary<F>(
        chunks: &HashMap<ChunkPos, ServerChunk>,
        pos: ChunkPos,
        chunk_light: &mut ChunkLight,
        light_bfs: &mut LightBfs,
        mut is_opaque: F,
    ) where
        F: FnMut(usize) -> u8,
    {
        let neighbors = [
            (ChunkPos::new(pos.x() - 1, pos.y(), pos.z()), -1, 0, 0),
            (ChunkPos::new(pos.x() + 1, pos.y(), pos.z()), 1, 0, 0),
            (ChunkPos::new(pos.x(), pos.y() - 1, pos.z()), 0, -1, 0),
            (ChunkPos::new(pos.x(), pos.y() + 1, pos.z()), 0, 1, 0),
            (ChunkPos::new(pos.x(), pos.y(), pos.z() - 1), 0, 0, -1),
            (ChunkPos::new(pos.x(), pos.y(), pos.z() + 1), 0, 0, 1),
        ];

        let mut seeded = false;
        for (n_pos, dx, dy, dz) in neighbors {
            if let Some(sc) = chunks.get(&n_pos)
                && let Some(nl) = sc.chunk.light()
            {
                if dx == -1 {
                    for y in 0..32u32 {
                        for z in 0..32u32 {
                            let n_idx = ((y << 10) | (z << 5) | 31) as usize;
                            let c_idx = ((y << 10) | (z << 5)) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(0, y as i32, z as i32);
                                    seeded = true;
                                }
                            }
                        }
                    }
                } else if dx == 1 {
                    for y in 0..32u32 {
                        for z in 0..32u32 {
                            let n_idx = ((y << 10) | (z << 5)) as usize;
                            let c_idx = ((y << 10) | (z << 5) | 31) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(31, y as i32, z as i32);
                                    seeded = true;
                                }
                            }
                        }
                    }
                } else if dy == -1 {
                    for x in 0..32u32 {
                        for z in 0..32u32 {
                            let n_idx = ((31 << 10) | (z << 5) | x) as usize;
                            let c_idx = ((z << 5) | x) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(x as i32, 0, z as i32);
                                    seeded = true;
                                }
                            }
                        }
                    }
                } else if dy == 1 {
                    for x in 0..32u32 {
                        for z in 0..32u32 {
                            let n_idx = ((z << 5) | x) as usize;
                            let c_idx = ((31 << 10) | (z << 5) | x) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(x as i32, 31, z as i32);
                                    seeded = true;
                                }
                            }
                        }
                    }
                } else if dz == -1 {
                    for y in 0..32u32 {
                        for x in 0..32u32 {
                            let n_idx = ((y << 10) | (31 << 5) | x) as usize;
                            let c_idx = ((y << 10) | x) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(x as i32, y as i32, 0);
                                    seeded = true;
                                }
                            }
                        }
                    }
                } else if dz == 1 {
                    for y in 0..32u32 {
                        for x in 0..32u32 {
                            let n_idx = ((y << 10) | x) as usize;
                            let c_idx = ((y << 10) | (31 << 5) | x) as usize;
                            let n_sky = nl.sky.get(n_idx);
                            if n_sky > 1 && is_opaque(c_idx) == 0 {
                                let target = n_sky - 1;
                                if chunk_light.sky.get(c_idx) < target {
                                    chunk_light.sky.set(c_idx, target);
                                    light_bfs.enqueue_add(x as i32, y as i32, 31);
                                    seeded = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if seeded {
            light_bfs.propagate_block_add(&mut chunk_light.sky, is_opaque);
        }
    }
}

impl telos_sim::fluid::FluidWorldReader for ServerWorld {
    fn get_block(&self, pos: BlockPos) -> BlockStateId {
        self.get_loaded_block(pos)
    }

    fn registry(&self) -> &BlockRegistry {
        &self.registry
    }
}

impl telos_sim::NavWorldReader for ServerWorld {
    fn get_block(&self, pos: BlockPos) -> BlockStateId {
        self.get_loaded_block(pos)
    }

    fn registry(&self) -> &BlockRegistry {
        &self.registry
    }
}
