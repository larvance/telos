//! Inventory container, item stacks, and deterministic click interaction logic.

use crate::crafting::{RecipeRegistry, find_recipe_2x2_with_registry};
use crate::enchantment::CompactEnchantments;
use bevy_ecs::component::Component;
use std::ops::Range;
use telos_voxel::block_entity::{BlockEntityData, BlockEntitySlot};
use thiserror::Error;

/// Maximum number of items in a standard stackable item stack.
pub const MAX_STACK_SIZE: u16 = 64;

/// Total number of player inventory slots:
/// - 0..9: Hotbar (9 slots)
/// - 9..36: Main storage (27 slots)
/// - 36..40: Armor slots (4 slots: Helmet, Chestplate, Leggings, Boots)
/// - 40..44: 2x2 Crafting inputs (4 slots)
/// - 44: Crafting result (1 slot)
/// - 45: Offhand (1 slot)
pub const PLAYER_INVENTORY_SLOTS: usize = 46;

/// Hotbar slot range (0..9).
pub const HOTBAR_SLOTS: Range<usize> = 0..9;
/// Main inventory storage slot range (9..36).
pub const STORAGE_SLOTS: Range<usize> = 9..36;
/// Armor slot range (36..40).
pub const ARMOR_SLOTS: Range<usize> = 36..40;
/// Helmet armor slot index (36).
pub const ARMOR_HELMET_SLOT: usize = 36;
/// Chestplate armor slot index (37).
pub const ARMOR_CHESTPLATE_SLOT: usize = 37;
/// Leggings armor slot index (38).
pub const ARMOR_LEGGINGS_SLOT: usize = 38;
/// Boots armor slot index (39).
pub const ARMOR_BOOTS_SLOT: usize = 39;
/// 2x2 Crafting input slots range (40..44).
pub const CRAFTING_INPUT_SLOTS: Range<usize> = 40..44;
/// Crafting result output slot index (44).
pub const CRAFTING_RESULT_SLOT: usize = 44;
/// Offhand slot index (45).
pub const OFFHAND_SLOT: usize = 45;

/// Iron helmet item identifier (16).
pub const ITEM_IRON_HELMET: u32 = 16;
/// Iron chestplate item identifier (17).
pub const ITEM_IRON_CHESTPLATE: u32 = 17;
/// Iron leggings item identifier (18).
pub const ITEM_IRON_LEGGINGS: u32 = 18;
/// Iron boots item identifier (35).
pub const ITEM_IRON_BOOTS: u32 = 35;

/// Returns true if the item is a helmet.
#[must_use]
pub const fn is_helmet(item: u32) -> bool {
    item == ITEM_IRON_HELMET || item == ITEM_LEATHER_HELMET || item == ITEM_GOLDEN_HELMET
}

/// Returns true if the item is a chestplate.
#[must_use]
pub const fn is_chestplate(item: u32) -> bool {
    item == ITEM_IRON_CHESTPLATE
        || item == ITEM_LEATHER_CHESTPLATE
        || item == ITEM_GOLDEN_CHESTPLATE
}

/// Returns true if the item is leggings.
#[must_use]
pub const fn is_leggings(item: u32) -> bool {
    item == ITEM_IRON_LEGGINGS || item == ITEM_LEATHER_LEGGINGS || item == ITEM_GOLDEN_LEGGINGS
}

/// Returns true if the item is boots.
#[must_use]
pub const fn is_boots(item: u32) -> bool {
    item == 19
        || item == 31
        || item == ITEM_IRON_BOOTS
        || item == ITEM_LEATHER_BOOTS
        || item == ITEM_GOLDEN_BOOTS
}

/// Returns true if the item is any equippable armor item.
#[must_use]
pub const fn is_armor(item: u32) -> bool {
    is_helmet(item) || is_chestplate(item) || is_leggings(item) || is_boots(item)
}

/// Returns the matching armor slot index for an armor item, or None.
#[must_use]
pub const fn matching_armor_slot(item: u32) -> Option<usize> {
    if is_helmet(item) {
        Some(ARMOR_HELMET_SLOT)
    } else if is_chestplate(item) {
        Some(ARMOR_CHESTPLATE_SLOT)
    } else if is_leggings(item) {
        Some(ARMOR_LEGGINGS_SLOT)
    } else if is_boots(item) {
        Some(ARMOR_BOOTS_SLOT)
    } else {
        None
    }
}

/// Bow weapon item identifier (61).
pub const ITEM_BOW: u32 = 61;
/// Arrow projectile ammo item identifier (62).
pub const ITEM_ARROW: u32 = 62;
/// Chest container item identifier (63).
pub const ITEM_CHEST: u32 = 63;
/// Furnace block container item identifier (64).
pub const ITEM_FURNACE: u32 = 64;
/// Potion item identifier (36).
pub const ITEM_POTION: u32 = 36;
/// Glass bottle item identifier (38).
pub const ITEM_GLASS_BOTTLE: u32 = 38;
/// Crafting table block container item identifier (14).
pub const ITEM_CRAFTING_TABLE: u32 = 14;
/// Cooked porkchop food item identifier (65).
pub const ITEM_COOKED_PORKCHOP: u32 = 65;
/// Cooked beef food item identifier (66).
pub const ITEM_COOKED_BEEF: u32 = 66;
/// Charcoal combustible fuel item identifier (67).
pub const ITEM_CHARCOAL: u32 = 67;
/// Coal mineral item identifier (54).
pub const ITEM_COAL: u32 = 54;
/// Raw iron item identifier (52).
pub const ITEM_RAW_IRON: u32 = 52;
/// Raw gold item identifier (53).
pub const ITEM_RAW_GOLD: u32 = 53;
/// Diamond gemstone item identifier (20).
pub const ITEM_DIAMOND: u32 = 20;

/// Wooden pickaxe item identifier (68).
pub const ITEM_WOODEN_PICKAXE: u32 = 68;
/// Stone pickaxe item identifier (69).
pub const ITEM_STONE_PICKAXE: u32 = 69;
/// Iron pickaxe item identifier (70).
pub const ITEM_IRON_PICKAXE: u32 = 70;
/// Wooden axe item identifier (71).
pub const ITEM_WOODEN_AXE: u32 = 71;
/// Stone axe item identifier (72).
pub const ITEM_STONE_AXE: u32 = 72;
/// Iron axe item identifier (73).
pub const ITEM_IRON_AXE: u32 = 73;
/// Wooden shovel item identifier (74).
pub const ITEM_WOODEN_SHOVEL: u32 = 74;
/// Stone shovel item identifier (75).
pub const ITEM_STONE_SHOVEL: u32 = 75;
/// Iron shovel item identifier (76).
pub const ITEM_IRON_SHOVEL: u32 = 76;
/// Wooden sword item identifier (77).
pub const ITEM_WOODEN_SWORD: u32 = 77;
/// Stone sword item identifier (78).
pub const ITEM_STONE_SWORD: u32 = 78;
/// Iron sword item identifier (79).
pub const ITEM_IRON_SWORD: u32 = 79;
/// Wooden hoe item identifier (80).
pub const ITEM_WOODEN_HOE: u32 = 80;
/// Stone hoe item identifier (81).
pub const ITEM_STONE_HOE: u32 = 81;
/// Iron hoe item identifier (82).
pub const ITEM_IRON_HOE: u32 = 82;

/// Leather helmet item identifier (83).
pub const ITEM_LEATHER_HELMET: u32 = 83;
/// Leather chestplate item identifier (84).
pub const ITEM_LEATHER_CHESTPLATE: u32 = 84;
/// Leather leggings item identifier (85).
pub const ITEM_LEATHER_LEGGINGS: u32 = 85;
/// Leather boots item identifier (86).
pub const ITEM_LEATHER_BOOTS: u32 = 86;

/// Golden helmet item identifier (87).
pub const ITEM_GOLDEN_HELMET: u32 = 87;
/// Golden chestplate item identifier (88).
pub const ITEM_GOLDEN_CHESTPLATE: u32 = 88;
/// Golden leggings item identifier (89).
pub const ITEM_GOLDEN_LEGGINGS: u32 = 89;
/// Golden boots item identifier (90).
pub const ITEM_GOLDEN_BOOTS: u32 = 90;

