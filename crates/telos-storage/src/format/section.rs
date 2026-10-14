//! Tagged section encoder/decoder and payload sections (BLOCKS, STATUS).

use crate::error::{Result, StorageError};
use telos_voxel::{
    block_entity::{
        BlockEntityData, BlockEntitySlot, BlockEntityTable, CHEST_CONTAINER_SLOTS,
        FURNACE_CONTAINER_SLOTS,
    },
    coords::LocalIdx,
    state::BlockStateId,
    storage::{Blocks, Packed},
};

/// Tag identifying the `BLOCKS` section.
pub const TAG_BLOCKS: u8 = 0x01;
/// Tag identifying the `SKY_LIGHT` section.
pub const TAG_SKY_LIGHT: u8 = 0x02;
/// Tag identifying the `BLOCK_LIGHT` section.
pub const TAG_BLOCK_LIGHT: u8 = 0x03;
/// Tag identifying the `BIOMES` section.
pub const TAG_BIOMES: u8 = 0x04;
/// Tag identifying the `BLOCK_ENTITIES` section.
pub const TAG_BLOCK_ENTITIES: u8 = 0x05;
/// Tag identifying the `STATUS` section.
pub const TAG_STATUS: u8 = 0x08;

/// Section flag bit indicating a critical section that unknown readers must reject.
pub const SFLAG_CRITICAL: u8 = 1 << 7;

