//! Server world persistence using `.tlr` region files via `telos-storage`.

use hashbrown::HashMap;
use std::fs::{OpenOptions, create_dir_all};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use telos_content::{FrozenRegistries, RegistryRemap, WorldRegistryMap};
use telos_core::coords::ChunkPos;
use telos_storage::format::header::CodecId;
use telos_storage::format::section::{ChunkPayload, ChunkStatus};
use telos_storage::io::{FileRegionIo, RegionIo};
use telos_storage::region::{RegionFile, RegionPos};
use telos_voxel::chunk::Chunk;
use telos_voxel::registry::BlockRegistry;
use telos_voxel::state::BlockStateId;
use tracing::{debug, error};

/// Server storage backend managing cubic `.tlr` region containers.
pub struct WorldStorage {
    regions_dir: PathBuf,
    regions: HashMap<RegionPos, RegionFile<FileRegionIo>>,
    save_map: Option<WorldRegistryMap>,
    remap: Option<RegistryRemap>,
    compression_enabled: bool,
    compression_level: u32,
}

impl WorldStorage {
    /// Initializes `WorldStorage` at the given root directory (e.g. `saves/world`) without save-ID remapping.
    pub fn new(root_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::new_with_compression(root_dir, true, 3)
    }

    /// Initializes `WorldStorage` at the given root directory with explicit compression settings.
    pub fn new_with_compression(
        root_dir: impl AsRef<Path>,
        compression_enabled: bool,
        compression_level: u32,
    ) -> std::io::Result<Self> {
        let regions_dir = root_dir.as_ref().join("regions");
        create_dir_all(&regions_dir)?;
        let mut storage = Self {
            regions_dir,
            regions: HashMap::new(),
            save_map: None,
            remap: None,
            compression_enabled,
            compression_level,
        };
        let _ = storage.prune_empty_regions();
        Ok(storage)
    }

    /// Initializes `WorldStorage` at root directory with full save-ID mapping and palette remapping.
    pub fn open_or_create(
        root_dir: impl AsRef<Path>,
        registries: &FrozenRegistries,
    ) -> std::io::Result<Self> {
        Self::open_or_create_with_compression(root_dir, registries, true, 3)
    }

    /// Initializes `WorldStorage` at root directory with full save-ID mapping and custom compression settings.
    pub fn open_or_create_with_compression(
        root_dir: impl AsRef<Path>,
        registries: &FrozenRegistries,
        compression_enabled: bool,
        compression_level: u32,
    ) -> std::io::Result<Self> {
        let root = root_dir.as_ref();
        let regions_dir = root.join("regions");
        create_dir_all(&regions_dir)?;

        let save_map = WorldRegistryMap::open_or_create(root, registries)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let remap = save_map.build_remap(registries);

        let mut storage = Self {
            regions_dir,
            regions: HashMap::new(),
            save_map: Some(save_map),
            remap: Some(remap),
            compression_enabled,
            compression_level,
        };
        let _ = storage.prune_empty_regions();
        Ok(storage)
    }

    /// Sets the compression configuration for newly opened or committed region files.
    pub fn set_compression(&mut self, enabled: bool, level: u32) {
        self.compression_enabled = enabled;
        self.compression_level = level;
        for region in self.regions.values_mut() {
            region.set_compression(enabled, level);
        }
    }

