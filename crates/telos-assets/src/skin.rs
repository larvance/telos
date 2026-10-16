//! Player skin format handling, legacy 64x32 auto-upgrading, and high-definition Hytale skins.

use crate::error::AssetError;
use crate::image_buf::RgbaImage;

/// Classification of supported humanoid skin formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinFormat {
    /// Modern standard 64x64 classic humanoid skin (4px arm width).
    StandardClassic,
    /// Modern standard 64x64 slim humanoid skin (3px arm width).
    StandardSlim,
    /// Legacy 64x32 single-layer humanoid skin with mirrored limbs.
    LegacyClassic,
    /// High-definition 128x128 Hytale skin.
    HytaleHd,
}

impl SkinFormat {
    /// Returns true if the skin format uses slim 3-pixel arms.
    #[must_use]
    pub const fn is_slim(&self) -> bool {
        matches!(self, Self::StandardSlim)
    }

    /// Returns the canonical resolution width and height for this format.
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::StandardClassic | Self::StandardSlim => (64, 64),
            Self::LegacyClassic => (64, 32),
            Self::HytaleHd => (128, 128),
        }
    }
}

/// Detects the skin format based on texture dimensions and an optional slim model hint.
pub fn detect_skin_format(
    width: u32,
    height: u32,
    is_slim_hint: Option<bool>,
) -> Result<SkinFormat, AssetError> {
    match (width, height) {
        (64, 32) => Ok(SkinFormat::LegacyClassic),
        (64, 64) => {
            if is_slim_hint.unwrap_or(false) {
                Ok(SkinFormat::StandardSlim)
            } else {
                Ok(SkinFormat::StandardClassic)
            }
        }
        (128, 128) => Ok(SkinFormat::HytaleHd),
        _ => Err(AssetError::InvalidDimensions {
            name: "player_skin".to_string(),
            width,
            height,
        }),
    }
}

/// Upgrades a legacy 64x32 skin to standard 64x64 layout.
///
/// In 64x32 skins:
/// - The top 64x32 contains head, hat, torso, right arm, and right leg.
/// - Left leg and left arm are omitted and mirrored from their right counterparts.
/// - Outer overlay layers (jacket, sleeves, pant legs) are absent.
///
/// This function constructs the corresponding 64x64 skin by mirroring the right limbs
/// into the designated left limb UV areas (left leg at x:16..32, y:48..64; left arm at x:32..48, y:48..64)
/// and setting all 2nd layer overlays to fully transparent.
#[must_use]
pub fn upgrade_legacy_skin_64x32_to_64x64(legacy: &RgbaImage) -> RgbaImage {
    assert!(
        legacy.width == 64 && legacy.height == 32,
        "Source image must be 64x32"
    );

    let mut upgraded = RgbaImage::new(64, 64);

    // 1. Copy original 64x32 top half directly
    for y in 0..32 {
        for x in 0..64 {
            let px = legacy.get_pixel(x, y);
            upgraded.set_pixel(x, y, px);
        }
    }

    // 2. Generate Left Leg from Right Leg (Right leg is at x: 0..16, y: 16..32)
    // Destination Left Leg is at x: 16..32, y: 48..64
    // Top/Bottom faces (4x4)
    // Top: (4..8, 16..20) -> (20..24, 48..52) [horizontal flip for left limb]
    copy_flipped_sub_rect(legacy, &mut upgraded, 4, 16, 4, 4, 20, 48, true);
    // Bottom: (8..12, 16..20) -> (24..28, 48..52) [horizontal flip]
    copy_flipped_sub_rect(legacy, &mut upgraded, 8, 16, 4, 4, 24, 48, true);

    // Side faces (4x12)
    // Outer/Right face: (0..4, 20..32) -> Inner/Left face: (24..28, 52..64)
    copy_flipped_sub_rect(legacy, &mut upgraded, 0, 20, 4, 12, 24, 52, false);
    // Front face: (4..8, 20..32) -> Front face: (20..24, 52..64) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 4, 20, 4, 12, 20, 52, true);
    // Inner/Left face: (8..12, 20..32) -> Outer/Right face: (16..20, 52..64)
    copy_flipped_sub_rect(legacy, &mut upgraded, 8, 20, 4, 12, 16, 52, false);
    // Back face: (12..16, 20..32) -> Back face: (28..32, 52..64) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 12, 20, 4, 12, 28, 52, true);

    // 3. Generate Left Arm from Right Arm (Right arm is at x: 40..56, y: 16..32)
    // Destination Left Arm is at x: 32..48, y: 48..64
    // Top/Bottom faces (4x4)
    // Top: (44..48, 16..20) -> (36..40, 48..52) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 44, 16, 4, 4, 36, 48, true);
    // Bottom: (48..52, 16..20) -> (40..44, 48..52) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 48, 16, 4, 4, 40, 48, true);

    // Side faces (4x12)
    // Outer/Right face: (40..44, 20..32) -> Inner/Left face: (40..44, 52..64)
    copy_flipped_sub_rect(legacy, &mut upgraded, 40, 20, 4, 12, 40, 52, false);
    // Front face: (44..48, 20..32) -> Front face: (36..40, 52..64) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 44, 20, 4, 12, 36, 52, true);
    // Inner/Left face: (48..52, 20..32) -> Outer/Right face: (32..36, 52..64)
    copy_flipped_sub_rect(legacy, &mut upgraded, 48, 20, 4, 12, 32, 52, false);
    // Back face: (52..56, 20..32) -> Back face: (44..48, 52..64) [flipped horizontally]
    copy_flipped_sub_rect(legacy, &mut upgraded, 52, 20, 4, 12, 44, 52, true);

    upgraded.zero_transparent_rgb();
    upgraded
}

