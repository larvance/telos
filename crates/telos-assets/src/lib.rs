//! # telos-assets
//!
//! Asset management, resource pack stack resolution, PNG decoding, and 2D texture array baking.
//!
//! Preserves headless server isolation (pure CPU, zero Vulkan/winit dependencies).

pub mod builder;
pub mod error;
pub mod gui;
pub mod image_buf;
pub mod material;
pub mod mcmeta;
pub mod pack;
pub mod skin;

pub use builder::{AnimatedTextureInfo, BakedTextureArray, MipCopyRegion, TextureArrayBuilder};
pub use error::AssetError;
pub use gui::{ContainerLayoutDef, GuiStyleSheet, HudThemeDef, NineSliceBorderDef, SlotLayoutDef};
pub use image_buf::RgbaImage;
pub use material::MaterialTextureMap;
pub use mcmeta::{
    AnimationDef, AnimationFrameDef, GuiMetaDef, GuiScaling, NineSliceBorder, TextureMetaDef,
};
pub use pack::ResourcePackStack;
pub use skin::{
    SkinFormat, default_classic_explorer_skin, default_hd_adventurer_skin, default_slim_scout_skin,
    detect_skin_format, load_skin_image_from_png, upgrade_legacy_skin_64x32_to_64x64,
};