    /// Returns whether compression is enabled for this world storage.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.compression_enabled
    }

    /// Returns the compression level for this world storage.
    #[must_use]
    pub fn compression_level(&self) -> u32 {
        self.compression_level
    }

    /// Returns a reference to the active `WorldRegistryMap`, if initialized.
    #[must_use]
    pub fn save_map(&self) -> Option<&WorldRegistryMap> {
        self.save_map.as_ref()
    }

    /// Retrieves an open region file, or opens it if it exists.
    /// If `create` is true, creates a new region file on disk if it does not already exist.
    fn get_or_open_region(
        &mut self,
        rpos: RegionPos,
        create: bool,
    ) -> Result<Option<&mut RegionFile<FileRegionIo>>, telos_storage::StorageError> {
        if !self.regions.contains_key(&rpos) {
            let filename = rpos.filename();
            let path = self.regions_dir.join(filename);
            if !create {
                if !path.is_file() {
                    return Ok(None);
                }
                if let Ok(metadata) = std::fs::metadata(&path)
                    && (metadata.len() == 0 || metadata.len() == 40960)
                {
                    // Empty region file on disk with 0 saved chunks
                    return Ok(None);
                }
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(create)
                .truncate(false)
                .open(&path)
                .map_err(telos_storage::StorageError::Io)?;
            let io = FileRegionIo::new(file);
            let region = RegionFile::open_with_compression(
                io,
                rpos.x,
                rpos.y,
                rpos.z,
                self.compression_enabled,
                self.compression_level,
            )?;
            self.regions.insert(rpos, region);
        }
        Ok(self.regions.get_mut(&rpos))
    }

    /// Prunes any 0-byte or empty region files (containing 0 stored chunks) from disk.
    ///
    /// Returns the number of pruned empty region files.
    pub fn prune_empty_regions(&mut self) -> Result<usize, telos_storage::StorageError> {
        let mut pruned = 0;
        let read_dir = match std::fs::read_dir(&self.regions_dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(telos_storage::StorageError::Io(e)),
        };

        // First, drop any cached regions that are empty so file locks/handles are closed.
        let mut empty_cached = Vec::new();
        for (rpos, region) in &self.regions {
            if region.is_empty() {
                empty_cached.push(*rpos);
            }
        }
        for rpos in empty_cached {
            self.regions.remove(&rpos);
        }

        for entry in read_dir {
            let entry = entry.map_err(telos_storage::StorageError::Io)?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("tlr") {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let len = metadata.len();
                if len == 0 || len == 40960 {
                    let _ = std::fs::remove_file(&path);
                    pruned += 1;
                    continue;
                }
                if let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) {
                    let io = FileRegionIo::new(file);
                    if let Ok(region) = RegionFile::open(io, 0, 0, 0)
                        && region.is_empty()
                    {
                        drop(region);
                        let _ = std::fs::remove_file(&path);
                        pruned += 1;
                    }
                }
            }
        }
        Ok(pruned)
    }

    /// Attempts to load a chunk from disk.
    /// Returns `Ok(Some(chunk))` if found, or `Ok(None)` if chunk is not yet saved.
    pub fn load_chunk(
        &mut self,
        pos: ChunkPos,
        registry: &BlockRegistry,
    ) -> Result<Option<Chunk>, telos_storage::StorageError> {
        let rpos = RegionPos::from_chunk(pos);
        let Some(region) = self.get_or_open_region(rpos, false)? else {
            return Ok(None);
        };
        let maybe_payload = region.read_chunk(pos)?;
        let Some(payload) = maybe_payload else {
            return Ok(None);
        };

        let blocks = if let Some(ref remap) = self.remap {
            remap.remap_blocks_save_to_runtime(payload.blocks)
        } else {
            payload.blocks
        };

        let is_opaque = |id: BlockStateId| {
            registry
                .flags(id)
                .contains(telos_voxel::state::StateFlags::OPAQUE_FULL)
        };
        let mut chunk = Chunk::from_blocks(pos, blocks, is_opaque);
        chunk.block_entities = payload.block_entities;
        Ok(Some(chunk))
    }

    /// Commits a collection of modified chunks to their respective region files on disk.
    pub fn save_chunks<'a, I>(&mut self, chunks: I) -> Result<usize, telos_storage::StorageError>
    where
        I: IntoIterator<Item = (ChunkPos, &'a Chunk)>,
    {
        let mut grouped: HashMap<RegionPos, Vec<(ChunkPos, Option<ChunkPayload>)>> = HashMap::new();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);

        let mut count = 0;
        for (pos, chunk) in chunks {
            let rpos = RegionPos::from_chunk(pos);
            let raw_blocks = chunk.to_blocks();
            let disk_blocks = if let Some(ref remap) = self.remap {
                remap.remap_blocks_runtime_to_save(&raw_blocks)
            } else {
                raw_blocks
            };
            let payload = ChunkPayload::with_block_entities(
                disk_blocks,
                ChunkStatus::default(),
                chunk.block_entities.clone(),
            );
            grouped.entry(rpos).or_default().push((pos, Some(payload)));
            count += 1;
        }

        for (rpos, chunk_list) in grouped {
            let Some(region) = self.get_or_open_region(rpos, true)? else {
                continue;
            };
            region.commit_chunks(&chunk_list, CodecId::Zstd, timestamp)?;
            debug!("Committed {} chunks to region {:?}", chunk_list.len(), rpos);
        }

        Ok(count)
    }

    /// Flushes all open region files to durable storage via `fsync`.
    pub fn flush(&mut self) -> Result<(), telos_storage::StorageError> {
        for (rpos, region) in &mut self.regions {
            if let Err(err) = region.io_mut().sync_data() {
                error!("Failed to sync region {:?}: {err}", rpos);
                return Err(err);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_storage_does_not_create_empty_region_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut storage = WorldStorage::new(temp.path()).expect("WorldStorage::new");
        let reg = BlockRegistry::new();

        // Querying non-existent chunks across different regions must NOT create region files
        for rx in -2..=2 {
            for rz in -2..=2 {
                let pos = ChunkPos::new(rx * 8, 0, rz * 8);
                let loaded = storage.load_chunk(pos, &reg).expect("load_chunk");
                assert!(loaded.is_none());
            }
        }

        // Verify regions directory has zero files
        let count = std::fs::read_dir(temp.path().join("regions"))
            .expect("read_dir")
            .count();
        assert_eq!(count, 0, "No region files should be created by load_chunk");
    }

    #[test]
    fn test_prune_empty_regions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let regions_dir = temp.path().join("regions");
        std::fs::create_dir_all(&regions_dir).expect("create_dir_all");

        // Create an empty dummy 40KiB file (mimicking unallocated region container)
        let dummy_40k = regions_dir.join("r.0.0.0.tlr");
        std::fs::write(&dummy_40k, vec![0u8; 40960]).expect("write dummy");
        assert!(dummy_40k.is_file());

        let mut storage = WorldStorage::new(temp.path()).expect("new");
        // Storage::new already prunes, let's verify dummy is gone
        assert!(
            !dummy_40k.exists(),
            "Dummy 40KiB region file should have been pruned on new"
        );

        // Write a 0-byte file and check explicit prune
        let dummy_0 = regions_dir.join("r.1.1.1.tlr");
        std::fs::write(&dummy_0, b"").expect("write 0-byte");
        assert!(dummy_0.is_file());
        let pruned = storage.prune_empty_regions().expect("prune");
        assert_eq!(pruned, 1);
        assert!(!dummy_0.exists());
    }
}