fn copy_flipped_sub_rect(
    src: &RgbaImage,
    dst: &mut RgbaImage,
    sx: u32,
    sy: u32,
    w: u32,
    h: u32,
    dx: u32,
    dy: u32,
    flip_x: bool,
) {
    for y in 0..h {
        for x in 0..w {
            let src_x = if flip_x { sx + (w - 1 - x) } else { sx + x };
            let src_y = sy + y;
            let px = src.get_pixel(src_x, src_y);
            dst.set_pixel(dx + x, dy + y, px);
        }
    }
}

/// Loads and validates a skin image from raw PNG bytes, auto-converting legacy 64x32 skins.
pub fn load_skin_image_from_png(
    png_bytes: &[u8],
    is_slim_hint: Option<bool>,
) -> Result<(RgbaImage, SkinFormat), AssetError> {
    let img = image::load_from_memory(png_bytes).map_err(|e| AssetError::Decode {
        path: std::path::PathBuf::from("in-memory-skin.png"),
        message: e.to_string(),
    })?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let format = detect_skin_format(width, height, is_slim_hint)?;

    let mut raw_image = RgbaImage {
        width,
        height,
        data: rgba.into_raw(),
    };
    raw_image.zero_transparent_rgb();

    let (final_image, final_format) = match format {
        SkinFormat::LegacyClassic => {
            let upgraded = upgrade_legacy_skin_64x32_to_64x64(&raw_image);
            (upgraded, SkinFormat::StandardClassic)
        }
        f => (raw_image, f),
    };

    Ok((final_image, final_format))
}

// ----------------------------------------------------------------------------
// Procedural Default Skins
// ----------------------------------------------------------------------------