/// Encodes an unsigned 32-bit integer as LEB128 varint.
pub fn encode_varint_u32(mut val: u32, buf: &mut Vec<u8>) {
    while val >= 0x80 {
        buf.push(((val & 0x7f) as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

/// Decodes an unsigned 32-bit integer from LEB128 varint.
pub fn decode_varint_u32(cursor: &mut &[u8]) -> Result<u32> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;

    for _ in 0..5 {
        if cursor.is_empty() {
            return Err(StorageError::CorruptPayload(
                "Unexpected EOF in varint u32".into(),
            ));
        }
        let byte = cursor[0];
        *cursor = &cursor[1..];

        result |= u32::from(byte & 0x7f) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
    }

    Err(StorageError::CorruptPayload(
        "Overflow in varint u32".into(),
    ))
}

/// Encodes an unsigned 64-bit integer as LEB128 varint.
pub fn encode_varint_u64(mut val: u64, buf: &mut Vec<u8>) {
    while val >= 0x80 {
        buf.push(((val & 0x7f) as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

/// Decodes an unsigned 64-bit integer from LEB128 varint.
pub fn decode_varint_u64(cursor: &mut &[u8]) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;

    for _ in 0..10 {
        if cursor.is_empty() {
            return Err(StorageError::CorruptPayload(
                "Unexpected EOF in varint u64".into(),
            ));
        }
        let byte = cursor[0];
        *cursor = &cursor[1..];

        result |= u64::from(byte & 0x7f) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
    }

    Err(StorageError::CorruptPayload(
        "Overflow in varint u64".into(),
    ))
}

/// Chunk status metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChunkStatus {
    /// Generation stage: 0=Empty, 1=Noise, 2=Surface, 3=Carvers, 4=Features, 5=Light, 6=Full.
    pub gen_stage: u8,
    /// Inhabited ticks by players.
    pub inhabited_ticks: u64,
    /// Bitflags for light validity, column estimation, etc.
    pub flags: u16,
}

impl ChunkStatus {
    /// Encodes status into binary payload.
    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.gen_stage);
        encode_varint_u64(self.inhabited_ticks, buf);
        buf.extend_from_slice(&self.flags.to_le_bytes());
    }

    /// Decodes status from binary payload.
    pub fn decode(mut cursor: &[u8]) -> Result<Self> {
        if cursor.is_empty() {
            return Err(StorageError::CorruptPayload("Empty status payload".into()));
        }
        let gen_stage = cursor[0];
        cursor = &cursor[1..];
        let inhabited_ticks = decode_varint_u64(&mut cursor)?;
        if cursor.len() < 2 {
            return Err(StorageError::CorruptPayload(
                "Truncated status flags".into(),
            ));
        }
        let flags = u16::from_le_bytes(cursor[0..2].try_into().unwrap());
        Ok(Self {
            gen_stage,
            inhabited_ticks,
            flags,
        })
    }
}

/// An unknown or unparsed section preserved for forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    /// Section tag identifier.
    pub tag: u8,
    /// Section flags (bit 7 = CRITICAL).
    pub sflags: u8,
    /// Raw unparsed byte payload.
    pub data: Vec<u8>,
}

/// Decoded chunk payload data containing blocks, status, block entities, and raw unknown sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPayload {
    /// Voxel block storage.
    pub blocks: Blocks,
    /// Generation status and metadata.
    pub status: ChunkStatus,
    /// Block entities side table.
    pub block_entities: BlockEntityTable,
    /// Forward-compatible preserved sections.
    pub unknown_sections: Vec<RawSection>,
}

impl ChunkPayload {
    /// Creates a new `ChunkPayload` with the given blocks and status.
    #[must_use]
    pub fn new(blocks: Blocks, status: ChunkStatus) -> Self {
        Self {
            blocks,
            status,
            block_entities: BlockEntityTable::new(),
            unknown_sections: Vec::new(),
        }
    }

    /// Creates a new `ChunkPayload` with blocks, status, and block entities.
    #[must_use]
    pub fn with_block_entities(
        blocks: Blocks,
        status: ChunkStatus,
        block_entities: BlockEntityTable,
    ) -> Self {
        Self {
            blocks,
            status,
            block_entities,
            unknown_sections: Vec::new(),
        }
    }

    /// Encodes all sections into uncompressed payload bytes with canonical tag ordering.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16384);
        self.encode_into(&mut out);
        out
    }

    /// Encodes all sections directly into the provided output buffer.
    pub fn encode_into(&self, out: &mut Vec<u8>) {
        if self.unknown_sections.is_empty() {
            // Fast path: known sections (BLOCKS 0x01, optional BLOCK_ENTITIES 0x05, STATUS 0x08)
            // are strictly canonical in ascending order.
            let has_be = !self.block_entities.is_empty();
            let section_count = 2 + u32::from(has_be);
            encode_varint_u32(section_count, out);

            // 1. BLOCKS section (0x01)
            out.push(TAG_BLOCKS);
            out.push(0); // sflags
            let blocks_len = blocks_section_len(&self.blocks);
            encode_varint_u32(blocks_len as u32, out);
            encode_blocks(&self.blocks, out);

            // 2. BLOCK_ENTITIES section (0x05)
            if has_be {
                out.push(TAG_BLOCK_ENTITIES);
                out.push(0);
                let mut be_buf = Vec::new();
                encode_block_entities(&self.block_entities, &mut be_buf);
                encode_varint_u32(be_buf.len() as u32, out);
                out.extend_from_slice(&be_buf);
            }

            // 3. STATUS section (0x08)
            out.push(TAG_STATUS);
            out.push(0);
            let status_len = 1 + varint_u64_len(self.status.inhabited_ticks) + 2;
            encode_varint_u32(status_len as u32, out);
            self.status.encode(out);
            return;
        }

        // Generic fallback when unknown_sections are present
        let mut sections: Vec<(u8, u8, Vec<u8>)> = Vec::new();

        let mut blocks_buf = Vec::new();
        encode_blocks(&self.blocks, &mut blocks_buf);
        sections.push((TAG_BLOCKS, 0, blocks_buf));

        if !self.block_entities.is_empty() {
            let mut be_buf = Vec::new();
            encode_block_entities(&self.block_entities, &mut be_buf);
            sections.push((TAG_BLOCK_ENTITIES, 0, be_buf));
        }

        let mut status_buf = Vec::new();
        self.status.encode(&mut status_buf);
        sections.push((TAG_STATUS, 0, status_buf));

        for s in &self.unknown_sections {
            sections.push((s.tag, s.sflags, s.data.clone()));
        }

        sections.sort_by_key(|&(tag, _, _)| tag);

        encode_varint_u32(sections.len() as u32, out);

        for (tag, sflags, data) in sections {
            out.push(tag);
            out.push(sflags);
            encode_varint_u32(data.len() as u32, out);
            out.extend_from_slice(&data);
        }
    }

    /// Decodes chunk payload from uncompressed bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut cursor = bytes;
        let section_count = decode_varint_u32(&mut cursor)?;

        let mut blocks: Option<Blocks> = None;
        let mut status: Option<ChunkStatus> = None;
        let mut block_entities: Option<BlockEntityTable> = None;
        let mut unknown_sections = Vec::new();

        for _ in 0..section_count {
            if cursor.len() < 2 {
                return Err(StorageError::CorruptPayload(
                    "Truncated section header".into(),
                ));
            }
            let tag = cursor[0];
            let sflags = cursor[1];
            cursor = &cursor[2..];

            let len = decode_varint_u32(&mut cursor)? as usize;
            if cursor.len() < len {
                return Err(StorageError::Truncated {
                    actual: cursor.len() as u64,
                    expected: len as u64,
                });
            }

            let section_data = &cursor[..len];
            cursor = &cursor[len..];

            match tag {
                TAG_BLOCKS => {
                    blocks = Some(decode_blocks(section_data)?);
                }
                TAG_BLOCK_ENTITIES => {
                    block_entities = Some(decode_block_entities(section_data)?);
                }
                TAG_STATUS => {
                    status = Some(ChunkStatus::decode(section_data)?);
                }
                _ => {
                    if (sflags & SFLAG_CRITICAL) != 0 {
                        return Err(StorageError::CorruptPayload(format!(
                            "Unrecognized critical section tag {tag:#04x}"
                        )));
                    }
                    unknown_sections.push(RawSection {
                        tag,
                        sflags,
                        data: section_data.to_vec(),
                    });
                }
            }
        }

        let blocks = blocks.ok_or_else(|| {
            StorageError::CorruptPayload("Missing required BLOCKS section".into())
        })?;
        let status = status.unwrap_or_default();
        let block_entities = block_entities.unwrap_or_default();

        Ok(Self {
            blocks,
            status,
            block_entities,
            unknown_sections,
        })
    }
}

