use crate::block::BlockId;
use crate::inventory::{ItemId, ItemStack};

const TIMBER_LOG: ItemId = ItemId(6);
const HEWN_PLANK: ItemId = ItemId(7);
const RUBBLESTONE: ItemId = ItemId(10);
const CRAFTING_TABLE: ItemId = ItemId(BlockId::CRAFTING_TABLE.0);
const FURNACE: ItemId = ItemId(BlockId::FURNACE.0);
const IRON_INGOT: ItemId = ItemId::IRON_INGOT;
const GOLD_INGOT: ItemId = ItemId::GOLD_INGOT;
const DIAMOND_GEM: ItemId = ItemId::DIAMOND_GEM;
const DUNE_SAND: ItemId = ItemId(5);
const SNOWCAP: ItemId = ItemId(15);
const SAND: ItemId = ItemId(29);
const TNT_BLOCK: ItemId = ItemId(79);
const TRAPDOOR: ItemId = ItemId(80);
const BED_ITEM: ItemId = ItemId(88);
const STONE_SLAB: ItemId = ItemId(107);
const WOODEN_SLAB: ItemId = ItemId(109);
const PRESSURE_PLATE: ItemId = ItemId(106);
const TORCH: ItemId = ItemId(34);
const PUMPKIN: ItemId = ItemId(166);
const JACK_O_LANTERN: ItemId = ItemId(167);
const HAY_BALE: ItemId = ItemId(169);
const BOOKSHELF: ItemId = ItemId(170);
const WHITE_WOOL: ItemId = ItemId(128);
const WHITE_CARPET: ItemId = ItemId(144);
const LEATHER: ItemId = ItemId::LEATHER;
const FEATHER: ItemId = ItemId::FEATHER;
const BONE_ITEM: ItemId = ItemId::BONE_ITEM;
const STRING_ITEM: ItemId = ItemId::STRING;
const BUCKET: ItemId = ItemId::EMPTY_BUCKET;
const FLINT_AND_STEEL: ItemId = ItemId::FLINT_AND_STEEL;
const SHEARS: ItemId = ItemId::SHEARS_ITEM;
const BOW: ItemId = ItemId::BOW;
const ARROW: ItemId = ItemId::ARROW;
const RAW_BEEF: ItemId = ItemId::RAW_BEEF;
const COOKED_BEEF: ItemId = ItemId::COOKED_BEEF;
const RAW_PORKCHOP: ItemId = ItemId::RAW_PORKCHOP;
const COOKED_PORKCHOP: ItemId = ItemId::COOKED_PORKCHOP;
const RAW_CHICKEN: ItemId = ItemId::RAW_CHICKEN;
const COOKED_CHICKEN: ItemId = ItemId::COOKED_CHICKEN;
const RAW_MUTTON: ItemId = ItemId::RAW_MUTTON;
const COOKED_MUTTON: ItemId = ItemId::COOKED_MUTTON;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RecipeMatchMode {
    Shapeless,
    NormalizedShaped,
    ExactShaped,
}

#[derive(Copy, Clone, Debug)]
struct RecipeDef {
    width: usize,
    height: usize,
    pattern: &'static [Option<ItemId>],
    output: ItemStack,
    mode: RecipeMatchMode,
    requires_table: bool,
}