/// Generates the canonical 64x64 Classic Explorer player skin.
#[must_use]
pub fn default_classic_explorer_skin() -> RgbaImage {
    let mut skin = RgbaImage::new(64, 64);

    let skin_base = [215, 170, 130, 255];
    let skin_shadow = [190, 145, 110, 255];
    let hair_color = [60, 40, 25, 255];
    let hair_highlight = [85, 55, 35, 255];
    let eye_white = [240, 240, 245, 255];
    let eye_iris = [45, 95, 165, 255];

    let tunic_primary = [40, 125, 140, 255];
    let tunic_shadow = [30, 95, 105, 255];
    let belt_leather = [80, 50, 25, 255];
    let buckle_brass = [210, 175, 55, 255];
    let pants_blue = [45, 55, 115, 255];
    let boots_dark = [40, 30, 20, 255];

    // 1. Head (0..32, 0..16)
    // Base skin tone
    for y in 0..16 {
        for x in 0..32 {
            let shade = if (x + y) % 3 == 0 { skin_shadow } else { skin_base };
            skin.set_pixel(x, y, shade);
        }
    }
    // Hair on top (4..12, 0..8) & sides
    for y in 0..8 {
        for x in 0..32 {
            let h_shade = if (x + y) % 2 == 0 { hair_highlight } else { hair_color };
            skin.set_pixel(x, y, h_shade);
        }
    }
    // Face (8..16, 8..16)
    for y in 8..16 {
        for x in 8..16 {
            skin.set_pixel(x, y, skin_base);
        }
    }
    // Hair fringe
    for x in 8..16 {
        skin.set_pixel(x, 8, hair_color);
    }
    skin.set_pixel(8, 9, hair_color);
    skin.set_pixel(15, 9, hair_color);

    // Eyes: (10, 12) & (13, 12)
    skin.set_pixel(9, 12, eye_white);
    skin.set_pixel(10, 12, eye_iris);
    skin.set_pixel(13, 12, eye_iris);
    skin.set_pixel(14, 12, eye_white);

    // Mouth / chin
    skin.set_pixel(11, 14, [160, 100, 80, 255]);
    skin.set_pixel(12, 14, [160, 100, 80, 255]);

    // 2. Torso (16..40, 16..32)
    for y in 16..32 {
        for x in 16..40 {
            let color = if y >= 28 {
                belt_leather
            } else if (x + y) % 3 == 0 {
                tunic_shadow
            } else {
                tunic_primary
            };
            skin.set_pixel(x, y, color);
        }
    }
    // Belt buckle (23, 29) & (24, 29)
    skin.set_pixel(23, 29, buckle_brass);
    skin.set_pixel(24, 29, buckle_brass);

    // 3. Right Arm (40..56, 16..32) & Left Arm (32..48, 48..64)
    for y in 16..32 {
        for x in 40..56 {
            let is_sleeve = y < 24;
            let c = if is_sleeve { tunic_primary } else { skin_base };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 48..64 {
        for x in 32..48 {
            let is_sleeve = y < 56;
            let c = if is_sleeve { tunic_primary } else { skin_base };
            skin.set_pixel(x, y, c);
        }
    }

    // 4. Right Leg (0..16, 16..32) & Left Leg (16..32, 48..64)
    for y in 16..32 {
        for x in 0..16 {
            let is_boot = y >= 26;
            let c = if is_boot { boots_dark } else { pants_blue };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 48..64 {
        for x in 16..32 {
            let is_boot = y >= 58;
            let c = if is_boot { boots_dark } else { pants_blue };
            skin.set_pixel(x, y, c);
        }
    }

    // 5. 2nd Layer Overlays: Hat (32..64, 0..16) & Jacket (16..40, 32..48)
    for x in 32..64 {
        for y in 0..8 {
            // Hat hair volume
            if (x + y) % 4 == 0 {
                skin.set_pixel(x, y, [75, 50, 30, 220]);
            }
        }
    }

    // Jacket collar (19..25, 32..36)
    for y in 32..36 {
        for x in 20..24 {
            skin.set_pixel(x, y, [55, 35, 20, 240]);
        }
    }

    skin.zero_transparent_rgb();
    skin
}

/// Generates the canonical 64x64 Slim Scout player skin (3px arm).
#[must_use]
pub fn default_slim_scout_skin() -> RgbaImage {
    let mut skin = RgbaImage::new(64, 64);

    let skin_base = [225, 185, 150, 255];
    let hair_auburn = [140, 60, 30, 255];
    let hair_highlight = [170, 80, 40, 255];
    let tunic_green = [50, 115, 65, 255];
    let tunic_dark = [35, 85, 45, 255];
    let belt_leather = [70, 45, 25, 255];
    let pants_brown = [60, 50, 40, 255];
    let boots_leather = [45, 30, 20, 255];

    // Head
    for y in 0..16 {
        for x in 0..32 {
            let shade = if (x + y) % 2 == 0 { hair_highlight } else { hair_auburn };
            skin.set_pixel(x, y, shade);
        }
    }
    // Face
    for y in 8..16 {
        for x in 8..16 {
            skin.set_pixel(x, y, skin_base);
        }
    }
    // Braid/fringe
    for x in 8..16 {
        skin.set_pixel(x, 8, hair_auburn);
    }
    skin.set_pixel(8, 9, hair_auburn);
    skin.set_pixel(15, 9, hair_auburn);

    // Green eyes
    skin.set_pixel(9, 12, [245, 245, 250, 255]);
    skin.set_pixel(10, 12, [40, 130, 75, 255]);
    skin.set_pixel(13, 12, [40, 130, 75, 255]);
    skin.set_pixel(14, 12, [245, 245, 250, 255]);

    // Torso
    for y in 16..32 {
        for x in 16..40 {
            let c = if y >= 28 {
                belt_leather
            } else if (x + y) % 3 == 0 {
                tunic_dark
            } else {
                tunic_green
            };
            skin.set_pixel(x, y, c);
        }
    }

    // Right Arm (3px width) & Left Arm
    for y in 16..32 {
        for x in 40..54 {
            let c = if y < 24 { tunic_green } else { skin_base };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 48..64 {
        for x in 32..46 {
            let c = if y < 56 { tunic_green } else { skin_base };
            skin.set_pixel(x, y, c);
        }
    }

    // Legs
    for y in 16..32 {
        for x in 0..16 {
            let c = if y >= 26 { boots_leather } else { pants_brown };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 48..64 {
        for x in 16..32 {
            let c = if y >= 58 { boots_leather } else { pants_brown };
            skin.set_pixel(x, y, c);
        }
    }

    skin.zero_transparent_rgb();
    skin
}

/// Generates a high-definition 128x128 Hytale Adventurer player skin.
#[must_use]
pub fn default_hytale_adventurer_skin() -> RgbaImage {
    let mut skin = RgbaImage::new(128, 128);

    let skin_tone = [218, 176, 142, 255];
    let skin_soft_shadow = [196, 152, 120, 255];
    let eye_sclera = [245, 245, 250, 255];
    let eye_amber = [185, 115, 35, 255];
    let eye_amber_light = [225, 155, 55, 255];

    let coat_navy = [32, 44, 68, 255];
    let coat_navy_dark = [22, 30, 48, 255];
    let coat_trim_gold = [215, 175, 65, 255];
    let fur_cowl = [230, 225, 215, 255];
    let fur_shadow = [190, 185, 175, 255];
    let leather_pauldron = [115, 68, 38, 255];
    let leather_strap = [65, 38, 20, 255];
    let pants_slate = [52, 60, 72, 255];
    let boots_heavy = [42, 28, 18, 255];
    let metal_buckle = [200, 205, 215, 255];

    // 1. Head (0..64, 0..32 in 128-space)
    for y in 0..32 {
        for x in 0..64 {
            let c = if (x + y) % 3 == 0 { skin_soft_shadow } else { skin_tone };
            skin.set_pixel(x, y, c);
        }
    }

    // Styled hair on head
    for y in 0..18 {
        for x in 0..64 {
            let wave = ((x / 4) + (y / 2)) % 3;
            let h = match wave {
                0 => [75, 50, 32, 255],
                1 => [95, 65, 42, 255],
                _ => [60, 38, 24, 255],
            };
            skin.set_pixel(x, y, h);
        }
    }

    // Face front (16..32, 16..32)
    for y in 16..32 {
        for x in 16..32 {
            skin.set_pixel(x, y, skin_tone);
        }
    }

    // Expressive HD Eyes (20..23, 23..26) & (25..28, 23..26)
    // Left eye
    skin.set_pixel(19, 23, eye_sclera);
    skin.set_pixel(19, 24, eye_sclera);
    skin.set_pixel(20, 23, eye_amber);
    skin.set_pixel(20, 24, eye_amber_light);
    skin.set_pixel(21, 23, eye_amber);
    skin.set_pixel(21, 24, eye_sclera);

    // Right eye
    skin.set_pixel(26, 23, eye_sclera);
    skin.set_pixel(26, 24, eye_amber);
    skin.set_pixel(27, 23, eye_amber_light);
    skin.set_pixel(27, 24, eye_amber);
    skin.set_pixel(28, 23, eye_sclera);
    skin.set_pixel(28, 24, eye_sclera);

    // Eyebrows
    for x in 19..23 {
        skin.set_pixel(x, 21, [55, 35, 20, 255]);
    }
    for x in 25..29 {
        skin.set_pixel(x, 21, [55, 35, 20, 255]);
    }

    // Lips / mouth
    skin.set_pixel(23, 28, [170, 110, 90, 255]);
    skin.set_pixel(24, 28, [170, 110, 90, 255]);

    // 2. Torso (32..80, 32..64)
    for y in 32..64 {
        for x in 32..80 {
            let is_belt = (54..60).contains(&y);
            let c = if is_belt {
                leather_strap
            } else if (x + y) % 4 == 0 {
                coat_navy_dark
            } else {
                coat_navy
            };
            skin.set_pixel(x, y, c);
        }
    }

    // Gold trim vertical lapels
    for y in 32..54 {
        skin.set_pixel(46, y, coat_trim_gold);
        skin.set_pixel(49, y, coat_trim_gold);
    }
    // Big metal adventurer belt buckle
    for y in 55..59 {
        for x in 46..50 {
            skin.set_pixel(x, y, metal_buckle);
        }
    }

    // 3. Arms (Right: 80..112, 32..64; Left: 64..96, 96..128)
    for y in 32..64 {
        for x in 80..112 {
            let is_sleeve = y < 52;
            let c = if is_sleeve { coat_navy } else { skin_tone };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 96..128 {
        for x in 64..96 {
            let is_sleeve = y < 116;
            let c = if is_sleeve { coat_navy } else { skin_tone };
            skin.set_pixel(x, y, c);
        }
    }

    // 4. Legs (Right: 0..32, 32..64; Left: 32..64, 96..128)
    for y in 32..64 {
        for x in 0..32 {
            let is_boot = y >= 52;
            let c = if is_boot { boots_heavy } else { pants_slate };
            skin.set_pixel(x, y, c);
        }
    }
    for y in 96..128 {
        for x in 32..64 {
            let is_boot = y >= 116;
            let c = if is_boot { boots_heavy } else { pants_slate };
            skin.set_pixel(x, y, c);
        }
    }

    // 5. HD Overlays: Fur Cowl & Leather Pauldrons
    // Hat overlay (64..128, 0..32): Hood / fur collar
    for y in 0..32 {
        for x in 64..128 {
            if y < 16 || (y >= 26 && ((x / 2) % 2 == 0)) {
                let fur = if (x + y) % 3 == 0 { fur_shadow } else { fur_cowl };
                skin.set_pixel(x, y, fur);
            }
        }
    }

    // Jacket overlay (32..80, 64..96): Pauldrons & pouch
    // Pauldron edges
    for y in 64..74 {
        for x in 34..42 {
            skin.set_pixel(x, y, leather_pauldron);
        }
        for x in 54..62 {
            skin.set_pixel(x, y, leather_pauldron);
        }
    }
    // Hip pouch
    for y in 86..94 {
        for x in 54..60 {
            skin.set_pixel(x, y, [90, 55, 30, 255]);
        }
    }

    skin.zero_transparent_rgb();
    skin
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skin_format_detection() {
        assert_eq!(
            detect_skin_format(64, 32, None).unwrap(),
            SkinFormat::LegacyClassic
        );
        assert_eq!(
            detect_skin_format(64, 64, Some(false)).unwrap(),
            SkinFormat::StandardClassic
        );
        assert_eq!(
            detect_skin_format(64, 64, Some(true)).unwrap(),
            SkinFormat::StandardSlim
        );
        assert_eq!(
            detect_skin_format(128, 128, None).unwrap(),
            SkinFormat::HytaleHd
        );
        assert!(detect_skin_format(32, 32, None).is_err());
    }

    #[test]
    fn test_upgrade_legacy_skin() {
        let mut legacy = RgbaImage::new(64, 32);
        // Fill right leg front with a test pattern
        legacy.set_pixel(5, 25, [123, 45, 67, 255]);
        // Fill right arm top with a test pattern
        legacy.set_pixel(45, 17, [200, 100, 50, 255]);

        let upgraded = upgrade_legacy_skin_64x32_to_64x64(&legacy);
        assert_eq!(upgraded.width, 64);
        assert_eq!(upgraded.height, 64);

        // Original pixel should be intact in top half
        assert_eq!(upgraded.get_pixel(5, 25), [123, 45, 67, 255]);
        assert_eq!(upgraded.get_pixel(45, 17), [200, 100, 50, 255]);

        // Mirrored pixels should be present in bottom half
        // Right leg front x=5 -> mirrored left leg front x = 20 + (4 - 1 - (5 - 4)) = 20 + 2 = 22
        // y = 52 + (25 - 20) = 57
        assert_eq!(upgraded.get_pixel(22, 57), [123, 45, 67, 255]);
    }

    #[test]
    fn test_default_skins_generation() {
        let classic = default_classic_explorer_skin();
        assert_eq!(classic.width, 64);
        assert_eq!(classic.height, 64);

        let slim = default_slim_scout_skin();
        assert_eq!(slim.width, 64);
        assert_eq!(slim.height, 64);

        let hytale = default_hytale_adventurer_skin();
        assert_eq!(hytale.width, 128);
        assert_eq!(hytale.height, 128);
    }
}