#[inline]
pub(crate) const fn varint_u32_len(val: u32) -> usize {
    if val < (1 << 7) {
        1
    } else if val < (1 << 14) {
        2
    } else if val < (1 << 21) {
        3
    } else if val < (1 << 28) {
        4
    } else {
        5
    }
}

#[inline]
pub(crate) const fn varint_u64_len(mut val: u64) -> usize {
    let mut len = 1;
    while val >= 0x80 {
        len += 1;
        val >>= 7;
    }
    len
}

#[inline]
fn blocks_section_len(blocks: &Blocks) -> usize {
    match blocks {
        Blocks::Uniform(state) => 2 + varint_u32_len(state.as_u32()),
        Blocks::Packed(packed) => {
            let mut len = 1 + varint_u32_len(packed.palette().len() as u32);
            for s in packed.palette() {
                len += varint_u32_len(s.as_u32());
            }
            len += packed.words().len() * 8;
            len
        }
    }
}

/// Encodes `Blocks` container into the binary BLOCKS section format.
pub fn encode_blocks(blocks: &Blocks, buf: &mut Vec<u8>) {
    match blocks {
        Blocks::Uniform(state) => {
            buf.push(0); // 0 bits
            encode_varint_u32(1, buf); // palette_len = 1
            encode_varint_u32(state.as_u32(), buf);
        }
        Blocks::Packed(packed) => {
            let bits = packed.bits() as u8;
            buf.push(bits);

            let palette = packed.palette();
            encode_varint_u32(palette.len() as u32, buf);
            for state in palette {
                encode_varint_u32(state.as_u32(), buf);
            }

            #[cfg(target_endian = "little")]
            {
                let words = packed.words();
                let bytes = unsafe {
                    std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), words.len() * 8)
                };
                buf.extend_from_slice(bytes);
            }
            #[cfg(not(target_endian = "little"))]
            {
                for &word in packed.words() {
                    buf.extend_from_slice(&word.to_le_bytes());
                }
            }
        }
    }
}