/// Anvil block item identifier (91).
pub const ITEM_ANVIL: u32 = 91;
/// Book item identifier (92).
pub const ITEM_BOOK: u32 = 92;
/// Enchanted Book item identifier (93).
pub const ITEM_ENCHANTED_BOOK: u32 = 93;
/// Ink sac item identifier (94).
pub const ITEM_INK_SAC: u32 = 94;
/// Raw cod item identifier (95).
pub const ITEM_COD: u32 = 95;
/// Cooked cod item identifier (96).
pub const ITEM_COOKED_COD: u32 = 96;
/// Oak boat item identifier (97).
pub const ITEM_OAK_BOAT: u32 = 97;
/// String item identifier (55).
pub const ITEM_STRING: u32 = 55;
/// Fishing rod item identifier (98).
pub const ITEM_FISHING_ROD: u32 = 98;
/// Minecart vehicle item identifier (99).
pub const ITEM_MINECART: u32 = 99;
/// Standard rail track item identifier (100).
pub const ITEM_RAIL: u32 = 100;
/// Powered booster rail track item identifier (101).
pub const ITEM_POWERED_RAIL: u32 = 101;
/// Detector switch rail track item identifier (102).
pub const ITEM_DETECTOR_RAIL: u32 = 102;
/// Activator trigger rail track item identifier (103).
pub const ITEM_ACTIVATOR_RAIL: u32 = 103;
/// Iron ingot metallic crafting resource identifier (52).
pub const ITEM_IRON_INGOT: u32 = 52;
/// Flint mineral resource item identifier (104).
pub const ITEM_FLINT: u32 = 104;
/// Flint and steel portal ignition tool identifier (105).
pub const ITEM_FLINT_AND_STEEL: u32 = 105;
/// Nether quartz mineral resource item identifier (106).
pub const ITEM_NETHER_QUARTZ: u32 = 106;
/// Gold nugget mineral resource item identifier (107).
pub const ITEM_GOLD_NUGGET: u32 = 107;

/// Returns true if the item is a boat.
#[must_use]
pub const fn is_boat(item: u32) -> bool {
    item == ITEM_OAK_BOAT
}

/// Returns true if the item is a minecart.
#[must_use]
pub const fn is_minecart(item: u32) -> bool {
    item == ITEM_MINECART
}

/// Returns true if the item is flint and steel.
#[must_use]
pub const fn is_flint_and_steel(item: u32) -> bool {
    item == ITEM_FLINT_AND_STEEL
}

/// Returns true if the item is any type of rail track.
#[must_use]
pub const fn is_rail(item: u32) -> bool {
    matches!(
        item,
        ITEM_RAIL | ITEM_POWERED_RAIL | ITEM_DETECTOR_RAIL | ITEM_ACTIVATOR_RAIL
    )
}

/// Returns true if the item is a fishing rod.
#[must_use]
pub const fn is_fishing_rod(item: u32) -> bool {
    item == ITEM_FISHING_ROD
}

/// Returns true if the item is a sword.
#[must_use]
pub const fn is_sword(item: u32) -> bool {
    matches!(item, ITEM_WOODEN_SWORD | ITEM_STONE_SWORD | ITEM_IRON_SWORD)
}

/// Returns true if the item is a pickaxe.
#[must_use]
pub const fn is_pickaxe(item: u32) -> bool {
    matches!(
        item,
        ITEM_WOODEN_PICKAXE | ITEM_STONE_PICKAXE | ITEM_IRON_PICKAXE
    )
}

/// Returns true if the item is an axe.
#[must_use]
pub const fn is_axe(item: u32) -> bool {
    matches!(item, ITEM_WOODEN_AXE | ITEM_STONE_AXE | ITEM_IRON_AXE)
}

/// Returns true if the item is a shovel.
#[must_use]
pub const fn is_shovel(item: u32) -> bool {
    matches!(
        item,
        ITEM_WOODEN_SHOVEL | ITEM_STONE_SHOVEL | ITEM_IRON_SHOVEL
    )
}

/// Returns true if the item is a hoe.
#[must_use]
pub const fn is_hoe(item: u32) -> bool {
    matches!(item, ITEM_WOODEN_HOE | ITEM_STONE_HOE | ITEM_IRON_HOE)
}

/// Returns true if the item is any mining or digging tool.
#[must_use]
pub const fn is_tool(item: u32) -> bool {
    is_pickaxe(item) || is_axe(item) || is_shovel(item) || is_hoe(item)
}

/// Returns true if the item is a plain book.
#[must_use]
pub const fn is_book(item: u32) -> bool {
    item == ITEM_BOOK
}

/// Returns true if the item is an enchanted book.
#[must_use]
pub const fn is_enchanted_book(item: u32) -> bool {
    item == ITEM_ENCHANTED_BOOK
}

/// Returns true if the item is an anvil block.
#[must_use]
pub const fn is_anvil(item: u32) -> bool {
    item == ITEM_ANVIL
}

/// Returns true if the item is a bow.
#[must_use]
pub const fn is_bow(item: u32) -> bool {
    item == ITEM_BOW
}

/// Returns true if the item is an arrow.
#[must_use]
pub const fn is_arrow(item: u32) -> bool {
    item == ITEM_ARROW
}

/// Returns true if the item is a chest.
#[must_use]
pub const fn is_chest(item: u32) -> bool {
    item == ITEM_CHEST
}

/// Returns true if the item is a furnace.
#[must_use]
pub const fn is_furnace(item: u32) -> bool {
    item == ITEM_FURNACE
}

/// Returns true if the item is a potion bottle.
#[must_use]
pub const fn is_potion(item: u32) -> bool {
    item == ITEM_POTION
}

/// Mining tool tier for breakable blocks and mining level checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolTier {
    /// Wood material tier.
    Wood = 0,
    /// Stone material tier.
    Stone = 1,
    /// Iron material tier.
    Iron = 2,
}

/// Resolves tool tier for a pickaxe item.
#[must_use]
pub const fn pickaxe_tier(item: u32) -> Option<ToolTier> {
    match item {
        ITEM_WOODEN_PICKAXE => Some(ToolTier::Wood),
        ITEM_STONE_PICKAXE => Some(ToolTier::Stone),
        ITEM_IRON_PICKAXE => Some(ToolTier::Iron),
        _ => None,
    }
}

/// Maximum durability for an item, or `None` if the item is not damageable.
#[must_use]
pub const fn max_durability(item: u32) -> Option<u16> {
    match item {
        // Wooden tools
        ITEM_WOODEN_PICKAXE
        | ITEM_WOODEN_AXE
        | ITEM_WOODEN_SHOVEL
        | ITEM_WOODEN_SWORD
        | ITEM_WOODEN_HOE => Some(59),
        // Stone tools
        ITEM_STONE_PICKAXE
        | ITEM_STONE_AXE
        | ITEM_STONE_SHOVEL
        | ITEM_STONE_SWORD
        | ITEM_STONE_HOE => Some(131),
        // Iron tools
        ITEM_IRON_PICKAXE
        | ITEM_IRON_AXE
        | ITEM_IRON_SHOVEL
        | ITEM_IRON_SWORD
        | ITEM_IRON_HOE => Some(250),
        // Leather armor
        ITEM_LEATHER_HELMET => Some(55),
        ITEM_LEATHER_CHESTPLATE => Some(80),
        ITEM_LEATHER_LEGGINGS => Some(75),
        ITEM_LEATHER_BOOTS => Some(65),
        // Golden armor
        ITEM_GOLDEN_HELMET => Some(77),
        ITEM_GOLDEN_CHESTPLATE => Some(112),
        ITEM_GOLDEN_LEGGINGS => Some(105),
        ITEM_GOLDEN_BOOTS => Some(91),
        // Iron armor
        ITEM_IRON_HELMET => Some(165),
        ITEM_IRON_CHESTPLATE => Some(240),
        ITEM_IRON_LEGGINGS => Some(225),
        ITEM_IRON_BOOTS => Some(195),
        // Weapons and utility tools
        ITEM_BOW => Some(384),
        ITEM_FISHING_ROD => Some(64),
        ITEM_FLINT_AND_STEEL => Some(64),
        _ => None,
    }
}

/// Returns true if the item has durability and can take damage.
#[must_use]
pub const fn is_damageable(item: u32) -> bool {
    max_durability(item).is_some()
}

/// Applies durability damage to an item stack, respecting the Unbreaking enchantment.
/// Returns `true` if the item broke (reached or exceeded max durability and was cleared).
pub fn apply_durability_damage(stack: &mut ItemStack, amount: u16, rng_sample: f32) -> bool {
    if stack.is_empty() {
        return false;
    }
    let Some(max_dur) = max_durability(stack.item) else {
        return false;
    };
    if crate::enchantment::should_prevent_durability_loss(stack.enchantments, rng_sample) {
        return false;
    }
    stack.damage = stack.damage.saturating_add(amount);
    if stack.damage >= max_dur {
        *stack = ItemStack::EMPTY;
        true
    } else {
        false
    }
}

