use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use crate::block::BlockId;

pub const MAX_STACK_SIZE: u8 = 64;
pub const FIRST_NON_BLOCK_ITEM_ID: u16 = 300;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Pod, Zeroable)]
pub struct ItemId(pub u16);

impl ItemId {
    pub const STICK: Self = Self(FIRST_NON_BLOCK_ITEM_ID);
    pub const WOODEN_PICKAXE: Self = Self(Self::STICK.0 + 1);
    pub const WOODEN_SWORD: Self = Self(Self::STICK.0 + 2);
    pub const STONE_PICKAXE: Self = Self(Self::STICK.0 + 3);
    pub const STONE_SWORD: Self = Self(Self::STICK.0 + 4);
    pub const STONE_SHOVEL: Self = Self(Self::STICK.0 + 5);
    pub const STONE_AXE: Self = Self(Self::STICK.0 + 6);
    pub const IRON_PICKAXE: Self = Self(Self::STICK.0 + 7);
    pub const IRON_SWORD: Self = Self(Self::STICK.0 + 8);
    pub const IRON_SHOVEL: Self = Self(Self::STICK.0 + 9);
    pub const IRON_AXE: Self = Self(Self::STICK.0 + 10);
    pub const DIAMOND_PICKAXE: Self = Self(Self::STICK.0 + 11);
    pub const DIAMOND_SWORD: Self = Self(Self::STICK.0 + 12);
    pub const DIAMOND_SHOVEL: Self = Self(Self::STICK.0 + 13);
    pub const DIAMOND_AXE: Self = Self(Self::STICK.0 + 14);
    pub const WOODEN_SHOVEL: Self = Self(Self::STICK.0 + 15);
    pub const WOODEN_AXE: Self = Self(Self::STICK.0 + 16);
    pub const WOODEN_HOE: Self = Self(Self::STICK.0 + 17);
    pub const STONE_HOE: Self = Self(Self::STICK.0 + 18);
    pub const IRON_HOE: Self = Self(Self::STICK.0 + 19);
    pub const DIAMOND_HOE: Self = Self(Self::STICK.0 + 20);
    pub const IRON_INGOT: Self = Self(Self::STICK.0 + 21);
    pub const GOLD_INGOT: Self = Self(Self::STICK.0 + 22);
    pub const DIAMOND_GEM: Self = Self(Self::STICK.0 + 23);
    pub const COAL: Self = Self(Self::STICK.0 + 24);
    pub const WHEAT_ITEM: Self = Self(Self::STICK.0 + 25);
    pub const WHEAT_SEEDS: Self = Self(Self::STICK.0 + 26);
    pub const BREAD: Self = Self(Self::STICK.0 + 27);
    pub const COPPER_INGOT: Self = Self(Self::STICK.0 + 28);
    pub const IRON_HELMET: Self = Self(Self::STICK.0 + 29);
    pub const IRON_CHESTPLATE: Self = Self(Self::STICK.0 + 30);
    pub const IRON_LEGGINGS: Self = Self(Self::STICK.0 + 31);
    pub const IRON_BOOTS: Self = Self(Self::STICK.0 + 32);
    pub const DIAMOND_HELMET: Self = Self(Self::STICK.0 + 33);
    pub const DIAMOND_CHESTPLATE: Self = Self(Self::STICK.0 + 34);
    pub const DIAMOND_LEGGINGS: Self = Self(Self::STICK.0 + 35);
    pub const DIAMOND_BOOTS: Self = Self(Self::STICK.0 + 36);
    pub const BONE_MEAL: Self = Self(Self::STICK.0 + 37);
    pub const EMPTY_BUCKET: Self = Self(Self::STICK.0 + 38);
    pub const WATER_BUCKET: Self = Self(Self::STICK.0 + 39);
    pub const LAVA_BUCKET: Self = Self(Self::STICK.0 + 40);
    pub const FLINT_AND_STEEL: Self = Self(Self::STICK.0 + 41);
    pub const SHEARS_ITEM: Self = Self(Self::STICK.0 + 42);
    pub const BOW: Self = Self(Self::STICK.0 + 43);
    pub const ARROW: Self = Self(Self::STICK.0 + 44);
    pub const LEATHER: Self = Self(Self::STICK.0 + 45);
    pub const FEATHER: Self = Self(Self::STICK.0 + 46);
    pub const BONE_ITEM: Self = Self(Self::STICK.0 + 47);
    pub const STRING: Self = Self(Self::STICK.0 + 48);
    pub const RAW_BEEF: Self = Self(Self::STICK.0 + 49);
    pub const COOKED_BEEF: Self = Self(Self::STICK.0 + 50);
    pub const RAW_PORKCHOP: Self = Self(Self::STICK.0 + 51);
    pub const COOKED_PORKCHOP: Self = Self(Self::STICK.0 + 52);
    pub const RAW_CHICKEN: Self = Self(Self::STICK.0 + 53);
    pub const COOKED_CHICKEN: Self = Self(Self::STICK.0 + 54);
    pub const RAW_MUTTON: Self = Self(Self::STICK.0 + 55);
    pub const COOKED_MUTTON: Self = Self(Self::STICK.0 + 56);
    pub const LEATHER_HELMET: Self = Self(Self::STICK.0 + 57);
    pub const LEATHER_CHESTPLATE: Self = Self(Self::STICK.0 + 58);
    pub const LEATHER_LEGGINGS: Self = Self(Self::STICK.0 + 59);
    pub const LEATHER_BOOTS: Self = Self(Self::STICK.0 + 60);
    pub const GOLD_PICKAXE: Self = Self(Self::STICK.0 + 61);
    pub const GOLD_SWORD: Self = Self(Self::STICK.0 + 62);
    pub const GOLD_SHOVEL: Self = Self(Self::STICK.0 + 63);
    pub const GOLD_AXE: Self = Self(Self::STICK.0 + 64);
    pub const GOLD_HOE: Self = Self(Self::STICK.0 + 65);
    pub const GOLD_HELMET: Self = Self(Self::STICK.0 + 66);
    pub const GOLD_CHESTPLATE: Self = Self(Self::STICK.0 + 67);
    pub const GOLD_LEGGINGS: Self = Self(Self::STICK.0 + 68);
    pub const GOLD_BOOTS: Self = Self(Self::STICK.0 + 69);
    pub const PORTAL_GUN: Self = Self(Self::STICK.0 + 70);