/// Decodes binary BLOCKS section into `Blocks`.
pub fn decode_blocks(mut cursor: &[u8]) -> Result<Blocks> {
    if cursor.is_empty() {
        return Err(StorageError::CorruptPayload("Empty BLOCKS section".into()));
    }

    let bits = cursor[0];
    cursor = &cursor[1..];

    if bits == 0 {
        let palette_len = decode_varint_u32(&mut cursor)?;
        if palette_len != 1 {
            return Err(StorageError::CorruptPayload(format!(
                "Uniform blocks section must have palette_len=1, got {palette_len}"
            )));
        }
        let state_id = decode_varint_u32(&mut cursor)?;
        return Ok(Blocks::Uniform(BlockStateId::new(state_id)));
    }

    let log2 = match bits {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        other => {
            return Err(StorageError::CorruptPayload(format!(
                "Invalid bit width {other} in BLOCKS section"
            )));
        }
    };

    let palette_len = decode_varint_u32(&mut cursor)? as usize;
    let max_palette_len = 1usize << bits;
    if palette_len == 0 || palette_len > max_palette_len {
        return Err(StorageError::CorruptPayload(format!(
            "Palette length {palette_len} invalid for {bits} bits (max {max_palette_len})"
        )));
    }

    let mut palette = Vec::with_capacity(palette_len);
    for _ in 0..palette_len {
        let state_id = decode_varint_u32(&mut cursor)?;
        palette.push(BlockStateId::new(state_id));
    }

    let word_count = 512usize << log2;
    let expected_words_bytes = word_count * 8;
    if cursor.len() < expected_words_bytes {
        return Err(StorageError::Truncated {
            actual: cursor.len() as u64,
            expected: expected_words_bytes as u64,
        });
    }

    let mut words: Vec<u64> = Vec::with_capacity(word_count);
    #[cfg(target_endian = "little")]
    {
        let u64_bytes = &cursor[..expected_words_bytes];
        unsafe {
            let dst = words.as_mut_ptr().cast::<u8>();
            std::ptr::copy_nonoverlapping(u64_bytes.as_ptr(), dst, expected_words_bytes);
            words.set_len(word_count);
        }
    }
    #[cfg(not(target_endian = "little"))]
    {
        for chunk in cursor[..expected_words_bytes].chunks_exact(8) {
            words.push(u64::from_le_bytes(chunk.try_into().unwrap()));
        }
    }
    let words = words.into_boxed_slice();

    // Fast SIMD / bounds-checked validation of packed palette indices
    validate_packed_indices(&words, log2, bits, palette_len)?;

    let packed = Packed::from_raw_parts(log2, palette.into_boxed_slice(), words);
    Ok(Blocks::Packed(Box::new(packed)))
}