/// Returns the base melee attack damage of an item when held in the main hand.
#[must_use]
pub const fn weapon_base_damage(item: u32) -> f32 {
    match item {
        ITEM_WOODEN_SWORD => 4.0,
        ITEM_STONE_SWORD => 5.0,
        ITEM_IRON_SWORD => 6.0,
        ITEM_WOODEN_AXE => 7.0,
        ITEM_STONE_AXE => 9.0,
        ITEM_IRON_AXE => 9.0,
        ITEM_WOODEN_PICKAXE => 2.0,
        ITEM_STONE_PICKAXE => 3.0,
        ITEM_IRON_PICKAXE => 4.0,
        ITEM_WOODEN_SHOVEL => 2.5,
        ITEM_STONE_SHOVEL => 3.5,
        ITEM_IRON_SHOVEL => 4.5,
        _ => 1.0,
    }
}

/// Checks if a slot accepts the specified item.
#[must_use]
pub const fn is_slot_valid_for_item(slot: usize, item: u32) -> bool {
    if item == 0 {
        return true;
    }
    match slot {
        ARMOR_HELMET_SLOT => is_helmet(item),
        ARMOR_CHESTPLATE_SLOT => is_chestplate(item),
        ARMOR_LEGGINGS_SLOT => is_leggings(item),
        ARMOR_BOOTS_SLOT => is_boots(item),
        CRAFTING_RESULT_SLOT => false, // Cannot directly place items into crafting result!
        _ => true,
    }
}

/// Returns the user-facing name of an item ID.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn item_name(item: u32) -> &'static str {
    match item {
        0 => "Air",
        1 => "Stone",
        2 => "Dirt",
        3 => "Grass Block",
        4 => "Cobblestone",
        5 => "Oak Log",
        6 => "Water",
        7 => "Oak Planks",
        8 => "Oak Leaves",
        9 => "Glass",
        10 => "Stone Slab",
        11 => "Oak Stairs",
        12 => "Sand",
        13 => "Stick",
        14 => "Crafting Table",
        15 => "Torch",
        16 => "Iron Helmet",
        17 => "Iron Chestplate",
        18 => "Iron Leggings",
        19 => "Logic Wire",
        20 => "Powered Wire",
        21 => "Power Block",
        22 => "Lever",
        23 => "Lever (On)",
        24 => "Logic Lamp",
        25 => "Logic Lamp (Lit)",
        26 => "Logic Repeater",
        27 => "Logic Repeater (Powered)",
        28 => "Logic Inverter",
        29 => "Logic Inverter (Off)",
        30 => "Logic Diode",
        31 => "Lava",
        32 => "Flowing Lava",
        33 => "Fire",
        34 => "Nether Portal",
        35 => "Iron Boots",
        36 => "Potion",
        37 => "Splash Potion",
        38 => "Glass Bottle",
        39 => "Brewing Stand",
        40 => "Nether Wart",
        41 => "Blaze Powder",
        42 => "Sugar",
        43 => "Glistering Melon",
        44 => "Spider Eye",
        45 => "Fermented Spider Eye",
        46 => "Ghast Tear",
        47 => "Magma Cream",
        48 => "Rotten Flesh",
        49 => "Raw Porkchop",
        50 => "Raw Beef",
        51 => "Leather",
        52 => "Iron Ingot",
        53 => "Gold Ingot",
        54 => "Coal",
        55 => "String",
        56 => "Gunpowder",
        57 => "Bread",
        58 => "Wheat",
        59 => "Saddle",
        60 => "Name Tag",
        61 => "Bow",
        62 => "Arrow",
        63 => "Chest",
        64 => "Furnace",
        65 => "Cooked Porkchop",
        66 => "Cooked Beef",
        67 => "Charcoal",
        68 => "Wooden Pickaxe",
        69 => "Stone Pickaxe",
        70 => "Iron Pickaxe",
        71 => "Wooden Axe",
        72 => "Stone Axe",
        73 => "Iron Axe",
        74 => "Wooden Shovel",
        75 => "Stone Shovel",
        76 => "Iron Shovel",
        77 => "Wooden Sword",
        78 => "Stone Sword",
        79 => "Iron Sword",
        80 => "Wooden Hoe",
        81 => "Stone Hoe",
        82 => "Iron Hoe",
        83 => "Leather Cap",
        84 => "Leather Tunic",
        85 => "Leather Pants",
        86 => "Leather Boots",
        87 => "Golden Helmet",
        88 => "Golden Chestplate",
        89 => "Golden Leggings",
        90 => "Golden Boots",
        91 => "Anvil",
        92 => "Book",
        93 => "Enchanted Book",
        94 => "Ink Sac",
        95 => "Raw Cod",
        96 => "Cooked Cod",
        97 => "Oak Boat",
        98 => "Fishing Rod",
        99 => "Minecart",
        100 => "Rail",
        101 => "Powered Rail",
        102 => "Detector Rail",
        103 => "Activator Rail",
        104 => "Flint",
        105 => "Flint and Steel",
        106 => "Nether Quartz",
        107 => "Gold Nugget",
        _ => "Unknown Item",
    }
}

/// A stack of items in an inventory container or carried by the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemStack {
    /// Item identifier (0 = Air / Empty).
    pub item: u32,
    /// Number of items in this stack (0 = Empty).
    pub count: u16,
    /// Item damage / durability consumed so far (0 = undamaged).
    pub damage: u16,
    /// Compact bitpacked enchantments (empty if unenchanted).
    pub enchantments: CompactEnchantments,
}

impl ItemStack {
    /// Empty item stack constant.
    pub const EMPTY: Self = Self {
        item: 0,
        count: 0,
        damage: 0,
        enchantments: CompactEnchantments::EMPTY,
    };

    /// Creates a new unenchanted item stack, normalizing 0-count to Air.
    #[must_use]
    pub const fn new(item: u32, count: u16) -> Self {
        if item == 0 || count == 0 {
            Self::EMPTY
        } else {
            Self {
                item,
                count,
                damage: 0,
                enchantments: CompactEnchantments::EMPTY,
            }
        }
    }

    /// Creates a new item stack with custom enchantments.
    #[must_use]
    pub const fn new_enchanted(item: u32, count: u16, enchantments: CompactEnchantments) -> Self {
        if item == 0 || count == 0 {
            Self::EMPTY
        } else {
            Self {
                item,
                count,
                damage: 0,
                enchantments,
            }
        }
    }

    /// Creates a new item stack with custom damage and enchantments.
    #[must_use]
    pub const fn new_with_damage(
        item: u32,
        count: u16,
        damage: u16,
        enchantments: CompactEnchantments,
    ) -> Self {
        if item == 0 || count == 0 {
            Self::EMPTY
        } else {
            Self {
                item,
                count,
                damage,
                enchantments,
            }
        }
    }

    /// Builder method to attach a damage value to this item stack.
    #[must_use]
    pub const fn with_damage(mut self, damage: u16) -> Self {
        self.damage = damage;
        self
    }

    /// Whether this item stack is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.item == 0 || self.count == 0
    }

    /// Normalizes empty states so `count == 0` strictly implies `item == 0`.
    pub fn normalize(&mut self) {
        if self.item == 0 || self.count == 0 {
            self.item = 0;
            self.count = 0;
            self.damage = 0;
            self.enchantments = CompactEnchantments::EMPTY;
        }
    }

    /// Checks if this stack can combine with another stack without exceeding the limit.
    #[must_use]
    pub fn can_stack(&self, other: &Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.item == other.item
            && self.damage == other.damage
            && self.enchantments == other.enchantments
            && self.count + other.count <= MAX_STACK_SIZE
    }
}

/// Mouse button used during an inventory click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickButton {
    /// Left mouse button (0).
    Left,
    /// Right mouse button (1).
    Right,
}

/// Click interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode {
    /// Normal pickup, placement, or stack merging (left/right click).
    Pickup,
    /// Quick transfer between hotbar and main inventory (shift-click).
    QuickMove,
    /// Swap with hotbar slot via number key.
    SwapHotbar,
    /// Drop item stack into world.
    Drop,
}

/// Errors returned by inventory manipulation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    /// Slot index was out of bounds for the container.
    #[error("Slot index {0} is out of bounds")]
    SlotOutOfBounds(usize),
    /// Slot does not accept the item.
    #[error("Slot {0} does not accept item {1}")]
    InvalidItemForSlot(usize, u32),
}

/// Player inventory component containing 46 item slots and a cursor carried stack.
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct Inventory {
    /// 46 slots:
    /// - 0..9: Hotbar (9 slots)
    /// - 9..36: Main storage (27 slots)
    /// - 36..40: Armor (4 slots)
    /// - 40..44: Crafting inputs (4 slots)
    /// - 44: Crafting result (1 slot)
    /// - 45: Offhand (1 slot)
    pub slots: [ItemStack; PLAYER_INVENTORY_SLOTS],
    /// Currently selected hotbar slot index (0..=8).
    pub selected_slot: usize,
    /// Item stack currently held on the cursor.
    pub carried: ItemStack,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [ItemStack::EMPTY; PLAYER_INVENTORY_SLOTS],
            selected_slot: 0,
            carried: ItemStack::EMPTY,
        }
    }
}