    pub fn display_name(self) -> Option<&'static str> {
        match self {
            Self::PORTAL_GUN => Some("Portal Gun"),
            _ => None,
        }
    }

    pub fn is_block(self) -> bool {
        self.0 < FIRST_NON_BLOCK_ITEM_ID
    }

    pub fn as_block_id(self) -> Option<BlockId> {
        self.is_block().then_some(BlockId(self.0))
    }

    pub fn is_tool(self) -> bool {
        matches!(
            self,
            Self::WOODEN_PICKAXE
                | Self::WOODEN_SWORD
                | Self::STONE_PICKAXE
                | Self::STONE_SWORD
                | Self::STONE_SHOVEL
                | Self::STONE_AXE
                | Self::IRON_PICKAXE
                | Self::IRON_SWORD
                | Self::IRON_SHOVEL
                | Self::IRON_AXE
                | Self::DIAMOND_PICKAXE
                | Self::DIAMOND_SWORD
                | Self::DIAMOND_SHOVEL
                | Self::DIAMOND_AXE
                | Self::WOODEN_SHOVEL
                | Self::WOODEN_AXE
                | Self::WOODEN_HOE
                | Self::STONE_HOE
                | Self::IRON_HOE
                | Self::DIAMOND_HOE
                | Self::GOLD_PICKAXE
                | Self::GOLD_SWORD
                | Self::GOLD_SHOVEL
                | Self::GOLD_AXE
                | Self::GOLD_HOE
                | Self::FLINT_AND_STEEL
                | Self::SHEARS_ITEM
                | Self::BOW
        )
    }

    pub fn is_armor(self) -> bool {
        matches!(
            self,
            Self::IRON_HELMET
                | Self::IRON_CHESTPLATE
                | Self::IRON_LEGGINGS
                | Self::IRON_BOOTS
                | Self::DIAMOND_HELMET
                | Self::DIAMOND_CHESTPLATE
                | Self::DIAMOND_LEGGINGS
                | Self::DIAMOND_BOOTS
                | Self::LEATHER_HELMET
                | Self::LEATHER_CHESTPLATE
                | Self::LEATHER_LEGGINGS
                | Self::LEATHER_BOOTS
                | Self::GOLD_HELMET
                | Self::GOLD_CHESTPLATE
                | Self::GOLD_LEGGINGS
                | Self::GOLD_BOOTS
        )
    }
}