#[inline]
fn validate_packed_indices(words: &[u64], log2: u8, bits: u8, palette_len: usize) -> Result<()> {
    let max_palette_len = 1usize << bits;
    // Fast path: if palette is at max capacity, all bit patterns are valid indices
    if palette_len >= max_palette_len {
        return Ok(());
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: target feature avx2 checked at runtime
            unsafe {
                if validate_packed_indices_avx2(words, bits, palette_len) {
                    return Ok(());
                }
            }
        }
    }

    // Scalar fallback and exact error reporter
    let pw_log2 = 6 - u32::from(log2);
    let entries_per_word = 1usize << pw_log2;
    let mask = (1u64 << bits) - 1;
    for &w in words {
        for e in 0..entries_per_word {
            let slot = ((w >> (e * (bits as usize))) & mask) as usize;
            if slot >= palette_len {
                return Err(StorageError::CorruptPayload(format!(
                    "Packed voxel index {slot} out of bounds for palette length {palette_len}"
                )));
            }
        }
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(
    clippy::ptr_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::wildcard_imports
)]
unsafe fn validate_packed_indices_avx2(words: &[u64], bits: u8, palette_len: usize) -> bool {
    use std::arch::x86_64::*;

    let byte_len = words.len() * 8;
    let bytes_ptr = words.as_ptr() as *const u8;

    match bits {
        4 => {
            let mask_0f = _mm256_set1_epi8(0x0F);
            let mut max_vec = _mm256_setzero_si256();

            let num_vectors = byte_len / 32;
            for i in 0..num_vectors {
                // SAFETY: caller guarantees words length is exact multiple of 32 bytes and non-null
                let v = unsafe { _mm256_loadu_si256(bytes_ptr.add(i * 32) as *const __m256i) };
                let low = _mm256_and_si256(v, mask_0f);
                let high = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask_0f);
                let max_nibbles = _mm256_max_epu8(low, high);
                max_vec = _mm256_max_epu8(max_vec, max_nibbles);
            }

            let mut buf = [0u8; 32];
            // SAFETY: buf is 32 bytes, properly sized for 256-bit SIMD register store
            unsafe {
                _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, max_vec);
            }
            let mut max_slot = 0u8;
            for &b in &buf {
                max_slot = max_slot.max(b);
            }

            (max_slot as usize) < palette_len
        }
        8 => {
            let mut max_vec = _mm256_setzero_si256();
            let num_vectors = byte_len / 32;
            for i in 0..num_vectors {
                // SAFETY: caller guarantees words length is exact multiple of 32 bytes and non-null
                let v = unsafe { _mm256_loadu_si256(bytes_ptr.add(i * 32) as *const __m256i) };
                max_vec = _mm256_max_epu8(max_vec, v);
            }

            let mut buf = [0u8; 32];
            // SAFETY: buf is 32 bytes, properly sized for 256-bit SIMD register store
            unsafe {
                _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, max_vec);
            }
            let mut max_slot = 0u8;
            for &b in &buf {
                max_slot = max_slot.max(b);
            }

            (max_slot as usize) < palette_len
        }
        1 => {
            if palette_len >= 2 {
                true
            } else {
                words.iter().all(|&w| w == 0)
            }
        }
        _ => false,
    }
}

/// Encodes `BlockEntityTable` into the binary `BLOCK_ENTITIES` section format.
pub fn encode_block_entities(table: &BlockEntityTable, buf: &mut Vec<u8>) {
    let mut entries: Vec<(&LocalIdx, &BlockEntityData)> = table.iter().collect();
    entries.sort_by_key(|(idx, _)| idx.as_u16());

    #[allow(clippy::cast_possible_truncation)]
    encode_varint_u32(entries.len() as u32, buf);
    for (idx, data) in entries {
        encode_varint_u32(u32::from(idx.as_u16()), buf);
        match data {
            BlockEntityData::Chest { custom_name, items } => {
                buf.push(1); // kind 1 = Chest
                if let Some(name) = custom_name {
                    #[allow(clippy::cast_possible_truncation)]
                    encode_varint_u32(name.len() as u32, buf);
                    buf.extend_from_slice(name.as_bytes());
                } else {
                    encode_varint_u32(0, buf);
                }
                let non_empty: Vec<&BlockEntitySlot> =
                    items.iter().filter(|s| !s.is_empty()).collect();
                #[allow(clippy::cast_possible_truncation)]
                encode_varint_u32(non_empty.len() as u32, buf);
                for slot in non_empty {
                    buf.push(slot.slot);
                    encode_varint_u32(slot.item, buf);
                    encode_varint_u32(u32::from(slot.count), buf);
                }
            }
            BlockEntityData::Furnace {
                custom_name,
                items,
                burn_time_remaining,
                total_burn_time,
                cook_progress,
                cook_duration,
            } => {
                buf.push(2); // kind 2 = Furnace
                if let Some(name) = custom_name {
                    #[allow(clippy::cast_possible_truncation)]
                    encode_varint_u32(name.len() as u32, buf);
                    buf.extend_from_slice(name.as_bytes());
                } else {
                    encode_varint_u32(0, buf);
                }
                encode_varint_u32(u32::from(*burn_time_remaining), buf);
                encode_varint_u32(u32::from(*total_burn_time), buf);
                encode_varint_u32(u32::from(*cook_progress), buf);
                encode_varint_u32(u32::from(*cook_duration), buf);
                let non_empty: Vec<&BlockEntitySlot> =
                    items.iter().filter(|s| !s.is_empty()).collect();
                #[allow(clippy::cast_possible_truncation)]
                encode_varint_u32(non_empty.len() as u32, buf);
                for slot in non_empty {
                    buf.push(slot.slot);
                    encode_varint_u32(slot.item, buf);
                    encode_varint_u32(u32::from(slot.count), buf);
                }
            }
        }
    }
}