impl Inventory {
    /// Returns the currently active item stack selected on the hotbar.
    #[must_use]
    pub fn selected_item(&self) -> &ItemStack {
        let idx = self.selected_slot.min(8);
        &self.slots[idx]
    }

    /// Gets a reference to an inventory slot.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }

    /// Gets a mutable reference to an inventory slot.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(slot)
    }

    /// Updates the crafting result slot (44) based on crafting input slots (40..44) using standard recipes.
    pub fn update_crafting(&mut self) {
        self.update_crafting_with_registry(&RecipeRegistry::standard());
    }

    /// Updates the crafting result slot (44) based on crafting input slots (40..44) using the provided recipe registry.
    pub fn update_crafting_with_registry(&mut self, registry: &RecipeRegistry) {
        let inputs = [
            self.slots[40],
            self.slots[41],
            self.slots[42],
            self.slots[43],
        ];
        self.slots[CRAFTING_RESULT_SLOT] =
            find_recipe_2x2_with_registry(&inputs, registry).unwrap_or(ItemStack::EMPTY);
    }

    /// Inserts as much of `stack` as possible into the specified slot range.
    ///
    /// Returns the number of items successfully inserted.
    pub fn insert_into_range(&mut self, stack: &mut ItemStack, range: Range<usize>) -> u16 {
        if stack.is_empty() {
            return 0;
        }
        let initial_count = stack.count;

        // 1. Merge into existing matching stacks
        for i in range.clone() {
            let target = &mut self.slots[i];
            if target.item == stack.item && target.count < MAX_STACK_SIZE {
                let space = MAX_STACK_SIZE - target.count;
                let to_move = stack.count.min(space);
                target.count += to_move;
                stack.count -= to_move;
                if stack.count == 0 {
                    stack.normalize();
                    return initial_count;
                }
            }
        }

        // 2. Place remainder into empty slots
        for i in range {
            let target = &mut self.slots[i];
            if target.is_empty() {
                *target = *stack;
                stack.count = 0;
                stack.normalize();
                return initial_count;
            }
        }

        stack.normalize();
        initial_count - stack.count
    }

    /// Inserts a stack into main storage (9..36) first, then hotbar (0..9).
    pub fn insert_into_storage_or_hotbar(&mut self, stack: &mut ItemStack) -> u16 {
        let mut inserted = self.insert_into_range(stack, STORAGE_SLOTS);
        if !stack.is_empty() {
            inserted += self.insert_into_range(stack, HOTBAR_SLOTS);
        }
        inserted
    }

    /// Attempts to add an item to storage or hotbar, returning the number of items successfully added.
    pub fn try_add_item(&mut self, item: u32, count: u16) -> u16 {
        let mut stack = ItemStack::new(item, count);
        self.insert_into_storage_or_hotbar(&mut stack)
    }

    /// Returns any items remaining in crafting inputs (40..44) into storage or hotbar.
    ///
    /// Clears the crafting result slot (44).
    pub fn return_crafting_items(&mut self) {
        self.return_crafting_items_with_registry(&RecipeRegistry::standard());
    }

    /// Returns any items remaining in crafting inputs (40..44) into storage or hotbar using the provided registry.
    pub fn return_crafting_items_with_registry(&mut self, registry: &RecipeRegistry) {
        for i in CRAFTING_INPUT_SLOTS {
            if !self.slots[i].is_empty() {
                let mut stack = self.slots[i];
                self.insert_into_storage_or_hotbar(&mut stack);
                self.slots[i] = stack; // If full, whatever remains stays
            }
        }
        self.update_crafting_with_registry(registry);
    }

    /// Returns carried cursor stack into storage or hotbar.
    pub fn return_carried(&mut self) {
        if !self.carried.is_empty() {
            let mut stack = self.carried;
            self.insert_into_storage_or_hotbar(&mut stack);
            self.carried = stack;
        }
    }

    /// Returns both crafting items and carried cursor stack into storage or hotbar.
    pub fn return_crafting_and_carried(&mut self) {
        self.return_carried();
        self.return_crafting_items();
    }
}

/// Executes a deterministic inventory click on an inventory container using standard recipes.
///
/// Modifies the target slot and the `carried` cursor stack according to standard rules.
pub fn inventory_click(
    inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
    mode: ClickMode,
) -> Result<(), InventoryError> {
    inventory_click_with_registry(inv, slot_idx, button, mode, &RecipeRegistry::standard())
}

/// Executes a deterministic inventory click on an inventory container using the provided recipe registry.
#[allow(clippy::too_many_lines)]
pub fn inventory_click_with_registry(
    inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
    mode: ClickMode,
    registry: &RecipeRegistry,
) -> Result<(), InventoryError> {
    if slot_idx >= PLAYER_INVENTORY_SLOTS {
        return Err(InventoryError::SlotOutOfBounds(slot_idx));
    }

    // Special handling: Crafting Result Slot (44)
    if slot_idx == CRAFTING_RESULT_SLOT {
        handle_crafting_result_click(inv, button, mode, registry);
        return Ok(());
    }

    match mode {
        ClickMode::Pickup => handle_pickup_click(inv, slot_idx, button)?,
        ClickMode::QuickMove => handle_quick_move_click(inv, slot_idx),
        ClickMode::SwapHotbar => handle_swap_hotbar_click(inv, slot_idx, button)?,
        ClickMode::Drop => handle_drop_click(inv, slot_idx, button),
    }

    // Re-evaluate crafting if any crafting input slot changed
    if CRAFTING_INPUT_SLOTS.contains(&slot_idx) {
        inv.update_crafting_with_registry(registry);
    }

    Ok(())
}

/// Handles clicking on the crafting result slot (44).
fn handle_crafting_result_click(
    inv: &mut Inventory,
    _button: ClickButton,
    mode: ClickMode,
    registry: &RecipeRegistry,
) {
    let result = inv.slots[CRAFTING_RESULT_SLOT];
    if result.is_empty() {
        return;
    }

    match mode {
        ClickMode::Pickup => {
            // Can only take result if carried is empty or matches result item and can hold it
            if inv.carried.is_empty() {
                inv.carried = result;
            } else if inv.carried.item == result.item
                && inv.carried.count + result.count <= MAX_STACK_SIZE
            {
                inv.carried.count += result.count;
            } else {
                return; // Cannot take result
            }

            let inputs = [inv.slots[40], inv.slots[41], inv.slots[42], inv.slots[43]];
            let remainder = registry
                .find_match_2x2(&inputs)
                .and_then(crate::crafting::CraftingRecipe::remainder_item);

            // Deduct 1 item from each non-empty crafting input slot
            for i in CRAFTING_INPUT_SLOTS {
                if !inv.slots[i].is_empty() {
                    inv.slots[i].count -= 1;
                    if inv.slots[i].count == 0 {
                        inv.slots[i] =
                            remainder.map_or(ItemStack::EMPTY, |rem| ItemStack::new(rem, 1));
                    }
                }
            }
            inv.update_crafting_with_registry(registry);
        }
        ClickMode::QuickMove => {
            // Shift-click: craft as many times as possible into storage/hotbar
            for _ in 0..64 {
                let res = inv.slots[CRAFTING_RESULT_SLOT];
                if res.is_empty() {
                    break;
                }
                let mut to_insert = res;
                let inserted = inv.insert_into_storage_or_hotbar(&mut to_insert);
                if inserted < res.count {
                    // Could not insert full result: roll back partial insert if needed
                    break;
                }

                let inputs = [inv.slots[40], inv.slots[41], inv.slots[42], inv.slots[43]];
                let remainder = registry
                    .find_match_2x2(&inputs)
                    .and_then(crate::crafting::CraftingRecipe::remainder_item);

                // Deduct 1 item from each crafting input
                for i in CRAFTING_INPUT_SLOTS {
                    if !inv.slots[i].is_empty() {
                        inv.slots[i].count -= 1;
                        if inv.slots[i].count == 0 {
                            inv.slots[i] =
                                remainder.map_or(ItemStack::EMPTY, |rem| ItemStack::new(rem, 1));
                        }
                    }
                }
                inv.update_crafting_with_registry(registry);
            }
        }
        ClickMode::SwapHotbar | ClickMode::Drop => {
            // Not supported on crafting result
        }
    }
}

