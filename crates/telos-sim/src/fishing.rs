//! Fishing mechanics, aquatic loot tables, and bobber catch rolling.

use crate::inventory::{
    ITEM_BOW, ITEM_COD, ITEM_COOKED_COD, ITEM_ENCHANTED_BOOK, ITEM_FISHING_ROD, ITEM_GLASS_BOTTLE,
    ITEM_INK_SAC, ITEM_LEATHER_BOOTS, ITEM_STRING,
};

/// High-level category of an aquatic catch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingLootCategory {
    /// Common fish (cod, salmon, ink sac).
    Fish,
    /// Rare valuable items (enchanted books, bows, rods).
    Treasure,
    /// Common junk / debris (leather boots, strings, bottles).
    Junk,
}

/// A weighted fishing loot entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FishingLootEntry {
    /// Item identifier.
    pub item_id: u32,
    /// Canonical item identifier name in `telos` namespace.
    pub item_name: &'static str,
    /// Minimum stack count.
    pub min_count: u16,
    /// Maximum stack count.
    pub max_count: u16,
    /// Relative probability weight.
    pub weight: u32,
    /// Category of the loot.
    pub category: FishingLootCategory,
}

/// Fish pool entries (total weight: 100).
pub const FISH_LOOT_POOL: &[FishingLootEntry] = &[
    FishingLootEntry {
        item_id: ITEM_COD,
        item_name: "cod",
        min_count: 1,
        max_count: 1,
        weight: 65,
        category: FishingLootCategory::Fish,
    },
    FishingLootEntry {
        item_id: ITEM_COOKED_COD,
        item_name: "cooked_cod",
        min_count: 1,
        max_count: 1,
        weight: 20,
        category: FishingLootCategory::Fish,
    },
    FishingLootEntry {
        item_id: ITEM_INK_SAC,
        item_name: "ink_sac",
        min_count: 1,
        max_count: 2,
        weight: 15,
        category: FishingLootCategory::Fish,
    },
];

/// Treasure pool entries (total weight: 100).
pub const TREASURE_LOOT_POOL: &[FishingLootEntry] = &[
    FishingLootEntry {
        item_id: ITEM_ENCHANTED_BOOK,
        item_name: "enchanted_book",
        min_count: 1,
        max_count: 1,
        weight: 40,
        category: FishingLootCategory::Treasure,
    },
    FishingLootEntry {
        item_id: ITEM_BOW,
        item_name: "bow",
        min_count: 1,
        max_count: 1,
        weight: 35,
        category: FishingLootCategory::Treasure,
    },
    FishingLootEntry {
        item_id: ITEM_FISHING_ROD,
        item_name: "fishing_rod",
        min_count: 1,
        max_count: 1,
        weight: 25,
        category: FishingLootCategory::Treasure,
    },
];

/// Junk pool entries (total weight: 100).
pub const JUNK_LOOT_POOL: &[FishingLootEntry] = &[
    FishingLootEntry {
        item_id: ITEM_STRING,
        item_name: "string",
        min_count: 1,
        max_count: 2,
        weight: 40,
        category: FishingLootCategory::Junk,
    },
    FishingLootEntry {
        item_id: ITEM_LEATHER_BOOTS,
        item_name: "leather_boots",
        min_count: 1,
        max_count: 1,
        weight: 30,
        category: FishingLootCategory::Junk,
    },
    FishingLootEntry {
        item_id: ITEM_GLASS_BOTTLE,
        item_name: "glass_bottle",
        min_count: 1,
        max_count: 1,
        weight: 30,
        category: FishingLootCategory::Junk,
    },
];

const LCG_MULT: u64 = 6_364_136_223_846_793_005;
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

/// Rolls a random catch from the aquatic loot tables given a 64-bit random state seed and raining state.
///
/// Returns `(item_id, count, item_name)`.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn roll_fishing_loot(seed: u64, is_raining: bool) -> (u32, u16, &'static str) {
    // Linear congruential generator step
    let mut state = seed.wrapping_mul(LCG_MULT).wrapping_add(LCG_ADD);
    let category_roll = (state >> 32) % 100;

    // Default probabilities: Fish 85%, Treasure 5%, Junk 10%
    // In rain, treasure slightly boosted: Fish 80%, Treasure 8%, Junk 12%
    let pool: &[FishingLootEntry] = if is_raining {
        if category_roll < 80 {
            FISH_LOOT_POOL
        } else if category_roll < 88 {
            TREASURE_LOOT_POOL
        } else {
            JUNK_LOOT_POOL
        }
    } else if category_roll < 85 {
        FISH_LOOT_POOL
    } else if category_roll < 90 {
        TREASURE_LOOT_POOL
    } else {
        JUNK_LOOT_POOL
    };

    state = state.wrapping_mul(LCG_MULT).wrapping_add(LCG_ADD);
    let mut item_roll = (state >> 32) % 100;

    let mut selected = &pool[0];
    for entry in pool {
        if item_roll < u64::from(entry.weight) {
            selected = entry;
            break;
        }
        item_roll -= u64::from(entry.weight);
    }

    state = state.wrapping_mul(LCG_MULT).wrapping_add(LCG_ADD);
    let count_span = u64::from(selected.max_count - selected.min_count + 1);
    let count = selected.min_count + ((state >> 32) % count_span) as u16;

    (selected.item_id, count, selected.item_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roll_fishing_loot_bounds_and_determinism() {
        let (item1, count1, name1) = roll_fishing_loot(12345, false);
        let (item2, count2, name2) = roll_fishing_loot(12345, false);
        assert_eq!(item1, item2);
        assert_eq!(count1, count2);
        assert_eq!(name1, name2);
        assert!(count1 >= 1);

        // Check variety across multiple seeds
        let mut caught = std::collections::HashSet::new();
        for s in 0..100 {
            let (item, count, _) = roll_fishing_loot(s, false);
            assert!((1..=2).contains(&count));
            caught.insert(item);
        }
        assert!(caught.len() >= 3, "Expected diverse catch across 100 rolls");
    }
}