/// Decodes binary `BLOCK_ENTITIES` section into `BlockEntityTable`.
#[allow(clippy::too_many_lines)]
pub fn decode_block_entities(mut cursor: &[u8]) -> Result<BlockEntityTable> {
    if cursor.is_empty() {
        return Ok(BlockEntityTable::new());
    }
    let count = decode_varint_u32(&mut cursor)? as usize;
    let mut table = BlockEntityTable::new();

    for _ in 0..count {
        let raw_idx = decode_varint_u32(&mut cursor)?;
        if raw_idx >= 32768 {
            return Err(StorageError::CorruptPayload(format!(
                "LocalIdx {raw_idx} out of range in BLOCK_ENTITIES"
            )));
        }
        #[allow(clippy::cast_possible_truncation)]
        let local_idx = LocalIdx::from_u16(raw_idx as u16)
            .ok_or_else(|| StorageError::CorruptPayload(format!("Invalid LocalIdx {raw_idx}")))?;

        if cursor.is_empty() {
            return Err(StorageError::CorruptPayload(
                "Truncated BLOCK_ENTITIES header".into(),
            ));
        }
        let kind = cursor[0];
        cursor = &cursor[1..];

        match kind {
            1 => {
                // Chest
                let name_len = decode_varint_u32(&mut cursor)? as usize;
                let custom_name = if name_len > 0 {
                    if cursor.len() < name_len {
                        return Err(StorageError::Truncated {
                            actual: cursor.len() as u64,
                            expected: name_len as u64,
                        });
                    }
                    let s = std::str::from_utf8(&cursor[..name_len]).map_err(|e| {
                        StorageError::CorruptPayload(format!("Invalid UTF-8 in custom name: {e}"))
                    })?;
                    cursor = &cursor[name_len..];
                    Some(s.to_string())
                } else {
                    None
                };

                let mut items = [BlockEntitySlot::EMPTY; CHEST_CONTAINER_SLOTS];
                #[allow(clippy::cast_possible_truncation)]
                for (i, slot) in items.iter_mut().enumerate() {
                    slot.slot = i as u8;
                }
                let slot_count = decode_varint_u32(&mut cursor)? as usize;
                if slot_count > CHEST_CONTAINER_SLOTS {
                    return Err(StorageError::CorruptPayload(format!(
                        "Chest slot count {slot_count} exceeds maximum {CHEST_CONTAINER_SLOTS}"
                    )));
                }
                for _ in 0..slot_count {
                    if cursor.is_empty() {
                        return Err(StorageError::CorruptPayload(
                            "Truncated slot in BLOCK_ENTITIES".into(),
                        ));
                    }
                    let slot_idx = cursor[0];
                    cursor = &cursor[1..];
                    let item = decode_varint_u32(&mut cursor)?;
                    let item_count = decode_varint_u32(&mut cursor)?;
                    if (slot_idx as usize) >= CHEST_CONTAINER_SLOTS {
                        return Err(StorageError::CorruptPayload(format!(
                            "Slot index {slot_idx} out of range"
                        )));
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let count_u16 = (item_count & 0xffff) as u16;
                    items[slot_idx as usize] = BlockEntitySlot::new(slot_idx, item, count_u16);
                }
                let chest = BlockEntityData::Chest { custom_name, items };
                table.insert(local_idx, chest);
            }
            2 => {
                // Furnace
                let name_len = decode_varint_u32(&mut cursor)? as usize;
                let custom_name = if name_len > 0 {
                    if cursor.len() < name_len {
                        return Err(StorageError::Truncated {
                            actual: cursor.len() as u64,
                            expected: name_len as u64,
                        });
                    }
                    let s = std::str::from_utf8(&cursor[..name_len]).map_err(|e| {
                        StorageError::CorruptPayload(format!("Invalid UTF-8 in custom name: {e}"))
                    })?;
                    cursor = &cursor[name_len..];
                    Some(s.to_string())
                } else {
                    None
                };

                #[allow(clippy::cast_possible_truncation)]
                let burn_time_remaining = (decode_varint_u32(&mut cursor)? & 0xffff) as u16;
                #[allow(clippy::cast_possible_truncation)]
                let total_burn_time = (decode_varint_u32(&mut cursor)? & 0xffff) as u16;
                #[allow(clippy::cast_possible_truncation)]
                let cook_progress = (decode_varint_u32(&mut cursor)? & 0xffff) as u16;
                #[allow(clippy::cast_possible_truncation)]
                let cook_duration = (decode_varint_u32(&mut cursor)? & 0xffff) as u16;

                let mut items = [BlockEntitySlot::EMPTY; FURNACE_CONTAINER_SLOTS];
                #[allow(clippy::cast_possible_truncation)]
                for (i, slot) in items.iter_mut().enumerate() {
                    slot.slot = i as u8;
                }
                let slot_count = decode_varint_u32(&mut cursor)? as usize;
                if slot_count > FURNACE_CONTAINER_SLOTS {
                    return Err(StorageError::CorruptPayload(format!(
                        "Furnace slot count {slot_count} exceeds maximum {FURNACE_CONTAINER_SLOTS}"
                    )));
                }
                for _ in 0..slot_count {
                    if cursor.is_empty() {
                        return Err(StorageError::CorruptPayload(
                            "Truncated slot in BLOCK_ENTITIES".into(),
                        ));
                    }
                    let slot_idx = cursor[0];
                    cursor = &cursor[1..];
                    let item = decode_varint_u32(&mut cursor)?;
                    let item_count = decode_varint_u32(&mut cursor)?;
                    if (slot_idx as usize) >= FURNACE_CONTAINER_SLOTS {
                        return Err(StorageError::CorruptPayload(format!(
                            "Furnace slot index {slot_idx} out of range"
                        )));
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let count_u16 = (item_count & 0xffff) as u16;
                    items[slot_idx as usize] = BlockEntitySlot::new(slot_idx, item, count_u16);
                }
                let furnace = BlockEntityData::Furnace {
                    custom_name,
                    items,
                    burn_time_remaining,
                    total_burn_time,
                    cook_progress,
                    cook_duration,
                };
                table.insert(local_idx, furnace);
            }
            other => {
                return Err(StorageError::CorruptPayload(format!(
                    "Unknown block entity kind {other}"
                )));
            }
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_payload_with_block_entities_roundtrip() {
        let blocks = Blocks::Uniform(BlockStateId::new(1));
        let status = ChunkStatus::default();
        let mut table = BlockEntityTable::new();

        let idx = LocalIdx::from_coords(4, 5, 6).unwrap();
        let mut chest = BlockEntityData::new_chest();
        if let BlockEntityData::Chest { custom_name, items } = &mut chest {
            *custom_name = Some("Treasure Chest".into());
            items[0] = BlockEntitySlot::new(0, 42, 64);
            items[26] = BlockEntitySlot::new(26, 10, 1);
        }
        table.insert(idx, chest);

        let payload = ChunkPayload::with_block_entities(blocks, status, table);
        let encoded = payload.encode();
        let decoded = ChunkPayload::decode(&encoded).expect("Decoding must succeed");

        assert_eq!(decoded.status, payload.status);
        assert_eq!(decoded.block_entities.len(), 1);

        let be = decoded
            .block_entities
            .get(idx)
            .expect("Must find chest at idx");
        assert_eq!(be.custom_name(), Some("Treasure Chest"));
        assert_eq!(be.items()[0].item, 42);
        assert_eq!(be.items()[0].count, 64);
        assert_eq!(be.items()[26].item, 10);
        assert_eq!(be.items()[26].count, 1);
        assert!(be.items()[1].is_empty());
    }
}