/// Handles standard pickup click (left or right click).
fn handle_pickup_click(
    inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
) -> Result<(), InventoryError> {
    match button {
        ClickButton::Left => {
            let slot = inv.slots[slot_idx];
            if inv.carried.is_empty() {
                // Pick up entire stack
                inv.carried = slot;
                inv.slots[slot_idx] = ItemStack::EMPTY;
            } else if slot.is_empty() {
                // Check if target slot accepts carried item (e.g. armor slots)
                if !is_slot_valid_for_item(slot_idx, inv.carried.item) {
                    return Err(InventoryError::InvalidItemForSlot(
                        slot_idx,
                        inv.carried.item,
                    ));
                }
                inv.slots[slot_idx] = inv.carried;
                inv.carried = ItemStack::EMPTY;
            } else if slot.item == inv.carried.item {
                // Merge stacks up to 64
                let space = MAX_STACK_SIZE.saturating_sub(slot.count);
                let to_move = inv.carried.count.min(space);
                inv.slots[slot_idx].count += to_move;
                inv.carried.count -= to_move;
                inv.carried.normalize();
            } else {
                // Swap slot and carried - verify carried item is valid for this slot
                if !is_slot_valid_for_item(slot_idx, inv.carried.item) {
                    return Err(InventoryError::InvalidItemForSlot(
                        slot_idx,
                        inv.carried.item,
                    ));
                }
                inv.slots[slot_idx] = inv.carried;
                inv.carried = slot;
            }
        }
        ClickButton::Right => {
            let slot = inv.slots[slot_idx];
            if inv.carried.is_empty() {
                if !slot.is_empty() {
                    // Take half of the slot (rounded up)
                    let take = slot.count.div_ceil(2);
                    inv.carried = ItemStack::new(slot.item, take);
                    inv.slots[slot_idx].count -= take;
                    inv.slots[slot_idx].normalize();
                }
            } else if slot.is_empty() {
                // Check if target slot accepts carried item
                if !is_slot_valid_for_item(slot_idx, inv.carried.item) {
                    return Err(InventoryError::InvalidItemForSlot(
                        slot_idx,
                        inv.carried.item,
                    ));
                }
                // Place 1 item into empty slot
                inv.slots[slot_idx] = ItemStack::new(inv.carried.item, 1);
                inv.carried.count -= 1;
                inv.carried.normalize();
            } else if slot.item == inv.carried.item && slot.count < MAX_STACK_SIZE {
                // Place 1 item into matching slot
                inv.slots[slot_idx].count += 1;
                inv.carried.count -= 1;
                inv.carried.normalize();
            } else {
                // Swap slot and carried
                if !is_slot_valid_for_item(slot_idx, inv.carried.item) {
                    return Err(InventoryError::InvalidItemForSlot(
                        slot_idx,
                        inv.carried.item,
                    ));
                }
                inv.slots[slot_idx] = inv.carried;
                inv.carried = slot;
            }
        }
    }
    Ok(())
}

/// Handles shift-click quick move transfer.
fn handle_quick_move_click(inv: &mut Inventory, slot_idx: usize) {
    let mut source = inv.slots[slot_idx];
    if source.is_empty() {
        return;
    }

    if HOTBAR_SLOTS.contains(&slot_idx) {
        // From Hotbar: check if armor first
        if let Some(armor_slot) = matching_armor_slot(source.item)
            && inv.slots[armor_slot].is_empty()
        {
            inv.slots[armor_slot] = ItemStack::new(source.item, 1);
            source.count -= 1;
            source.normalize();
            inv.slots[slot_idx] = source;
            return;
        }
        // Move to main storage
        inv.insert_into_range(&mut source, STORAGE_SLOTS);
        inv.slots[slot_idx] = source;
    } else if STORAGE_SLOTS.contains(&slot_idx) {
        // From Storage: check if armor first
        if let Some(armor_slot) = matching_armor_slot(source.item)
            && inv.slots[armor_slot].is_empty()
        {
            inv.slots[armor_slot] = ItemStack::new(source.item, 1);
            source.count -= 1;
            source.normalize();
            inv.slots[slot_idx] = source;
            return;
        }
        // Move to hotbar
        inv.insert_into_range(&mut source, HOTBAR_SLOTS);
        inv.slots[slot_idx] = source;
    } else if ARMOR_SLOTS.contains(&slot_idx)
        || CRAFTING_INPUT_SLOTS.contains(&slot_idx)
        || slot_idx == OFFHAND_SLOT
    {
        // From Armor, Crafting Inputs, or Offhand: move to storage then hotbar
        inv.insert_into_storage_or_hotbar(&mut source);
        inv.slots[slot_idx] = source;
    }
}

/// Handles number key hotbar swap.
fn handle_swap_hotbar_click(
    inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
) -> Result<(), InventoryError> {
    let hotbar_slot = match button {
        ClickButton::Left => inv.selected_slot.min(8),
        ClickButton::Right => 0,
    };

    // If slot is an armor slot, check compatibility
    if ARMOR_SLOTS.contains(&slot_idx) {
        let hotbar_item = inv.slots[hotbar_slot].item;
        if !is_slot_valid_for_item(slot_idx, hotbar_item) {
            return Err(InventoryError::InvalidItemForSlot(slot_idx, hotbar_item));
        }
    }

    inv.slots.swap(slot_idx, hotbar_slot);
    Ok(())
}

/// Handles dropping items.
fn handle_drop_click(inv: &mut Inventory, slot_idx: usize, button: ClickButton) {
    match button {
        ClickButton::Left => {
            inv.slots[slot_idx] = ItemStack::EMPTY;
        }
        ClickButton::Right => {
            if inv.slots[slot_idx].count > 1 {
                inv.slots[slot_idx].count -= 1;
            } else {
                inv.slots[slot_idx] = ItemStack::EMPTY;
            }
        }
    }
}

/// Returns the item stack dropped when a block with `block_id` is broken in survival mode.
#[must_use]
pub const fn block_to_drop_item(block_id: u32) -> Option<ItemStack> {
    match block_id {
        0 | 4 | 6 | 8 | 9 | 31..=34 | 37..=50 | 80 => None, // Air, bedrock, fluids, leaves, glass, spawners drop nothing
        1 | 35 | 79 => Some(ItemStack::new(4, 1)), // Stone / Cobblestone / Mossy Cobblestone -> Cobblestone
        2 | 3 => Some(ItemStack::new(2, 1)),       // Dirt / Grass Block -> Dirt
        5 => Some(ItemStack::new(12, 1)),          // Sand
        7 => Some(ItemStack::new(7, 1)),           // Planks
        10 => Some(ItemStack::new(10, 1)),         // Slab
        11 => Some(ItemStack::new(11, 1)),         // Stairs
        12 => Some(ItemStack::new(54, 1)),         // Coal ore -> Coal
        13 => Some(ItemStack::new(52, 1)),         // Iron ore -> Iron Ingot
        14 => Some(ItemStack::new(53, 1)),         // Gold ore -> Gold Ingot
        15 => Some(ItemStack::new(20, 1)),         // Diamond ore -> Diamond
        36 => Some(ItemStack::new(36, 1)),         // Obsidian
        74..=76 => Some(ItemStack::new(5, 1)),     // Logs -> Wood Log
        81 => Some(ItemStack::new(ITEM_CHEST, 1)), // Chest
        82 | 83 => Some(ItemStack::new(ITEM_FURNACE, 1)), // Furnace / Lit Furnace
        84 => Some(ItemStack::new(ITEM_CRAFTING_TABLE, 1)), // Crafting Table
        90 => Some(ItemStack::new(ITEM_NETHER_QUARTZ, 1)), // Nether Quartz Ore -> Nether Quartz
        91 => Some(ItemStack::new(ITEM_GOLD_NUGGET, 4)), // Nether Gold Ore -> 4 Gold Nuggets
        other => Some(ItemStack::new(other, 1)),   // Default self-drop
    }
}