const RECIPES: &[RecipeDef] = &[
    // 1 timber_log -> 4 hewn_plank (shapeless in 2x2 or 3x3)
    RecipeDef {
        width: 1,
        height: 1,
        pattern: &[Some(TIMBER_LOG)],
        output: ItemStack {
            item: HEWN_PLANK,
            count: 4,
            durability: None,
        },
        mode: RecipeMatchMode::Shapeless,
        requires_table: false,
    },
    // 4 hewn_plank (2x2) -> 1 crafting_table
    RecipeDef {
        width: 2,
        height: 2,
        pattern: &[
            Some(HEWN_PLANK),
            Some(HEWN_PLANK),
            Some(HEWN_PLANK),
            Some(HEWN_PLANK),
        ],
        output: ItemStack {
            item: CRAFTING_TABLE,
            count: 1,
            durability: None,
        },
        mode: RecipeMatchMode::NormalizedShaped,
        requires_table: false,
    },
    // 2 hewn_plank (vertical) -> 4 sticks
    RecipeDef {
        width: 1,
        height: 2,
        pattern: &[Some(HEWN_PLANK), Some(HEWN_PLANK)],
        output: ItemStack {
            item: ItemId::STICK,
            count: 4,
            durability: None,
        },
        mode: RecipeMatchMode::NormalizedShaped,
        requires_table: false,
    },
    // Wooden pickaxe (3x3 only)
    RecipeDef {
        width: 3,
        height: 3,
        pattern: &[
            Some(HEWN_PLANK),
            Some(HEWN_PLANK),
            Some(HEWN_PLANK),
            None,
            Some(ItemId::STICK),
            None,
            None,
            Some(ItemId::STICK),
            None,
        ],
        output: ItemStack {
            item: ItemId::WOODEN_PICKAXE,
            count: 1,
            durability: None,
        },
        mode: RecipeMatchMode::ExactShaped,
        requires_table: true,
    },
    // Wooden sword (3x3 only)
    RecipeDef {
        width: 3,
        height: 3,
        pattern: &[
            None,
            Some(HEWN_PLANK),
            None,
            None,
            Some(HEWN_PLANK),
            None,
            None,
            Some(ItemId::STICK),
            None,
        ],
        output: ItemStack {
            item: ItemId::WOODEN_SWORD,
            count: 1,
            durability: None,
        },
        mode: RecipeMatchMode::ExactShaped,
        requires_table: true,
    },
    // Furnace ring (3x3 only)
    RecipeDef {
        width: 3,
        height: 3,
        pattern: &[
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
            None,
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
            Some(RUBBLESTONE),
        ],
        output: ItemStack {
            item: FURNACE,
            count: 1,
            durability: None,
        },
        mode: RecipeMatchMode::ExactShaped,
        requires_table: true,
    },
    // Wooden shovel
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(HEWN_PLANK), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::WOODEN_SHOVEL, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Wooden axe
    RecipeDef { width: 3, height: 3, pattern: &[Some(HEWN_PLANK), Some(HEWN_PLANK), None, Some(HEWN_PLANK), Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::WOODEN_AXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Wooden hoe
    RecipeDef { width: 3, height: 3, pattern: &[Some(HEWN_PLANK), Some(HEWN_PLANK), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::WOODEN_HOE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone pickaxe
    RecipeDef { width: 3, height: 3, pattern: &[Some(RUBBLESTONE), Some(RUBBLESTONE), Some(RUBBLESTONE), None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::STONE_PICKAXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone sword
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(RUBBLESTONE), None, None, Some(RUBBLESTONE), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::STONE_SWORD, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone shovel
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(RUBBLESTONE), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::STONE_SHOVEL, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone axe
    RecipeDef { width: 3, height: 3, pattern: &[Some(RUBBLESTONE), Some(RUBBLESTONE), None, Some(RUBBLESTONE), Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::STONE_AXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone hoe
    RecipeDef { width: 3, height: 3, pattern: &[Some(RUBBLESTONE), Some(RUBBLESTONE), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::STONE_HOE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron pickaxe
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::IRON_PICKAXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron sword
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(IRON_INGOT), None, None, Some(IRON_INGOT), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::IRON_SWORD, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron shovel
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(IRON_INGOT), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::IRON_SHOVEL, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron axe
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), Some(IRON_INGOT), None, Some(IRON_INGOT), Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::IRON_AXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron hoe
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), Some(IRON_INGOT), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::IRON_HOE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron helmet
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), None, Some(IRON_INGOT), None, None, None], output: ItemStack { item: ItemId::IRON_HELMET, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron chestplate
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), None, Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT)], output: ItemStack { item: ItemId::IRON_CHESTPLATE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron leggings
    RecipeDef { width: 3, height: 3, pattern: &[Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), Some(IRON_INGOT), None, Some(IRON_INGOT), Some(IRON_INGOT), None, Some(IRON_INGOT)], output: ItemStack { item: ItemId::IRON_LEGGINGS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Iron boots
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, Some(IRON_INGOT), None, Some(IRON_INGOT), Some(IRON_INGOT), None, Some(IRON_INGOT)], output: ItemStack { item: ItemId::IRON_BOOTS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond pickaxe
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::DIAMOND_PICKAXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond sword
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(DIAMOND_GEM), None, None, Some(DIAMOND_GEM), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::DIAMOND_SWORD, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond shovel
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(DIAMOND_GEM), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::DIAMOND_SHOVEL, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond axe
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(DIAMOND_GEM), Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::DIAMOND_AXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond hoe
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::DIAMOND_HOE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond helmet
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(DIAMOND_GEM), None, None, None], output: ItemStack { item: ItemId::DIAMOND_HELMET, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond chestplate
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), None, Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM)], output: ItemStack { item: ItemId::DIAMOND_CHESTPLATE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond leggings
    RecipeDef { width: 3, height: 3, pattern: &[Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(DIAMOND_GEM)], output: ItemStack { item: ItemId::DIAMOND_LEGGINGS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Diamond boots
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, Some(DIAMOND_GEM), None, Some(DIAMOND_GEM), Some(DIAMOND_GEM), None, Some(DIAMOND_GEM)], output: ItemStack { item: ItemId::DIAMOND_BOOTS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Leather helmet
    RecipeDef { width: 3, height: 3, pattern: &[Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER), None, Some(LEATHER), None, None, None], output: ItemStack { item: ItemId::LEATHER_HELMET, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Leather chestplate
    RecipeDef { width: 3, height: 3, pattern: &[Some(LEATHER), None, Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER)], output: ItemStack { item: ItemId::LEATHER_CHESTPLATE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Leather leggings
    RecipeDef { width: 3, height: 3, pattern: &[Some(LEATHER), Some(LEATHER), Some(LEATHER), Some(LEATHER), None, Some(LEATHER), Some(LEATHER), None, Some(LEATHER)], output: ItemStack { item: ItemId::LEATHER_LEGGINGS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Leather boots
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, Some(LEATHER), None, Some(LEATHER), Some(LEATHER), None, Some(LEATHER)], output: ItemStack { item: ItemId::LEATHER_BOOTS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold pickaxe
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::GOLD_PICKAXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold sword
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(GOLD_INGOT), None, None, Some(GOLD_INGOT), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::GOLD_SWORD, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold shovel
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(GOLD_INGOT), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::GOLD_SHOVEL, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold axe
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(GOLD_INGOT), Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::GOLD_AXE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold hoe
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), Some(GOLD_INGOT), None, None, Some(ItemId::STICK), None, None, Some(ItemId::STICK), None], output: ItemStack { item: ItemId::GOLD_HOE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold helmet
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(GOLD_INGOT), None, None, None], output: ItemStack { item: ItemId::GOLD_HELMET, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold chestplate
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), None, Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT)], output: ItemStack { item: ItemId::GOLD_CHESTPLATE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold leggings
    RecipeDef { width: 3, height: 3, pattern: &[Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(GOLD_INGOT)], output: ItemStack { item: ItemId::GOLD_LEGGINGS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Gold boots
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, Some(GOLD_INGOT), None, Some(GOLD_INGOT), Some(GOLD_INGOT), None, Some(GOLD_INGOT)], output: ItemStack { item: ItemId::GOLD_BOOTS, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Bucket
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, Some(IRON_INGOT), None, Some(IRON_INGOT), None, Some(IRON_INGOT), None], output: ItemStack { item: BUCKET, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Shears
    RecipeDef { width: 2, height: 2, pattern: &[None, Some(IRON_INGOT), Some(IRON_INGOT), None], output: ItemStack { item: SHEARS, count: 1 , durability: None }, mode: RecipeMatchMode::NormalizedShaped, requires_table: false },
    // Flint and steel (rubblestone as flint substitute)
    RecipeDef { width: 2, height: 2, pattern: &[Some(IRON_INGOT), None, None, Some(RUBBLESTONE)], output: ItemStack { item: FLINT_AND_STEEL, count: 1 , durability: None }, mode: RecipeMatchMode::NormalizedShaped, requires_table: false },
    // Bow
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(ItemId::STICK), Some(STRING_ITEM), Some(ItemId::STICK), None, Some(STRING_ITEM), None, Some(ItemId::STICK), Some(STRING_ITEM)], output: ItemStack { item: BOW, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Arrow (rubblestone as flint substitute)
    RecipeDef { width: 3, height: 3, pattern: &[None, Some(RUBBLESTONE), None, None, Some(ItemId::STICK), None, None, Some(FEATHER), None], output: ItemStack { item: ARROW, count: 4 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Bookshelf: 6 planks (3x2)
    RecipeDef { width: 3, height: 3, pattern: &[Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), None, None, None], output: ItemStack { item: BOOKSHELF, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Hay bale
    RecipeDef { width: 3, height: 3, pattern: &[Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM)], output: ItemStack { item: HAY_BALE, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // White carpet: 2 white wool -> 3 carpet
    RecipeDef { width: 2, height: 1, pattern: &[Some(WHITE_WOOL), Some(WHITE_WOOL)], output: ItemStack { item: WHITE_CARPET, count: 3 , durability: None }, mode: RecipeMatchMode::NormalizedShaped, requires_table: false },
    // Bone -> bone meal
    RecipeDef { width: 1, height: 1, pattern: &[Some(BONE_ITEM)], output: ItemStack { item: ItemId::BONE_MEAL, count: 3 , durability: None }, mode: RecipeMatchMode::Shapeless, requires_table: false },
    // Pumpkin + torch -> jack o'lantern
    RecipeDef { width: 1, height: 2, pattern: &[Some(PUMPKIN), Some(TORCH)], output: ItemStack { item: JACK_O_LANTERN, count: 1 , durability: None }, mode: RecipeMatchMode::NormalizedShaped, requires_table: false },
    // Glass pane
    RecipeDef { width: 3, height: 3, pattern: &[Some(ItemId(12)), Some(ItemId(12)), Some(ItemId(12)), Some(ItemId(12)), Some(ItemId(12)), Some(ItemId(12)), None, None, None], output: ItemStack { item: ItemId(111), count: 16 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone stairs
    RecipeDef { width: 3, height: 3, pattern: &[Some(RUBBLESTONE), None, None, Some(RUBBLESTONE), Some(RUBBLESTONE), None, Some(RUBBLESTONE), Some(RUBBLESTONE), Some(RUBBLESTONE)], output: ItemStack { item: ItemId(116), count: 4 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Wooden stairs
    RecipeDef { width: 3, height: 3, pattern: &[Some(HEWN_PLANK), None, None, Some(HEWN_PLANK), Some(HEWN_PLANK), None, Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK)], output: ItemStack { item: ItemId(120), count: 4 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Bed: 3 snowcap top + 3 plank bottom
    RecipeDef { width: 3, height: 3, pattern: &[Some(SNOWCAP), Some(SNOWCAP), Some(SNOWCAP), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), None, None, None], output: ItemStack { item: BED_ITEM, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // TNT: checkerboard sand/dune_sand
    RecipeDef { width: 3, height: 3, pattern: &[Some(DUNE_SAND), Some(SAND), Some(DUNE_SAND), Some(SAND), Some(DUNE_SAND), Some(SAND), Some(DUNE_SAND), Some(SAND), Some(DUNE_SAND)], output: ItemStack { item: TNT_BLOCK, count: 1 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone slab: 3 rubblestone bottom row = 6 slabs
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, None, None, None, Some(RUBBLESTONE), Some(RUBBLESTONE), Some(RUBBLESTONE)], output: ItemStack { item: STONE_SLAB, count: 6 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Wooden slab: 3 planks bottom row = 6 slabs
    RecipeDef { width: 3, height: 3, pattern: &[None, None, None, None, None, None, Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK)], output: ItemStack { item: WOODEN_SLAB, count: 6 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Bread: 3 wheat (shapeless, can be in 2x2 or 3x3)
    RecipeDef { width: 1, height: 3, pattern: &[Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM), Some(ItemId::WHEAT_ITEM)], output: ItemStack { item: ItemId::BREAD, count: 1 , durability: None }, mode: RecipeMatchMode::Shapeless, requires_table: false },
    // Trapdoor: 6 planks 3x2 = 2 trapdoors
    RecipeDef { width: 3, height: 3, pattern: &[Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), Some(HEWN_PLANK), None, None, None], output: ItemStack { item: TRAPDOOR, count: 2 , durability: None }, mode: RecipeMatchMode::ExactShaped, requires_table: true },
    // Stone pressure plate: 2 rubblestone in a row
    RecipeDef { width: 2, height: 1, pattern: &[Some(RUBBLESTONE), Some(RUBBLESTONE)], output: ItemStack { item: PRESSURE_PLATE, count: 1 , durability: None }, mode: RecipeMatchMode::NormalizedShaped, requires_table: false },
];

pub fn match_inventory_crafting(inputs: &[Option<ItemStack>; 4]) -> Option<ItemStack> {
    match_recipe(inputs, 2, 2, false)
}

pub fn match_crafting_table(inputs: &[Option<ItemStack>; 9]) -> Option<ItemStack> {
    match_recipe(inputs, 3, 3, true)
}

fn match_recipe(
    inputs: &[Option<ItemStack>],
    grid_width: usize,
    grid_height: usize,
    has_crafting_table: bool,
) -> Option<ItemStack> {
    if grid_width == 0 || grid_height == 0 || inputs.len() != grid_width * grid_height {
        return None;
    }

    let item_grid: Vec<Option<ItemId>> = inputs
        .iter()
        .map(|slot| {
            slot.and_then(|stack| {
                if stack.count > 0 {
                    Some(stack.item)
                } else {
                    None
                }
            })
        })
        .collect();

    for recipe in RECIPES {
        if recipe.requires_table && !has_crafting_table {
            continue;
        }
        if recipe.width > grid_width || recipe.height > grid_height {
            continue;
        }

        let matched = match recipe.mode {
            RecipeMatchMode::Shapeless => matches_shapeless(recipe, &item_grid),
            RecipeMatchMode::NormalizedShaped => {
                matches_normalized_shaped(recipe, &item_grid, grid_width, grid_height)
            }
            RecipeMatchMode::ExactShaped => matches_exact_shaped(recipe, &item_grid, grid_width),
        };

        if matched {
            return Some(recipe.output);
        }
    }

    None
}

fn matches_shapeless(recipe: &RecipeDef, items: &[Option<ItemId>]) -> bool {
    let mut expected: Vec<ItemId> = recipe.pattern.iter().flatten().copied().collect();
    let mut actual: Vec<ItemId> = items.iter().flatten().copied().collect();
    expected.sort_by_key(|item| item.0);
    actual.sort_by_key(|item| item.0);
    expected == actual
}

fn matches_normalized_shaped(
    recipe: &RecipeDef,
    items: &[Option<ItemId>],
    grid_width: usize,
    grid_height: usize,
) -> bool {
    let Some((min_x, min_y, width, height)) = occupied_bounds(items, grid_width, grid_height) else {
        return false;
    };

    if width != recipe.width || height != recipe.height {
        return false;
    }

    for y in 0..height {
        for x in 0..width {
            let item = items[(min_y + y) * grid_width + (min_x + x)];
            let expected = recipe.pattern[y * recipe.width + x];
            if item != expected {
                return false;
            }
        }
    }

    true
}

fn matches_exact_shaped(recipe: &RecipeDef, items: &[Option<ItemId>], grid_width: usize) -> bool {
    for y in 0..recipe.height {
        for x in 0..recipe.width {
            let item = items[y * grid_width + x];
            let expected = recipe.pattern[y * recipe.width + x];
            if item != expected {
                return false;
            }
        }
    }
    true
}

fn occupied_bounds(
    items: &[Option<ItemId>],
    grid_width: usize,
    grid_height: usize,
) -> Option<(usize, usize, usize, usize)> {
    let mut min_x = grid_width;
    let mut min_y = grid_height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..grid_height {
        for x in 0..grid_width {
            if items[y * grid_width + x].is_some() {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if !found {
        return None;
    }

    Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
}

#[cfg(test)]
mod tests {
    use super::{match_crafting_table, match_inventory_crafting};
    use crate::inventory::{ItemId, ItemStack};

    fn stack(item: ItemId) -> Option<ItemStack> {
        Some(ItemStack::new(item, 1))
    }

    #[test]
    fn inventory_shapeless_log_to_planks() {
        let mut grid = [None; 4];
        grid[3] = stack(ItemId(6));
        let result = match_inventory_crafting(&grid);
        assert_eq!(result.map(|s| (s.item.0, s.count)), Some((7, 4)));
    }

    #[test]
    fn inventory_planks_to_sticks_in_any_column() {
        let mut grid = [None; 4];
        grid[1] = stack(ItemId(7));
        grid[3] = stack(ItemId(7));
        let result = match_inventory_crafting(&grid);
        assert_eq!(result.map(|s| (s.item, s.count)), Some((ItemId::STICK, 4)));
    }

    #[test]
    fn crafting_table_pickaxe_recipe_matches() {
        let mut grid = [None; 9];
        grid[0] = stack(ItemId(7));
        grid[1] = stack(ItemId(7));
        grid[2] = stack(ItemId(7));
        grid[4] = stack(ItemId::STICK);
        grid[7] = stack(ItemId::STICK);
        let result = match_crafting_table(&grid);
        assert_eq!(result.map(|s| s.item), Some(ItemId::WOODEN_PICKAXE));
    }

    #[test]
    fn crafting_table_sword_requires_center_column() {
        let mut centered = [None; 9];
        centered[1] = stack(ItemId(7));
        centered[4] = stack(ItemId(7));
        centered[7] = stack(ItemId::STICK);
        assert_eq!(
            match_crafting_table(&centered).map(|s| s.item),
            Some(ItemId::WOODEN_SWORD)
        );

        let mut shifted = [None; 9];
        shifted[0] = stack(ItemId(7));
        shifted[3] = stack(ItemId(7));
        shifted[6] = stack(ItemId::STICK);
        assert!(match_crafting_table(&shifted).is_none());
    }
}

// --- Smelting ---

#[derive(Copy, Clone, Debug)]
pub struct SmeltingRecipe {
    pub input: ItemId,
    pub output: ItemStack,
    pub smelt_time_secs: f32,
}

pub const SMELTING_RECIPES: &[SmeltingRecipe] = &[
    SmeltingRecipe { input: ItemId(11), output: ItemStack { item: ItemId::IRON_INGOT, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: ItemId(18), output: ItemStack { item: ItemId::GOLD_INGOT, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: ItemId(17), output: ItemStack { item: ItemId::COPPER_INGOT, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: ItemId(5),  output: ItemStack { item: ItemId(12), count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: ItemId(16), output: ItemStack { item: ItemId::COAL, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: RAW_BEEF, output: ItemStack { item: COOKED_BEEF, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: RAW_PORKCHOP, output: ItemStack { item: COOKED_PORKCHOP, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: RAW_CHICKEN, output: ItemStack { item: COOKED_CHICKEN, count: 1 , durability: None }, smelt_time_secs: 10.0 },
    SmeltingRecipe { input: RAW_MUTTON, output: ItemStack { item: COOKED_MUTTON, count: 1 , durability: None }, smelt_time_secs: 10.0 },
];

pub fn fuel_burn_time_secs(item: ItemId) -> Option<f32> {
    match item.0 {
        6 => Some(15.0),
        7 => Some(15.0),
        x if x == ItemId::COAL.0 => Some(80.0),
        x if x == ItemId::STICK.0 => Some(5.0),
        128..=143 => Some(5.0),
        _ => None,
    }
}

pub fn find_smelting_recipe(input: ItemId) -> Option<&'static SmeltingRecipe> {
    SMELTING_RECIPES.iter().find(|r| r.input == input)
}