impl From<BlockId> for ItemId {
    fn from(value: BlockId) -> Self {
        Self(value.0)
    }
}

impl From<ItemId> for BlockId {
    fn from(value: ItemId) -> Self {
        Self(value.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ToolTier {
    Wood,
    Stone,
    Iron,
    Diamond,
    Gold,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pickaxe,
    Sword,
    Shovel,
    Axe,
    Hoe,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

pub fn tool_properties(item: ItemId) -> Option<(ToolKind, ToolTier)> {
    match item {
        ItemId::WOODEN_PICKAXE => Some((ToolKind::Pickaxe, ToolTier::Wood)),
        ItemId::WOODEN_SWORD => Some((ToolKind::Sword, ToolTier::Wood)),
        ItemId::STONE_PICKAXE => Some((ToolKind::Pickaxe, ToolTier::Stone)),
        ItemId::STONE_SWORD => Some((ToolKind::Sword, ToolTier::Stone)),
        ItemId::STONE_SHOVEL => Some((ToolKind::Shovel, ToolTier::Stone)),
        ItemId::STONE_AXE => Some((ToolKind::Axe, ToolTier::Stone)),
        ItemId::IRON_PICKAXE => Some((ToolKind::Pickaxe, ToolTier::Iron)),
        ItemId::IRON_SWORD => Some((ToolKind::Sword, ToolTier::Iron)),
        ItemId::IRON_SHOVEL => Some((ToolKind::Shovel, ToolTier::Iron)),
        ItemId::IRON_AXE => Some((ToolKind::Axe, ToolTier::Iron)),
        ItemId::DIAMOND_PICKAXE => Some((ToolKind::Pickaxe, ToolTier::Diamond)),
        ItemId::DIAMOND_SWORD => Some((ToolKind::Sword, ToolTier::Diamond)),
        ItemId::DIAMOND_SHOVEL => Some((ToolKind::Shovel, ToolTier::Diamond)),
        ItemId::DIAMOND_AXE => Some((ToolKind::Axe, ToolTier::Diamond)),
        ItemId::WOODEN_SHOVEL => Some((ToolKind::Shovel, ToolTier::Wood)),
        ItemId::WOODEN_AXE => Some((ToolKind::Axe, ToolTier::Wood)),
        ItemId::WOODEN_HOE => Some((ToolKind::Hoe, ToolTier::Wood)),
        ItemId::STONE_HOE => Some((ToolKind::Hoe, ToolTier::Stone)),
        ItemId::IRON_HOE => Some((ToolKind::Hoe, ToolTier::Iron)),
        ItemId::DIAMOND_HOE => Some((ToolKind::Hoe, ToolTier::Diamond)),
        ItemId::GOLD_PICKAXE => Some((ToolKind::Pickaxe, ToolTier::Gold)),
        ItemId::GOLD_SWORD => Some((ToolKind::Sword, ToolTier::Gold)),
        ItemId::GOLD_SHOVEL => Some((ToolKind::Shovel, ToolTier::Gold)),
        ItemId::GOLD_AXE => Some((ToolKind::Axe, ToolTier::Gold)),
        ItemId::GOLD_HOE => Some((ToolKind::Hoe, ToolTier::Gold)),
        _ => None,
    }
}

pub fn armor_slot_for_item(item: ItemId) -> Option<ArmorSlot> {
    match item {
        ItemId::IRON_HELMET | ItemId::DIAMOND_HELMET | ItemId::LEATHER_HELMET | ItemId::GOLD_HELMET => {
            Some(ArmorSlot::Helmet)
        }
        ItemId::IRON_CHESTPLATE
        | ItemId::DIAMOND_CHESTPLATE
        | ItemId::LEATHER_CHESTPLATE
        | ItemId::GOLD_CHESTPLATE => Some(ArmorSlot::Chestplate),
        ItemId::IRON_LEGGINGS
        | ItemId::DIAMOND_LEGGINGS
        | ItemId::LEATHER_LEGGINGS
        | ItemId::GOLD_LEGGINGS => Some(ArmorSlot::Leggings),
        ItemId::IRON_BOOTS | ItemId::DIAMOND_BOOTS | ItemId::LEATHER_BOOTS | ItemId::GOLD_BOOTS => {
            Some(ArmorSlot::Boots)
        }
        _ => None,
    }
}

pub fn armor_tier(item: ItemId) -> Option<ToolTier> {
    match item {
        ItemId::IRON_HELMET | ItemId::IRON_CHESTPLATE | ItemId::IRON_LEGGINGS | ItemId::IRON_BOOTS => {
            Some(ToolTier::Iron)
        }
        ItemId::DIAMOND_HELMET
        | ItemId::DIAMOND_CHESTPLATE
        | ItemId::DIAMOND_LEGGINGS
        | ItemId::DIAMOND_BOOTS => Some(ToolTier::Diamond),
        ItemId::LEATHER_HELMET
        | ItemId::LEATHER_CHESTPLATE
        | ItemId::LEATHER_LEGGINGS
        | ItemId::LEATHER_BOOTS => Some(ToolTier::Wood),
        ItemId::GOLD_HELMET | ItemId::GOLD_CHESTPLATE | ItemId::GOLD_LEGGINGS | ItemId::GOLD_BOOTS => {
            Some(ToolTier::Gold)
        }
        _ => None,
    }
}

pub fn armor_defense_points(item: ItemId) -> u8 {
    match item {
        ItemId::IRON_HELMET => 2,
        ItemId::IRON_CHESTPLATE => 6,
        ItemId::IRON_LEGGINGS => 5,
        ItemId::IRON_BOOTS => 2,
        ItemId::DIAMOND_HELMET => 3,
        ItemId::DIAMOND_CHESTPLATE => 8,
        ItemId::DIAMOND_LEGGINGS => 6,
        ItemId::DIAMOND_BOOTS => 3,
        ItemId::LEATHER_HELMET => 1,
        ItemId::LEATHER_CHESTPLATE => 3,
        ItemId::LEATHER_LEGGINGS => 2,
        ItemId::LEATHER_BOOTS => 1,
        ItemId::GOLD_HELMET => 2,
        ItemId::GOLD_CHESTPLATE => 5,
        ItemId::GOLD_LEGGINGS => 3,
        ItemId::GOLD_BOOTS => 1,
        _ => 0,
    }
}

pub fn tool_max_durability(tier: ToolTier) -> u16 {
    match tier {
        ToolTier::Wood => 59,
        ToolTier::Stone => 131,
        ToolTier::Iron => 250,
        ToolTier::Diamond => 1561,
        ToolTier::Gold => 32,
    }
}

pub fn tool_speed_multiplier(tier: ToolTier) -> f32 {
    match tier {
        ToolTier::Wood => 2.0,
        ToolTier::Stone => 4.0,
        ToolTier::Iron => 6.0,
        ToolTier::Diamond => 8.0,
        ToolTier::Gold => 12.0,
    }
}

pub fn is_food(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::BREAD
            | ItemId::RAW_BEEF
            | ItemId::COOKED_BEEF
            | ItemId::RAW_PORKCHOP
            | ItemId::COOKED_PORKCHOP
            | ItemId::RAW_CHICKEN
            | ItemId::COOKED_CHICKEN
            | ItemId::RAW_MUTTON
            | ItemId::COOKED_MUTTON
    )
}

pub fn food_hunger_value(item: ItemId) -> u8 {
    match item {
        ItemId::BREAD => 5,
        ItemId::RAW_BEEF => 3,
        ItemId::COOKED_BEEF => 8,
        ItemId::RAW_PORKCHOP => 3,
        ItemId::COOKED_PORKCHOP => 8,
        ItemId::RAW_CHICKEN => 2,
        ItemId::COOKED_CHICKEN => 6,
        ItemId::RAW_MUTTON => 2,
        ItemId::COOKED_MUTTON => 6,
        _ => 0,
    }
}

pub fn is_bucket(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::EMPTY_BUCKET | ItemId::WATER_BUCKET | ItemId::LAVA_BUCKET
    )
}

pub fn max_stack_for_item(item: ItemId) -> u8 {
    if item.is_tool() || item.is_armor() {
        return 1;
    }
    if item == ItemId::PORTAL_GUN {
        return 1;
    }
    if item == ItemId::WATER_BUCKET || item == ItemId::LAVA_BUCKET {
        return 1;
    }
    if item == ItemId::EMPTY_BUCKET {
        return 16;
    }
    MAX_STACK_SIZE
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
    #[serde(default)]
    pub durability: Option<u16>,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u8) -> Self {
        assert!(
            count <= max_stack_for_item(item),
            "item stack count cannot exceed max stack for item"
        );
        Self {
            item,
            count,
            durability: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn can_merge(&self, other: &ItemStack) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.item == other.item
            && self.durability == other.durability
            && !self.item.is_tool()
            && self.count < max_stack_for_item(self.item)
    }

    pub fn merge(&mut self, other: &mut ItemStack) {
        if !self.can_merge(other) {
            return;
        }

        let space = max_stack_for_item(self.item) - self.count;
        let moved = space.min(other.count);
        self.count += moved;
        other.count -= moved;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(with = "inventory_slots_serde")]
    pub slots: [Option<ItemStack>; Self::TOTAL_SIZE],
}

impl Inventory {
    pub const HOTBAR_SIZE: usize = 9;
    pub const MAIN_SIZE: usize = 27;
    pub const TOTAL_SIZE: usize = Self::HOTBAR_SIZE + Self::MAIN_SIZE;

    pub fn new() -> Self {
        Self {
            slots: [None; Self::TOTAL_SIZE],
        }
    }

    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    pub fn set(&mut self, slot: usize, stack: Option<ItemStack>) {
        if let Some(target) = self.slots.get_mut(slot) {
            *target = stack;
        }
    }

    pub fn hotbar_slot(&self, idx: usize) -> Option<&ItemStack> {
        if idx >= Self::HOTBAR_SIZE {
            return None;
        }
        self.get(idx)
    }

    pub fn add_item(&mut self, item: ItemId, count: u8) -> u8 {
        let stack_limit = max_stack_for_item(item);
        let mut remaining = count;

        for slot in &mut self.slots {
            if remaining == 0 {
                return 0;
            }

            if let Some(stack) = slot.as_mut() {
                if stack.item == item && stack.durability.is_none() && stack.count < stack_limit {
                    let space = stack_limit - stack.count;
                    let moved = space.min(remaining);
                    stack.count += moved;
                    remaining -= moved;
                }
            }
        }

        for slot in &mut self.slots {
            if remaining == 0 {
                return 0;
            }

            if slot.is_none() {
                let moved = remaining.min(stack_limit);
                *slot = Some(ItemStack::new(item, moved));
                remaining -= moved;
            }
        }

        remaining
    }

    pub fn remove_item(&mut self, slot: usize, count: u8) -> Option<ItemStack> {
        if count == 0 {
            return None;
        }

        let slot_ref = self.slots.get_mut(slot)?;
        let stack = slot_ref.as_mut()?;
        let removed_count = stack.count.min(count);
        if removed_count == 0 {
            return None;
        }

        let removed = ItemStack {
            item: stack.item,
            count: removed_count,
            durability: if removed_count == stack.count {
                stack.durability
            } else {
                None
            },
        };
        stack.count -= removed_count;
        if stack.count == 0 {
            *slot_ref = None;
        }

        Some(removed)
    }

    pub fn contains(&self, item: ItemId) -> bool {
        self.slots
            .iter()
            .flatten()
            .any(|stack| stack.item == item && stack.count > 0)
    }

    pub fn count_item(&self, item: ItemId) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|stack| stack.item == item)
            .map(|stack| u32::from(stack.count))
            .sum()
    }

    pub fn swap(&mut self, slot_a: usize, slot_b: usize) {
        if slot_a < Self::TOTAL_SIZE && slot_b < Self::TOTAL_SIZE {
            self.slots.swap(slot_a, slot_b);
        }
    }
}

mod inventory_slots_serde {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Inventory, ItemStack};

    pub fn serialize<S>(
        slots: &[Option<ItemStack>; Inventory::TOTAL_SIZE],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        slots.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<[Option<ItemStack>; Inventory::TOTAL_SIZE], D::Error>
    where
        D: Deserializer<'de>,
    {
        let slots = Vec::<Option<ItemStack>>::deserialize(deserializer)?;
        slots.try_into().map_err(|slots: Vec<Option<ItemStack>>| {
            D::Error::invalid_length(
                slots.len(),
                &"a sequence with exactly Inventory::TOTAL_SIZE elements",
            )
        })
    }
}