/// Returns the item stack dropped when a block with `block_id` is broken using `held_tool`.
/// Respects tool tier requirements, Silk Touch, and Fortune.
#[must_use]
pub fn block_to_drop_with_tool(
    block_id: u32,
    held_tool: ItemStack,
    rng_sample: f32,
) -> Option<ItemStack> {
    let has_silk =
        held_tool.enchantments.get_level(crate::enchantment::EnchantmentKind::SilkTouch) > 0;
    let fortune = held_tool.enchantments.get_level(crate::enchantment::EnchantmentKind::Fortune);
    let p_tier = pickaxe_tier(held_tool.item);

    // If block drops nothing at all (air, fluids, bedrock, leaves, spawner, etc.)
    match block_id {
        0 | 6 | 8 | 31..=34 | 37..=50 | 80 => return None,
        _ => {}
    }

    // Glass: drops only with Silk Touch
    if block_id == 9 {
        return if has_silk {
            Some(ItemStack::new(9, 1))
        } else {
            None
        };
    }

    // Grass block: Silk touch drops grass block (3), otherwise dirt (2)
    if block_id == 3 {
        return if has_silk {
            Some(ItemStack::new(3, 1))
        } else {
            Some(ItemStack::new(2, 1))
        };
    }

    // Stone / Cobblestone / Mossy Cobblestone
    if block_id == 1 || block_id == 35 || block_id == 79 {
        let _tier = p_tier?;
        return if has_silk && block_id == 1 {
            Some(ItemStack::new(1, 1))
        } else {
            Some(ItemStack::new(4, 1))
        };
    }

    // Cobblestone block (4)
    if block_id == 4 {
        p_tier?;
        return Some(ItemStack::new(4, 1));
    }

    // Coal ore (12): Needs any pickaxe
    if block_id == 12 {
        p_tier?;
        return if has_silk {
            Some(ItemStack::new(12, 1))
        } else {
            let count = crate::enchantment::calculate_fortune_drop_count(1, fortune, rng_sample);
            Some(ItemStack::new(54, count))
        };
    }

    // Iron ore (13): Needs Stone or Iron pickaxe
    if block_id == 13 {
        let tier = p_tier?;
        if tier < ToolTier::Stone {
            return None;
        }
        return if has_silk {
            Some(ItemStack::new(13, 1))
        } else {
            Some(ItemStack::new(52, 1))
        };
    }

    // Gold ore (14): Needs Iron pickaxe
    if block_id == 14 {
        let tier = p_tier?;
        if tier < ToolTier::Iron {
            return None;
        }
        return if has_silk {
            Some(ItemStack::new(14, 1))
        } else {
            Some(ItemStack::new(53, 1))
        };
    }

    // Diamond ore (15): Needs Iron pickaxe
    if block_id == 15 {
        let tier = p_tier?;
        if tier < ToolTier::Iron {
            return None;
        }
        return if has_silk {
            Some(ItemStack::new(15, 1))
        } else {
            let count = crate::enchantment::calculate_fortune_drop_count(1, fortune, rng_sample);
            Some(ItemStack::new(20, count))
        };
    }

    // Obsidian (36): Needs Iron pickaxe
    if block_id == 36 {
        let tier = p_tier?;
        if tier < ToolTier::Iron {
            return None;
        }
        return Some(ItemStack::new(36, 1));
    }

    // Nether Quartz Ore (90): Needs any pickaxe
    if block_id == 90 {
        p_tier?;
        return if has_silk {
            Some(ItemStack::new(90, 1))
        } else {
            let count = crate::enchantment::calculate_fortune_drop_count(1, fortune, rng_sample);
            Some(ItemStack::new(ITEM_NETHER_QUARTZ, count))
        };
    }

    // Nether Gold Ore (91): Needs any pickaxe
    if block_id == 91 {
        p_tier?;
        return if has_silk {
            Some(ItemStack::new(91, 1))
        } else {
            let count = crate::enchantment::calculate_fortune_drop_count(4, fortune, rng_sample);
            Some(ItemStack::new(ITEM_GOLD_NUGGET, count))
        };
    }

    // Furnace (82, 83)
    if block_id == 82 || block_id == 83 {
        p_tier?;
        return Some(ItemStack::new(ITEM_FURNACE, 1));
    }

    // Anvil (91)
    if block_id == 91 {
        p_tier?;
        return Some(ItemStack::new(ITEM_ANVIL, 1));
    }

    // Fallback for non-pickaxe-gated blocks
    block_to_drop_item(block_id)
}

/// Number of item slots in a chest container.
pub const CHEST_CONTAINER_SLOTS: usize = 27;

/// Total number of slots in a dual chest container window (27 chest + 27 storage + 9 hotbar = 63).
pub const DUAL_CONTAINER_SLOTS: usize = 63;

/// Chest container slot range (0..27).
pub const CONTAINER_CHEST_SLOTS: Range<usize> = 0..27;
/// Player storage slot range in dual container window (27..54).
pub const CONTAINER_PLAYER_STORAGE_SLOTS: Range<usize> = 27..54;
/// Player hotbar slot range in dual container window (54..63).
pub const CONTAINER_PLAYER_HOTBAR_SLOTS: Range<usize> = 54..63;

/// Chest container inventory component holding 27 item slots.
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct ChestInventory {
    /// 27 container slots.
    pub slots: [ItemStack; CHEST_CONTAINER_SLOTS],
    /// Optional custom display title.
    pub custom_name: Option<String>,
}

impl Default for ChestInventory {
    fn default() -> Self {
        Self {
            slots: [ItemStack::EMPTY; CHEST_CONTAINER_SLOTS],
            custom_name: None,
        }
    }
}

impl ChestInventory {
    /// Creates a new empty chest inventory.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `ChestInventory` from `BlockEntityData`.
    #[must_use]
    pub fn from_block_entity(be: &BlockEntityData) -> Self {
        match be {
            BlockEntityData::Chest { custom_name, items } => {
                let mut slots = [ItemStack::EMPTY; CHEST_CONTAINER_SLOTS];
                for (i, slot) in items.iter().enumerate() {
                    if i < CHEST_CONTAINER_SLOTS && !slot.is_empty() {
                        slots[i] = ItemStack::new(slot.item, slot.count);
                    }
                }
                Self {
                    slots,
                    custom_name: custom_name.clone(),
                }
            }
            BlockEntityData::Furnace { .. } => Self::default(),
        }
    }

    /// Converts this `ChestInventory` to `BlockEntityData`.
    #[must_use]
    pub fn to_block_entity(&self) -> BlockEntityData {
        let mut items = [BlockEntitySlot::EMPTY; CHEST_CONTAINER_SLOTS];
        #[allow(clippy::cast_possible_truncation)]
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.is_empty() {
                items[i].slot = i as u8;
            } else {
                items[i] = BlockEntitySlot::new(i as u8, slot.item, slot.count);
            }
        }
        BlockEntityData::Chest {
            custom_name: self.custom_name.clone(),
            items,
        }
    }

    /// Returns whether this container has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(ItemStack::is_empty)
    }

    /// Gets a reference to a slot.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }

    /// Gets a mutable reference to a slot.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(slot)
    }
}

fn get_container_slot_mut<'a>(
    chest_slots: &'a mut [ItemStack; CHEST_CONTAINER_SLOTS],
    player_slots: &'a mut [ItemStack; PLAYER_INVENTORY_SLOTS],
    slot_idx: usize,
) -> &'a mut ItemStack {
    if slot_idx < CHEST_CONTAINER_SLOTS {
        &mut chest_slots[slot_idx]
    } else if slot_idx < 54 {
        &mut player_slots[slot_idx - 27 + 9]
    } else {
        &mut player_slots[slot_idx - 54]
    }
}

/// Executes a deterministic click interaction on a dual container window (chest + player inventory).
pub fn container_click(
    container: &mut ChestInventory,
    player_inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
    mode: ClickMode,
) -> Result<(), InventoryError> {
    if slot_idx >= DUAL_CONTAINER_SLOTS {
        return Err(InventoryError::SlotOutOfBounds(slot_idx));
    }

    match mode {
        ClickMode::Pickup => handle_container_pickup(container, player_inv, slot_idx, button),
        ClickMode::QuickMove => handle_container_quick_move(container, player_inv, slot_idx),
        ClickMode::SwapHotbar => {
            handle_container_swap_hotbar(container, player_inv, slot_idx, button);
        }
        ClickMode::Drop => handle_container_drop(container, player_inv, slot_idx, button),
    }

    Ok(())
}

fn handle_container_pickup(
    container: &mut ChestInventory,
    player_inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
) {
    let target = get_container_slot_mut(&mut container.slots, &mut player_inv.slots, slot_idx);
    match button {
        ClickButton::Left => {
            if player_inv.carried.is_empty() {
                player_inv.carried = *target;
                *target = ItemStack::EMPTY;
            } else if target.is_empty() {
                *target = player_inv.carried;
                player_inv.carried = ItemStack::EMPTY;
            } else if target.item == player_inv.carried.item {
                let space = MAX_STACK_SIZE.saturating_sub(target.count);
                let to_move = player_inv.carried.count.min(space);
                target.count += to_move;
                player_inv.carried.count -= to_move;
                player_inv.carried.normalize();
            } else {
                std::mem::swap(target, &mut player_inv.carried);
            }
        }
        ClickButton::Right => {
            if player_inv.carried.is_empty() {
                if !target.is_empty() {
                    let take = target.count.div_ceil(2);
                    player_inv.carried = ItemStack::new(target.item, take);
                    target.count -= take;
                    target.normalize();
                }
            } else if target.is_empty() {
                *target = ItemStack::new(player_inv.carried.item, 1);
                player_inv.carried.count -= 1;
                player_inv.carried.normalize();
            } else if target.item == player_inv.carried.item && target.count < MAX_STACK_SIZE {
                target.count += 1;
                player_inv.carried.count -= 1;
                player_inv.carried.normalize();
            }
        }
    }
}

fn handle_container_quick_move(
    container: &mut ChestInventory,
    player_inv: &mut Inventory,
    slot_idx: usize,
) {
    if slot_idx < CHEST_CONTAINER_SLOTS {
        // Move from chest into player inventory (storage first, then hotbar)
        let slot = &mut container.slots[slot_idx];
        if !slot.is_empty() {
            let mut to_move = *slot;
            player_inv.insert_into_storage_or_hotbar(&mut to_move);
            *slot = to_move;
        }
    } else {
        // Move from player inventory into chest
        let player_slot_idx = if slot_idx < 54 {
            slot_idx - 27 + 9
        } else {
            slot_idx - 54
        };
        let slot = &mut player_inv.slots[player_slot_idx];
        if !slot.is_empty() {
            let mut to_move = *slot;
            // 1. Merge into matching slots in chest
            for target in &mut container.slots {
                if target.item == to_move.item && target.count < MAX_STACK_SIZE {
                    let space = MAX_STACK_SIZE - target.count;
                    let move_amt = to_move.count.min(space);
                    target.count += move_amt;
                    to_move.count -= move_amt;
                    if to_move.count == 0 {
                        to_move.normalize();
                        break;
                    }
                }
            }
            // 2. Place remainder into empty slots in chest
            if !to_move.is_empty() {
                for target in &mut container.slots {
                    if target.is_empty() {
                        *target = to_move;
                        to_move = ItemStack::EMPTY;
                        break;
                    }
                }
            }
            *slot = to_move;
        }
    }
}

fn handle_container_swap_hotbar(
    container: &mut ChestInventory,
    player_inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
) {
    let hotbar_slot = match button {
        ClickButton::Left => player_inv.selected_slot.min(8),
        ClickButton::Right => return,
    };
    if slot_idx < CHEST_CONTAINER_SLOTS {
        std::mem::swap(
            &mut container.slots[slot_idx],
            &mut player_inv.slots[hotbar_slot],
        );
    } else if slot_idx < 54 {
        let player_slot = slot_idx - 27 + 9;
        let (left, right) = match player_slot.cmp(&hotbar_slot) {
            std::cmp::Ordering::Less => {
                let (l, r) = player_inv.slots.split_at_mut(hotbar_slot);
                (&mut l[player_slot], &mut r[0])
            }
            std::cmp::Ordering::Greater => {
                let (l, r) = player_inv.slots.split_at_mut(player_slot);
                (&mut l[hotbar_slot], &mut r[0])
            }
            std::cmp::Ordering::Equal => return,
        };
        std::mem::swap(left, right);
    }
}

fn handle_container_drop(
    container: &mut ChestInventory,
    player_inv: &mut Inventory,
    slot_idx: usize,
    button: ClickButton,
) {
    let target = get_container_slot_mut(&mut container.slots, &mut player_inv.slots, slot_idx);
    if target.is_empty() {
        return;
    }
    match button {
        ClickButton::Left => {
            target.count -= 1;
            target.normalize();
        }
        ClickButton::Right => {
            *target = ItemStack::EMPTY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_left_click_pickup_and_place() {
        let mut inv = Inventory::default();
        inv.slots[0] = ItemStack::new(1, 10); // 10 stone in slot 0

        // Left click on slot 0: picks up all 10 stone
        inventory_click(&mut inv, 0, ClickButton::Left, ClickMode::Pickup).unwrap();
        assert_eq!(inv.slots[0], ItemStack::EMPTY);
        assert_eq!(inv.carried, ItemStack::new(1, 10));

        // Left click on slot 1: places all 10 stone
        inventory_click(&mut inv, 1, ClickButton::Left, ClickMode::Pickup).unwrap();
        assert_eq!(inv.slots[1], ItemStack::new(1, 10));
        assert_eq!(inv.carried, ItemStack::EMPTY);
    }

    #[test]
    fn test_right_click_split_and_place_one() {
        let mut inv = Inventory::default();
        inv.slots[0] = ItemStack::new(1, 7); // 7 stone

        // Right click slot 0: takes half (4 stone into carried, 3 left in slot)
        inventory_click(&mut inv, 0, ClickButton::Right, ClickMode::Pickup).unwrap();
        assert_eq!(inv.slots[0], ItemStack::new(1, 3));
        assert_eq!(inv.carried, ItemStack::new(1, 4));

        // Right click empty slot 1: places 1 stone (carried now 3, slot 1 has 1)
        inventory_click(&mut inv, 1, ClickButton::Right, ClickMode::Pickup).unwrap();
        assert_eq!(inv.slots[1], ItemStack::new(1, 1));
        assert_eq!(inv.carried, ItemStack::new(1, 3));
    }

    #[test]
    fn test_quick_move_hotbar_to_storage() {
        let mut inv = Inventory::default();
        inv.slots[0] = ItemStack::new(2, 64); // 64 dirt in hotbar slot 0

        // Shift click slot 0: moves into main storage slot 9
        inventory_click(&mut inv, 0, ClickButton::Left, ClickMode::QuickMove).unwrap();
        assert_eq!(inv.slots[0], ItemStack::EMPTY);
        assert_eq!(inv.slots[9], ItemStack::new(2, 64));

        // Shift click slot 9: moves back to hotbar slot 0
        inventory_click(&mut inv, 9, ClickButton::Left, ClickMode::QuickMove).unwrap();
        assert_eq!(inv.slots[9], ItemStack::EMPTY);
        assert_eq!(inv.slots[0], ItemStack::new(2, 64));
    }

    #[test]
    fn test_crafting_interaction() {
        let mut inv = Inventory::default();
        // Place 1 oak log (5) in crafting input slot 40
        inv.slots[40] = ItemStack::new(5, 1);
        inv.update_crafting();
        assert_eq!(inv.slots[CRAFTING_RESULT_SLOT], ItemStack::new(7, 4)); // 4 planks

        // Click on result slot 44: picks up 4 planks, consumes 1 log
        inventory_click(
            &mut inv,
            CRAFTING_RESULT_SLOT,
            ClickButton::Left,
            ClickMode::Pickup,
        )
        .unwrap();
        assert_eq!(inv.carried, ItemStack::new(7, 4));
        assert_eq!(inv.slots[40], ItemStack::EMPTY);
        assert_eq!(inv.slots[CRAFTING_RESULT_SLOT], ItemStack::EMPTY);
    }

    #[test]
    fn test_armor_slot_validation() {
        let mut inv = Inventory {
            carried: ItemStack::new(1, 1),
            ..Default::default()
        };
        let res = inventory_click(
            &mut inv,
            ARMOR_HELMET_SLOT,
            ClickButton::Left,
            ClickMode::Pickup,
        );
        assert!(res.is_err());

        // Carried iron helmet (16): can place in helmet slot 36
        inv.carried = ItemStack::new(16, 1);
        let res = inventory_click(
            &mut inv,
            ARMOR_HELMET_SLOT,
            ClickButton::Left,
            ClickMode::Pickup,
        );
        assert!(res.is_ok());
        assert_eq!(inv.slots[ARMOR_HELMET_SLOT], ItemStack::new(16, 1));
        assert_eq!(inv.carried, ItemStack::EMPTY);
    }

    #[test]
    fn test_quick_move_auto_equips_armor() {
        let mut inv = Inventory::default();
        // Iron chestplate in hotbar slot 0
        inv.slots[0] = ItemStack::new(17, 1);
        inventory_click(&mut inv, 0, ClickButton::Left, ClickMode::QuickMove).unwrap();
        assert_eq!(inv.slots[0], ItemStack::EMPTY);
        assert_eq!(inv.slots[ARMOR_CHESTPLATE_SLOT], ItemStack::new(17, 1));

        // Shift click armor slot: moves back to storage/hotbar
        inventory_click(
            &mut inv,
            ARMOR_CHESTPLATE_SLOT,
            ClickButton::Left,
            ClickMode::QuickMove,
        )
        .unwrap();
        assert_eq!(inv.slots[ARMOR_CHESTPLATE_SLOT], ItemStack::EMPTY);
        assert_eq!(inv.slots[9], ItemStack::new(17, 1)); // Into first storage slot
    }

    #[test]
    fn test_return_crafting_items_on_close() {
        let mut inv = Inventory::default();
        inv.slots[40] = ItemStack::new(7, 2); // 2 planks in input
        inv.slots[42] = ItemStack::new(7, 2); // 2 planks in input
        inv.update_crafting();
        assert_eq!(inv.slots[CRAFTING_RESULT_SLOT], ItemStack::new(13, 4)); // 4 sticks

        inv.return_crafting_and_carried();
        assert_eq!(inv.slots[40], ItemStack::EMPTY);
        assert_eq!(inv.slots[42], ItemStack::EMPTY);
        assert_eq!(inv.slots[CRAFTING_RESULT_SLOT], ItemStack::EMPTY);
        assert_eq!(inv.slots[9], ItemStack::new(7, 4)); // Combined into storage
    }

    #[test]
    fn test_chest_inventory_block_entity_roundtrip() {
        let mut chest = ChestInventory::new();
        chest.custom_name = Some("Loot Box".into());
        chest.slots[0] = ItemStack::new(1, 32);
        chest.slots[26] = ItemStack::new(ITEM_CHEST, 1);

        let be = chest.to_block_entity();
        let restored = ChestInventory::from_block_entity(&be);
        assert_eq!(restored.custom_name.as_deref(), Some("Loot Box"));
        assert_eq!(restored.slots[0], ItemStack::new(1, 32));
        assert_eq!(restored.slots[26], ItemStack::new(ITEM_CHEST, 1));
        assert!(restored.slots[1].is_empty());
    }

    #[test]
    fn test_container_click_pickup_and_split() {
        let mut chest = ChestInventory::new();
        let mut player_inv = Inventory::default();

        // Put 10 iron ingots (52) into chest slot 0
        chest.slots[0] = ItemStack::new(52, 10);

        // Left click on chest slot 0: picks up all 10
        container_click(
            &mut chest,
            &mut player_inv,
            0,
            ClickButton::Left,
            ClickMode::Pickup,
        )
        .unwrap();
        assert_eq!(chest.slots[0], ItemStack::EMPTY);
        assert_eq!(player_inv.carried, ItemStack::new(52, 10));

        // Right click on chest slot 1: places 1
        container_click(
            &mut chest,
            &mut player_inv,
            1,
            ClickButton::Right,
            ClickMode::Pickup,
        )
        .unwrap();
        assert_eq!(chest.slots[1], ItemStack::new(52, 1));
        assert_eq!(player_inv.carried, ItemStack::new(52, 9));

        // Place remainder into player storage slot (container slot 27 = player slot 9)
        container_click(
            &mut chest,
            &mut player_inv,
            27,
            ClickButton::Left,
            ClickMode::Pickup,
        )
        .unwrap();
        assert_eq!(player_inv.slots[9], ItemStack::new(52, 9));
        assert_eq!(player_inv.carried, ItemStack::EMPTY);
    }

    #[test]
    fn test_container_quick_move_bidirectional() {
        let mut chest = ChestInventory::new();
        let mut player_inv = Inventory::default();

        // 64 stone in chest slot 0
        chest.slots[0] = ItemStack::new(1, 64);

        // Shift click chest slot 0 -> moves into player storage (slot 9)
        container_click(
            &mut chest,
            &mut player_inv,
            0,
            ClickButton::Left,
            ClickMode::QuickMove,
        )
        .unwrap();
        assert_eq!(chest.slots[0], ItemStack::EMPTY);
        assert_eq!(player_inv.slots[9], ItemStack::new(1, 64));

        // Shift click container slot 27 (player storage slot 9) -> moves back into chest slot 0
        container_click(
            &mut chest,
            &mut player_inv,
            27,
            ClickButton::Left,
            ClickMode::QuickMove,
        )
        .unwrap();
        assert_eq!(player_inv.slots[9], ItemStack::EMPTY);
        assert_eq!(chest.slots[0], ItemStack::new(1, 64));
    }

    #[test]
    fn test_minecart_and_rail_item_properties() {
        assert!(is_minecart(ITEM_MINECART));
        assert!(!is_minecart(ITEM_RAIL));
        assert!(is_rail(ITEM_RAIL));
        assert!(is_rail(ITEM_POWERED_RAIL));
        assert!(is_rail(ITEM_DETECTOR_RAIL));
        assert!(is_rail(ITEM_ACTIVATOR_RAIL));
        assert!(!is_rail(ITEM_MINECART));

        assert_eq!(item_name(ITEM_MINECART), "Minecart");
        assert_eq!(item_name(ITEM_RAIL), "Rail");
        assert_eq!(item_name(ITEM_POWERED_RAIL), "Powered Rail");
        assert_eq!(item_name(ITEM_DETECTOR_RAIL), "Detector Rail");
        assert_eq!(item_name(ITEM_ACTIVATOR_RAIL), "Activator Rail");
    }

    #[test]
    fn test_flint_and_steel_item_properties() {
        assert!(is_flint_and_steel(ITEM_FLINT_AND_STEEL));
        assert!(!is_flint_and_steel(ITEM_FLINT));
        assert!(!is_flint_and_steel(ITEM_IRON_INGOT));

        assert_eq!(item_name(ITEM_FLINT), "Flint");
        assert_eq!(item_name(ITEM_FLINT_AND_STEEL), "Flint and Steel");
    }

    #[test]
    fn test_nether_ore_drop_mappings() {
        assert_eq!(
            block_to_drop_item(90),
            Some(ItemStack::new(ITEM_NETHER_QUARTZ, 1))
        );
        assert_eq!(
            block_to_drop_item(91),
            Some(ItemStack::new(ITEM_GOLD_NUGGET, 4))
        );
        assert_eq!(block_to_drop_item(86), Some(ItemStack::new(86, 1))); // Basalt
        assert_eq!(block_to_drop_item(87), Some(ItemStack::new(87, 1))); // Blackstone
        assert_eq!(block_to_drop_item(88), Some(ItemStack::new(88, 1))); // Magma block
        assert_eq!(block_to_drop_item(89), Some(ItemStack::new(89, 1))); // Soul soil
        assert_eq!(block_to_drop_item(92), Some(ItemStack::new(92, 1))); // Ancient debris

        assert_eq!(item_name(ITEM_NETHER_QUARTZ), "Nether Quartz");
        assert_eq!(item_name(ITEM_GOLD_NUGGET), "Gold Nugget");
    }

    #[test]
    fn test_durability_and_block_drops_with_tools() {
        // Durability
        let mut pick = ItemStack::new(ITEM_IRON_PICKAXE, 1);
        assert_eq!(pick.damage, 0);
        assert_eq!(max_durability(pick.item), Some(250));

        // Apply 1 damage
        let broken = apply_durability_damage(&mut pick, 1, 0.0);
        assert!(!broken);
        assert_eq!(pick.damage, 1);

        // Broken when reaching max durability
        let broken = apply_durability_damage(&mut pick, 250, 0.0);
        assert!(broken);
        assert!(pick.is_empty());

        // Unbreaking chance
        let mut pick_unbreaking = ItemStack::new(ITEM_IRON_PICKAXE, 1);
        assert!(pick_unbreaking.enchantments.set_enchantment(crate::enchantment::EnchantmentKind::Unbreaking, 3));
        // Level 3 unbreaking: ignore_chance = 3/4 = 0.75.
        // rng_sample = 0.5 < 0.75 -> ignored!
        let broken = apply_durability_damage(&mut pick_unbreaking, 1, 0.5);
        assert!(!broken);
        assert_eq!(pick_unbreaking.damage, 0);

        // rng_sample = 0.9 >= 0.75 -> applied!
        let broken = apply_durability_damage(&mut pick_unbreaking, 1, 0.9);
        assert!(!broken);
        assert_eq!(pick_unbreaking.damage, 1);

        // Tool tiers on ores:
        // Stone ore (13 - Iron Ore): needs at least Stone pickaxe
        let wood_pick = ItemStack::new(ITEM_WOODEN_PICKAXE, 1);
        let stone_pick = ItemStack::new(ITEM_STONE_PICKAXE, 1);
        let iron_pick = ItemStack::new(ITEM_IRON_PICKAXE, 1);

        assert_eq!(block_to_drop_with_tool(13, ItemStack::EMPTY, 0.0), None);
        assert_eq!(block_to_drop_with_tool(13, wood_pick, 0.0), None);
        assert_eq!(block_to_drop_with_tool(13, stone_pick, 0.0), Some(ItemStack::new(ITEM_RAW_IRON, 1))); // raw iron

        // Diamond Ore (15): needs at least Iron pickaxe
        assert_eq!(block_to_drop_with_tool(15, stone_pick, 0.0), None);
        assert_eq!(block_to_drop_with_tool(15, iron_pick, 0.0), Some(ItemStack::new(ITEM_DIAMOND, 1))); // diamond

        // Silk Touch on Diamond Ore
        let mut silk_pick = ItemStack::new(ITEM_IRON_PICKAXE, 1);
        assert!(silk_pick.enchantments.set_enchantment(crate::enchantment::EnchantmentKind::SilkTouch, 1));
        assert_eq!(block_to_drop_with_tool(15, silk_pick, 0.0), Some(ItemStack::new(15, 1))); // diamond ore block

        // Fortune III on Coal Ore (12)
        let mut fortune_pick = ItemStack::new(ITEM_IRON_PICKAXE, 1);
        assert!(fortune_pick.enchantments.set_enchantment(crate::enchantment::EnchantmentKind::Fortune, 3));
        // With rng_sample = 0.9 (selects max multiplier = 4)
        let drop = block_to_drop_with_tool(12, fortune_pick, 0.9).unwrap();
        assert_eq!(drop.item, ITEM_COAL); // coal
        assert_eq!(drop.count, 4);
    }
}
