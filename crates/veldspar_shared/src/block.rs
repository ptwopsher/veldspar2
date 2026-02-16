use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

#[repr(transparent)]
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Pod,
    Zeroable,
)]
pub struct BlockId(pub u16);

impl BlockId {
    pub const AIR: Self = Self(0);
    pub const LOAM: Self = Self(3);
    pub const VERDANT_TURF: Self = Self(4);
    pub const TIMBER_LOG: Self = Self(6);
    pub const CANOPY_LEAVES: Self = Self(8);
    pub const TALL_GRASS: Self = Self(20);
    pub const WILDFLOWER: Self = Self(21);
    pub const RUBBLESTONE: Self = Self(10);
    pub const STILL_WATER: Self = Self(9);
    pub const SAND: Self = Self(29);
    pub const TORCH: Self = Self(34);
    pub const FLOWING_WATER_LEVEL0: Self = Self(35);
    pub const FLOWING_WATER_LEVEL1: Self = Self(36);
    pub const FLOWING_WATER_LEVEL2: Self = Self(37);
    pub const FLOWING_WATER_LEVEL3: Self = Self(38);
    pub const FLOWING_WATER_LEVEL4: Self = Self(39);
    pub const FLOWING_WATER_LEVEL5: Self = Self(40);
    pub const FLOWING_WATER_LEVEL6: Self = Self(41);
    pub const FLOWING_WATER_LEVEL7: Self = Self(42);
    pub const WOODEN_DOOR: Self = Self(43);
    pub const DOOR_LOWER: Self = Self(44);
    pub const DOOR_UPPER: Self = Self(45);
    pub const DOOR_LOWER_EAST: Self = Self(46);
    pub const DOOR_UPPER_EAST: Self = Self(47);
    pub const DOOR_LOWER_SOUTH: Self = Self(48);
    pub const DOOR_UPPER_SOUTH: Self = Self(49);
    pub const DOOR_LOWER_WEST: Self = Self(50);
    pub const DOOR_UPPER_WEST: Self = Self(51);
    pub const DOOR_LOWER_OPEN: Self = Self(52);
    pub const DOOR_UPPER_OPEN: Self = Self(53);
    pub const DOOR_LOWER_OPEN_EAST: Self = Self(54);
    pub const DOOR_UPPER_OPEN_EAST: Self = Self(55);
    pub const DOOR_LOWER_OPEN_SOUTH: Self = Self(56);
    pub const DOOR_UPPER_OPEN_SOUTH: Self = Self(57);
    pub const DOOR_LOWER_OPEN_WEST: Self = Self(58);
    pub const DOOR_UPPER_OPEN_WEST: Self = Self(59);
    pub const LADDER: Self = Self(60);
    pub const LADDER_EAST: Self = Self(61);
    pub const LADDER_SOUTH: Self = Self(62);
    pub const LADDER_WEST: Self = Self(63);
    pub const FENCE: Self = Self(64);
    pub const CRAFTING_TABLE: Self = Self(65);
    pub const FURNACE: Self = Self(66);
    pub const CHEST: Self = Self(67);
    pub const SAPLING: Self = Self(68);
    pub const SUGAR_CANE: Self = Self(69);
    pub const LAVA_SOURCE: Self = Self(70);
    pub const FLOWING_LAVA_LEVEL1: Self = Self(71);
    pub const FLOWING_LAVA_LEVEL2: Self = Self(72);
    pub const FLOWING_LAVA_LEVEL3: Self = Self(73);
    pub const FLOWING_LAVA_LEVEL4: Self = Self(74);
    pub const FLOWING_LAVA_LEVEL5: Self = Self(75);
    pub const FLOWING_LAVA_LEVEL6: Self = Self(76);
    pub const FLOWING_LAVA_LEVEL7: Self = Self(77);
    pub const OBSIDIAN: Self = Self(78);
    pub const TNT: Self = Self(79);
    pub const TRAPDOOR_CLOSED: Self = Self(80);
    pub const TRAPDOOR_OPEN: Self = Self(81);
    pub const TRAPDOOR_CLOSED_EAST: Self = Self(82);
    pub const TRAPDOOR_OPEN_EAST: Self = Self(83);
    pub const TRAPDOOR_CLOSED_SOUTH: Self = Self(84);
    pub const TRAPDOOR_OPEN_SOUTH: Self = Self(85);
    pub const TRAPDOOR_CLOSED_WEST: Self = Self(86);
    pub const TRAPDOOR_OPEN_WEST: Self = Self(87);
    pub const BED_FOOT: Self = Self(88);
    pub const BED_HEAD: Self = Self(89);
    pub const BED_FOOT_EAST: Self = Self(90);
    pub const BED_HEAD_EAST: Self = Self(91);
    pub const BED_FOOT_SOUTH: Self = Self(92);
    pub const BED_HEAD_SOUTH: Self = Self(93);
    pub const BED_FOOT_WEST: Self = Self(94);
    pub const BED_HEAD_WEST: Self = Self(95);
    pub const FARMLAND: Self = Self(96);
    pub const WHEAT_STAGE_0: Self = Self(97);
    pub const WHEAT_STAGE_1: Self = Self(98);
    pub const WHEAT_STAGE_2: Self = Self(99);
    pub const WHEAT_STAGE_3: Self = Self(100);
    pub const WHEAT_STAGE_4: Self = Self(101);
    pub const WHEAT_STAGE_5: Self = Self(102);
    pub const WHEAT_STAGE_6: Self = Self(103);
    pub const WHEAT_STAGE_7: Self = Self(104);
    pub const FIRE: Self = Self(105);
    pub const STONE_PRESSURE_PLATE: Self = Self(106);
    pub const STONE_SLAB_BOTTOM: Self = Self(107);
    pub const STONE_SLAB_TOP: Self = Self(108);
    pub const WOODEN_SLAB_BOTTOM: Self = Self(109);
    pub const WOODEN_SLAB_TOP: Self = Self(110);
    pub const GLASS_PANE: Self = Self(111);
    pub const LEVER_OFF: Self = Self(112);
    pub const LEVER_ON: Self = Self(113);
    pub const STONE_BUTTON_OFF: Self = Self(114);
    pub const STONE_BUTTON_ON: Self = Self(115);
    pub const STONE_STAIRS_NORTH: Self = Self(116);
    pub const STONE_STAIRS_EAST: Self = Self(117);
    pub const STONE_STAIRS_SOUTH: Self = Self(118);
    pub const STONE_STAIRS_WEST: Self = Self(119);
    pub const WOODEN_STAIRS_NORTH: Self = Self(120);
    pub const WOODEN_STAIRS_EAST: Self = Self(121);
    pub const WOODEN_STAIRS_SOUTH: Self = Self(122);
    pub const WOODEN_STAIRS_WEST: Self = Self(123);
    pub const SIGN_NORTH: Self = Self(124);
    pub const SIGN_SOUTH: Self = Self(125);
    pub const SIGN_EAST: Self = Self(126);
    pub const SIGN_WEST: Self = Self(127);
    pub const WOOL_WHITE: Self = Self(128);
    pub const WOOL_ORANGE: Self = Self(129);
    pub const WOOL_MAGENTA: Self = Self(130);
    pub const WOOL_LIGHT_BLUE: Self = Self(131);
    pub const WOOL_YELLOW: Self = Self(132);
    pub const WOOL_LIME: Self = Self(133);
    pub const WOOL_PINK: Self = Self(134);
    pub const WOOL_GRAY: Self = Self(135);
    pub const WOOL_LIGHT_GRAY: Self = Self(136);
    pub const WOOL_CYAN: Self = Self(137);
    pub const WOOL_PURPLE: Self = Self(138);
    pub const WOOL_BLUE: Self = Self(139);
    pub const WOOL_BROWN: Self = Self(140);
    pub const WOOL_GREEN: Self = Self(141);
    pub const WOOL_RED: Self = Self(142);
    pub const WOOL_BLACK: Self = Self(143);
    pub const CARPET_WHITE: Self = Self(144);
    pub const CARPET_ORANGE: Self = Self(145);
    pub const CARPET_MAGENTA: Self = Self(146);
    pub const CARPET_LIGHT_BLUE: Self = Self(147);
    pub const CARPET_YELLOW: Self = Self(148);
    pub const CARPET_LIME: Self = Self(149);
    pub const CARPET_PINK: Self = Self(150);
    pub const CARPET_GRAY: Self = Self(151);
    pub const CARPET_LIGHT_GRAY: Self = Self(152);
    pub const CARPET_CYAN: Self = Self(153);
    pub const CARPET_PURPLE: Self = Self(154);
    pub const CARPET_BLUE: Self = Self(155);
    pub const CARPET_BROWN: Self = Self(156);
    pub const CARPET_GREEN: Self = Self(157);
    pub const CARPET_RED: Self = Self(158);
    pub const CARPET_BLACK: Self = Self(159);
    pub const COBWEB: Self = Self(160);
    pub const VINE_NORTH: Self = Self(161);
    pub const VINE_EAST: Self = Self(162);
    pub const VINE_SOUTH: Self = Self(163);
    pub const VINE_WEST: Self = Self(164);
    pub const CACTUS: Self = Self(165);
    pub const PUMPKIN: Self = Self(166);
    pub const JACK_O_LANTERN: Self = Self(167);
    pub const MELON: Self = Self(168);
    pub const HAY_BALE: Self = Self(169);
    pub const BOOKSHELF: Self = Self(170);
    pub const RED_MUSHROOM: Self = Self(171);
    pub const BROWN_MUSHROOM: Self = Self(172);
    pub const SOUL_SAND: Self = Self(173);
    pub const HONEY_BLOCK: Self = Self(174);
    pub const ANDESITE: Self = Self(175);
    pub const DIORITE: Self = Self(176);
    pub const POLISHED_ANDESITE: Self = Self(177);
    pub const POLISHED_DIORITE: Self = Self(178);
    pub const POLISHED_GRANITE: Self = Self(179);
    pub const DEEPSLATE: Self = Self(180);
    pub const DEEPSLATE_BRICKS: Self = Self(181);
    pub const DEEPSLATE_TILES: Self = Self(182);
    pub const POLISHED_DEEPSLATE: Self = Self(183);
    pub const CALCITE: Self = Self(184);
    pub const BLACKSTONE: Self = Self(185);
    pub const POLISHED_BLACKSTONE_BRICKS: Self = Self(186);
    pub const TUFF_BRICKS: Self = Self(187);
    pub const PRISMARINE: Self = Self(188);
    pub const PRISMARINE_BRICKS: Self = Self(189);
    pub const END_STONE: Self = Self(190);
    pub const END_STONE_BRICKS: Self = Self(191);
    pub const QUARTZ_BRICKS: Self = Self(192);
    pub const QUARTZ_BLOCK_SIDE: Self = Self(193);
    pub const SMOOTH_STONE: Self = Self(194);
    pub const SANDSTONE_TOP: Self = Self(195);
    pub const RED_SANDSTONE: Self = Self(196);
    pub const RED_SANDSTONE_TOP: Self = Self(197);
    pub const MUD: Self = Self(198);
    pub const MUD_BRICKS: Self = Self(199);
    pub const PACKED_MUD: Self = Self(200);
    pub const MOSS_BLOCK: Self = Self(201);
    pub const MOSSY_STONE_BRICKS: Self = Self(202);
    pub const CHISELED_STONE_BRICKS: Self = Self(203);
    pub const CHISELED_DEEPSLATE: Self = Self(204);
    pub const CHISELED_TUFF: Self = Self(205);
    pub const CHISELED_TUFF_BRICKS: Self = Self(206);
    pub const CHISELED_SANDSTONE: Self = Self(207);
    pub const CHISELED_RED_SANDSTONE: Self = Self(208);
    pub const CHISELED_QUARTZ_BLOCK: Self = Self(209);
    pub const NETHER_BRICKS: Self = Self(210);
    pub const NETHERRACK: Self = Self(211);
    pub const NETHER_GOLD_ORE: Self = Self(212);
    pub const NETHER_QUARTZ_ORE: Self = Self(213);
    pub const SOUL_SOIL: Self = Self(214);
    pub const SMOOTH_BASALT: Self = Self(215);
    pub const WARPED_PLANKS: Self = Self(216);
    pub const CRIMSON_PLANKS: Self = Self(217);
    pub const BAMBOO_PLANKS: Self = Self(218);
    pub const CHERRY_PLANKS: Self = Self(219);
    pub const MANGROVE_PLANKS: Self = Self(220);
    pub const ACACIA_PLANKS: Self = Self(221);
    pub const BIRCH_PLANKS: Self = Self(222);
    pub const JUNGLE_PLANKS: Self = Self(223);
    pub const DARK_OAK_PLANKS: Self = Self(224);
}

pub const MAX_WATER_FLOW_LEVEL: u8 = 7;
pub const MAX_LAVA_FLOW_LEVEL: u8 = 7;

pub fn is_water_source_block(block: BlockId) -> bool {
    block == BlockId::STILL_WATER
}

pub fn water_level_from_block(block: BlockId) -> Option<u8> {
    if is_water_source_block(block) {
        return Some(0);
    }

    if (BlockId::FLOWING_WATER_LEVEL0.0..=BlockId::FLOWING_WATER_LEVEL7.0).contains(&block.0) {
        return Some((block.0 - BlockId::FLOWING_WATER_LEVEL0.0) as u8);
    }

    None
}

pub fn is_water_block(block: BlockId) -> bool {
    water_level_from_block(block).is_some()
}

pub fn water_flow_block_from_level(level: u8) -> BlockId {
    let clamped = level.min(MAX_WATER_FLOW_LEVEL);
    BlockId(BlockId::FLOWING_WATER_LEVEL0.0 + u16::from(clamped))
}

pub fn is_lava_source_block(block: BlockId) -> bool {
    block == BlockId::LAVA_SOURCE
}

pub fn lava_level_from_block(block: BlockId) -> Option<u8> {
    if is_lava_source_block(block) {
        return Some(0);
    }

    if (BlockId::FLOWING_LAVA_LEVEL1.0..=BlockId::FLOWING_LAVA_LEVEL7.0).contains(&block.0) {
        return Some((block.0 - BlockId::FLOWING_LAVA_LEVEL1.0 + 1) as u8);
    }

    None
}

pub fn is_lava_block(block: BlockId) -> bool {
    lava_level_from_block(block).is_some()
}

pub fn lava_flow_block_from_level(level: u8) -> BlockId {
    let clamped = level.clamp(1, MAX_LAVA_FLOW_LEVEL);
    BlockId(BlockId::FLOWING_LAVA_LEVEL1.0 + u16::from(clamped - 1))
}

pub fn is_wheat_block(block: BlockId) -> bool {
    (97..=104).contains(&block.0)
}

pub fn wheat_growth_stage(block: BlockId) -> Option<u8> {
    if is_wheat_block(block) {
        Some((block.0 - 97) as u8)
    } else {
        None
    }
}

pub fn wheat_block_at_stage(stage: u8) -> BlockId {
    BlockId(97 + stage.min(7) as u16)
}

pub fn is_trapdoor_block(block: BlockId) -> bool {
    (80..=87).contains(&block.0)
}

pub fn is_bed_block(block: BlockId) -> bool {
    (88..=95).contains(&block.0)
}

pub fn is_slab_block(block: BlockId) -> bool {
    (107..=110).contains(&block.0)
}

pub fn is_glass_pane(block: BlockId) -> bool {
    block.0 == 111
}

pub fn is_lever(block: BlockId) -> bool {
    (112..=113).contains(&block.0)
}

pub fn is_lever_on(block: BlockId) -> bool {
    block.0 == 113
}

pub fn is_button(block: BlockId) -> bool {
    (114..=115).contains(&block.0)
}

pub fn is_button_on(block: BlockId) -> bool {
    block.0 == 115
}

pub fn is_stairs(block: BlockId) -> bool {
    (116..=123).contains(&block.0)
}

pub fn is_stone_stairs(block: BlockId) -> bool {
    (116..=119).contains(&block.0)
}

pub fn is_wooden_stairs(block: BlockId) -> bool {
    (120..=123).contains(&block.0)
}

pub fn is_sign(block: BlockId) -> bool {
    (124..=127).contains(&block.0)
}

pub fn is_wool(block: BlockId) -> bool {
    (128..=143).contains(&block.0)
}

pub fn is_carpet(block: BlockId) -> bool {
    (144..=159).contains(&block.0)
}

pub fn wool_color_index(block: BlockId) -> Option<u8> {
    if is_wool(block) {
        Some((block.0 - 128) as u8)
    } else if is_carpet(block) {
        Some((block.0 - 144) as u8)
    } else {
        None
    }
}

pub fn is_vine(block: BlockId) -> bool {
    (161..=164).contains(&block.0)
}

pub fn is_cobweb(block: BlockId) -> bool {
    block.0 == 160
}

pub fn is_cactus(block: BlockId) -> bool {
    block.0 == 165
}

pub fn is_mushroom(block: BlockId) -> bool {
    (171..=172).contains(&block.0)
}

pub fn is_soul_sand(block: BlockId) -> bool {
    block.0 == 173
}

pub fn is_honey_block(block: BlockId) -> bool {
    block.0 == 174
}

pub fn stairs_facing(block: BlockId) -> Option<u8> {
    // 0=north, 1=east, 2=south, 3=west
    if is_stairs(block) {
        Some(((block.0 - 116) % 4) as u8)
    } else {
        None
    }
}

pub fn is_fire_block(block: BlockId) -> bool {
    block.0 == 105
}

pub fn is_flammable(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::TIMBER_LOG
            | BlockId(7)
            | BlockId::CANOPY_LEAVES
            | BlockId::FENCE
            | BlockId::CRAFTING_TABLE
            | BlockId::CHEST
            | BlockId::BAMBOO_PLANKS
            | BlockId::CHERRY_PLANKS
            | BlockId::MANGROVE_PLANKS
            | BlockId::ACACIA_PLANKS
            | BlockId::BIRCH_PLANKS
            | BlockId::JUNGLE_PLANKS
            | BlockId::DARK_OAK_PLANKS
    ) || is_wool(block)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockProperties {
    pub name: String,
    pub solid: bool,
    pub transparent: bool,
    pub hardness: f32,
    #[serde(default)]
    pub light_level: u8,
}

#[derive(Default, Debug, Clone)]
pub struct BlockRegistry {
    properties: Vec<BlockProperties>,
    by_name: HashMap<String, BlockId>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {
            properties: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    pub fn register(&mut self, props: BlockProperties) -> BlockId {
        if let Some(existing) = self.by_name.get(props.name.as_str()) {
            return *existing;
        }

        let next_index = self.properties.len();
        let id = BlockId(
            u16::try_from(next_index).expect("block registry exceeded BlockId capacity (u16::MAX)"),
        );

        self.by_name.insert(props.name.clone(), id);
        self.properties.push(props);
        id
    }

    pub fn get_properties(&self, id: BlockId) -> &BlockProperties {
        self.properties
            .get(id.0 as usize)
            .or_else(|| self.properties.get(BlockId::AIR.0 as usize))
            .expect("block registry is empty; call register_default_blocks() first")
    }

    pub fn get_by_name(&self, name: &str) -> Option<BlockId> {
        self.by_name.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.properties.len()
    }

    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

pub fn register_default_blocks() -> BlockRegistry {
    fn block(name: &str, solid: bool, transparent: bool, hardness: f32) -> BlockProperties {
        block_with_light(name, solid, transparent, hardness, 0)
    }

    fn block_with_light(
        name: &str,
        solid: bool,
        transparent: bool,
        hardness: f32,
        light_level: u8,
    ) -> BlockProperties {
        BlockProperties {
            name: name.to_string(),
            solid,
            transparent,
            hardness,
            light_level,
        }
    }

    let mut registry = BlockRegistry::new();

    let defaults = [
        block("air", false, true, 0.0),
        block("bedstone", true, false, 1000.0),
        block("granite", true, false, 4.0),
        block("loam", true, false, 1.2),
        block("verdant_turf", true, false, 0.8),
        block("dune_sand", true, false, 0.6),
        block("timber_log", true, false, 2.0),
        block("hewn_plank", true, false, 1.5),
        block("canopy_leaves", true, true, 0.2),
        block("still_water", false, true, 100.0),
        block("rubblestone", true, false, 3.5),
        block("iron_vein", true, false, 5.0),
        block("crystal_pane", true, true, 0.5),
        block("kiln_brick", true, false, 3.0),
        block("gravel_bed", true, false, 0.9),
        block("snowcap", true, true, 0.1),
        block("coal_vein", true, false, 3.0),
        block("copper_vein", true, false, 3.5),
        block("gold_vein", true, false, 5.0),
        block("diamond_vein", true, false, 6.0),
        block("tall_grass", false, true, 0.0),
        block("wildflower", false, true, 0.0),
        block("clay_deposit", true, false, 1.0),
        block("mossy_rubble", true, false, 3.5),
        block("ice", true, true, 0.5),           // 24
        block("packed_ice", true, false, 2.0),     // 25
        block("blue_ice", true, false, 2.8),       // 26
        block("hardened_clay", true, false, 3.0),   // 27
        block_with_light("magma_block", true, false, 3.0, 6), // 28
        block("sand", true, false, 0.5),            // 29
        block("tuff", true, false, 3.0),            // 30
        block("netherite_block", true, false, 50.0), // 31
        block("polished_blackstone", true, false, 3.0), // 32
        block("dripstone_block", true, false, 3.0),  // 33
        block_with_light("torch", false, true, 0.0, 14), // 34
        block("flowing_water_0", false, true, 100.0), // 35
        block("flowing_water_1", false, true, 100.0), // 36
        block("flowing_water_2", false, true, 100.0), // 37
        block("flowing_water_3", false, true, 100.0), // 38
        block("flowing_water_4", false, true, 100.0), // 39
        block("flowing_water_5", false, true, 100.0), // 40
        block("flowing_water_6", false, true, 100.0), // 41
        block("flowing_water_7", false, true, 100.0), // 42
        block("wooden_door", false, true, 1.5), // 43
        block("door_lower", true, true, 1.5), // 44
        block("door_upper", true, true, 1.5), // 45
        block("door_lower_east", true, true, 1.5), // 46
        block("door_upper_east", true, true, 1.5), // 47
        block("door_lower_south", true, true, 1.5), // 48
        block("door_upper_south", true, true, 1.5), // 49
        block("door_lower_west", true, true, 1.5), // 50
        block("door_upper_west", true, true, 1.5), // 51
        block("door_lower_open", false, true, 1.5), // 52
        block("door_upper_open", false, true, 1.5), // 53
        block("door_lower_open_east", false, true, 1.5), // 54
        block("door_upper_open_east", false, true, 1.5), // 55
        block("door_lower_open_south", false, true, 1.5), // 56
        block("door_upper_open_south", false, true, 1.5), // 57
        block("door_lower_open_west", false, true, 1.5), // 58
        block("door_upper_open_west", false, true, 1.5), // 59
        block("ladder", false, true, 0.4), // 60
        block("ladder_east", false, true, 0.4), // 61
        block("ladder_south", false, true, 0.4), // 62
        block("ladder_west", false, true, 0.4), // 63
        block("fence", true, true, 2.0), // 64
        block("crafting_table", true, false, 2.5), // 65
        block_with_light("furnace", true, false, 3.5, 10), // 66
        block("chest", true, false, 2.5), // 67
        block("sapling", false, true, 0.0), // 68
        block("sugar_cane", false, true, 0.0), // 69
        block_with_light("lava_source", false, false, 100.0, 14), // 70
        block_with_light("flowing_lava_1", false, false, 100.0, 14), // 71
        block_with_light("flowing_lava_2", false, false, 100.0, 14), // 72
        block_with_light("flowing_lava_3", false, false, 100.0, 14), // 73
        block_with_light("flowing_lava_4", false, false, 100.0, 14), // 74
        block_with_light("flowing_lava_5", false, false, 100.0, 14), // 75
        block_with_light("flowing_lava_6", false, false, 100.0, 14), // 76
        block_with_light("flowing_lava_7", false, false, 100.0, 14), // 77
        block("obsidian", true, false, 40.0), // 78
        block("tnt", true, false, 0.0), // 79
        block("trapdoor_closed", false, true, 3.0), // 80
        block("trapdoor_open", false, true, 3.0), // 81
        block("trapdoor_closed_east", false, true, 3.0), // 82
        block("trapdoor_open_east", false, true, 3.0), // 83
        block("trapdoor_closed_south", false, true, 3.0), // 84
        block("trapdoor_open_south", false, true, 3.0), // 85
        block("trapdoor_closed_west", false, true, 3.0), // 86
        block("trapdoor_open_west", false, true, 3.0), // 87
        block("bed_foot", false, true, 0.2), // 88
        block("bed_head", false, true, 0.2), // 89
        block("bed_foot_east", false, true, 0.2), // 90
        block("bed_head_east", false, true, 0.2), // 91
        block("bed_foot_south", false, true, 0.2), // 92
        block("bed_head_south", false, true, 0.2), // 93
        block("bed_foot_west", false, true, 0.2), // 94
        block("bed_head_west", false, true, 0.2), // 95
        block("farmland", true, false, 0.6), // 96
        block("wheat_stage_0", false, true, 0.0), // 97
        block("wheat_stage_1", false, true, 0.0), // 98
        block("wheat_stage_2", false, true, 0.0), // 99
        block("wheat_stage_3", false, true, 0.0), // 100
        block("wheat_stage_4", false, true, 0.0), // 101
        block("wheat_stage_5", false, true, 0.0), // 102
        block("wheat_stage_6", false, true, 0.0), // 103
        block("wheat_stage_7", false, true, 0.0), // 104
        block_with_light("fire", false, true, 0.0, 14), // 105
        block("stone_pressure_plate", false, true, 0.5), // 106
        block("stone_slab_bottom", true, true, 2.0), // 107
        block("stone_slab_top", true, true, 2.0), // 108
        block("wooden_slab_bottom", true, true, 2.0), // 109
        block("wooden_slab_top", true, true, 2.0), // 110
        block("glass_pane", false, true, 0.3), // 111
        block("lever_off", false, true, 0.5), // 112
        block("lever_on", false, true, 0.5), // 113
        block("stone_button_off", false, true, 0.5), // 114
        block("stone_button_on", false, true, 0.5), // 115
        block("stone_stairs_north", true, true, 2.0), // 116
        block("stone_stairs_east", true, true, 2.0), // 117
        block("stone_stairs_south", true, true, 2.0), // 118
        block("stone_stairs_west", true, true, 2.0), // 119
        block("wooden_stairs_north", true, true, 2.0), // 120
        block("wooden_stairs_east", true, true, 2.0), // 121
        block("wooden_stairs_south", true, true, 2.0), // 122
        block("wooden_stairs_west", true, true, 2.0), // 123
        block("sign_north", false, true, 1.0), // 124
        block("sign_south", false, true, 1.0), // 125
        block("sign_east", false, true, 1.0), // 126
        block("sign_west", false, true, 1.0), // 127
        block("wool_white", true, false, 0.8), // 128
        block("wool_orange", true, false, 0.8), // 129
        block("wool_magenta", true, false, 0.8), // 130
        block("wool_light_blue", true, false, 0.8), // 131
        block("wool_yellow", true, false, 0.8), // 132
        block("wool_lime", true, false, 0.8), // 133
        block("wool_pink", true, false, 0.8), // 134
        block("wool_gray", true, false, 0.8), // 135
        block("wool_light_gray", true, false, 0.8), // 136
        block("wool_cyan", true, false, 0.8), // 137
        block("wool_purple", true, false, 0.8), // 138
        block("wool_blue", true, false, 0.8), // 139
        block("wool_brown", true, false, 0.8), // 140
        block("wool_green", true, false, 0.8), // 141
        block("wool_red", true, false, 0.8), // 142
        block("wool_black", true, false, 0.8), // 143
        block("carpet_white", false, true, 0.1), // 144
        block("carpet_orange", false, true, 0.1), // 145
        block("carpet_magenta", false, true, 0.1), // 146
        block("carpet_light_blue", false, true, 0.1), // 147
        block("carpet_yellow", false, true, 0.1), // 148
        block("carpet_lime", false, true, 0.1), // 149
        block("carpet_pink", false, true, 0.1), // 150
        block("carpet_gray", false, true, 0.1), // 151
        block("carpet_light_gray", false, true, 0.1), // 152
        block("carpet_cyan", false, true, 0.1), // 153
        block("carpet_purple", false, true, 0.1), // 154
        block("carpet_blue", false, true, 0.1), // 155
        block("carpet_brown", false, true, 0.1), // 156
        block("carpet_green", false, true, 0.1), // 157
        block("carpet_red", false, true, 0.1), // 158
        block("carpet_black", false, true, 0.1), // 159
        block("cobweb", false, true, 4.0), // 160
        block("vine_north", false, true, 0.2), // 161
        block("vine_east", false, true, 0.2), // 162
        block("vine_south", false, true, 0.2), // 163
        block("vine_west", false, true, 0.2), // 164
        block("cactus", true, true, 0.4), // 165
        block("pumpkin", true, false, 1.0), // 166
        block_with_light("jack_o_lantern", true, false, 1.0, 15), // 167
        block("melon", true, false, 1.0), // 168
        block("hay_bale", true, false, 0.5), // 169
        block("bookshelf", true, false, 1.5), // 170
        block("red_mushroom", false, true, 0.0), // 171
        block("brown_mushroom", false, true, 0.0), // 172
        block("soul_sand", true, false, 0.5), // 173
        block("honey_block", true, true, 0.0), // 174
        block("andesite", true, false, 3.0), // 175
        block("diorite", true, false, 3.0), // 176
        block("polished_andesite", true, false, 3.2), // 177
        block("polished_diorite", true, false, 3.2), // 178
        block("polished_granite", true, false, 3.2), // 179
        block("deepslate", true, false, 4.0), // 180
        block("deepslate_bricks", true, false, 4.0), // 181
        block("deepslate_tiles", true, false, 4.0), // 182
        block("polished_deepslate", true, false, 4.2), // 183
        block("calcite", true, false, 2.5), // 184
        block("blackstone", true, false, 3.6), // 185
        block("polished_blackstone_bricks", true, false, 3.8), // 186
        block("tuff_bricks", true, false, 3.5), // 187
        block("prismarine", true, false, 3.0), // 188
        block("prismarine_bricks", true, false, 3.2), // 189
        block("end_stone", true, false, 3.0), // 190
        block("end_stone_bricks", true, false, 3.2), // 191
        block("quartz_bricks", true, false, 2.8), // 192
        block("quartz_block_side", true, false, 2.8), // 193
        block("smooth_stone", true, false, 2.6), // 194
        block("sandstone_top", true, false, 1.0), // 195
        block("red_sandstone", true, false, 1.0), // 196
        block("red_sandstone_top", true, false, 1.0), // 197
        block("mud", true, false, 0.8), // 198
        block("mud_bricks", true, false, 1.5), // 199
        block("packed_mud", true, false, 1.2), // 200
        block("moss_block", true, false, 0.4), // 201
        block("mossy_stone_bricks", true, false, 3.2), // 202
        block("chiseled_stone_bricks", true, false, 3.2), // 203
        block("chiseled_deepslate", true, false, 4.2), // 204
        block("chiseled_tuff", true, false, 3.5), // 205
        block("chiseled_tuff_bricks", true, false, 3.5), // 206
        block("chiseled_sandstone", true, false, 1.2), // 207
        block("chiseled_red_sandstone", true, false, 1.2), // 208
        block("chiseled_quartz_block", true, false, 2.8), // 209
        block("nether_bricks", true, false, 2.5), // 210
        block("netherrack", true, false, 0.6), // 211
        block("nether_gold_ore", true, false, 3.2), // 212
        block("nether_quartz_ore", true, false, 3.0), // 213
        block("soul_soil", true, false, 0.8), // 214
        block("smooth_basalt", true, false, 4.2), // 215
        block("warped_planks", true, false, 2.0), // 216
        block("crimson_planks", true, false, 2.0), // 217
        block("bamboo_planks", true, false, 1.6), // 218
        block("cherry_planks", true, false, 2.0), // 219
        block("mangrove_planks", true, false, 2.0), // 220
        block("acacia_planks", true, false, 2.0), // 221
        block("birch_planks", true, false, 2.0), // 222
        block("jungle_planks", true, false, 2.0), // 223
        block("dark_oak_planks", true, false, 2.0), // 224
    ];

    for (idx, props) in defaults.into_iter().enumerate() {
        let id = registry.register(props);
        debug_assert_eq!(id.0 as usize, idx, "default block IDs must be stable");
    }

    registry
}

#[cfg(test)]
mod tests {
    use super::{
        is_lava_block, is_lava_source_block, is_water_block, is_water_source_block,
        lava_flow_block_from_level, lava_level_from_block, register_default_blocks,
        water_flow_block_from_level, water_level_from_block, BlockId,
    };

    #[test]
    fn registry_returns_known_block_properties() {
        let registry = register_default_blocks();

        let air = registry.get_properties(BlockId::AIR);
        assert_eq!(air.name, "air");
        assert!(!air.solid);
        assert!(air.transparent);
        assert_eq!(air.light_level, 0);

        let water_id = registry
            .get_by_name("still_water")
            .expect("still_water should be registered");
        assert_eq!(water_id, BlockId(9));
        let water = registry.get_properties(water_id);
        assert_eq!(water.name, "still_water");
        assert!(!water.solid);
        assert!(water.transparent);
        assert_eq!(water.light_level, 0);

        let granite = registry
            .get_by_name("granite")
            .expect("granite should be registered");
        assert_eq!(granite, BlockId(2));

        let torch = registry
            .get_by_name("torch")
            .expect("torch should be registered");
        assert_eq!(torch, BlockId(34));
        let torch_props = registry.get_properties(torch);
        assert_eq!(torch_props.name, "torch");
        assert!(!torch_props.solid);
        assert!(torch_props.transparent);
        assert_eq!(torch_props.light_level, 14);

        let wooden_door = registry
            .get_by_name("wooden_door")
            .expect("wooden_door should be registered");
        assert_eq!(wooden_door, BlockId::WOODEN_DOOR);

        let fence = registry
            .get_by_name("fence")
            .expect("fence should be registered");
        assert_eq!(fence, BlockId::FENCE);

        let crafting_table = registry
            .get_by_name("crafting_table")
            .expect("crafting_table should be registered");
        assert_eq!(crafting_table, BlockId::CRAFTING_TABLE);

        let furnace = registry
            .get_by_name("furnace")
            .expect("furnace should be registered");
        assert_eq!(furnace, BlockId::FURNACE);
        let furnace_props = registry.get_properties(furnace);
        assert_eq!(furnace_props.light_level, 10);

        let chest = registry
            .get_by_name("chest")
            .expect("chest should be registered");
        assert_eq!(chest, BlockId::CHEST);

        let magma = registry
            .get_by_name("magma_block")
            .expect("magma_block should be registered");
        let magma_props = registry.get_properties(magma);
        assert_eq!(magma_props.light_level, 6);

        let lava_source = registry
            .get_by_name("lava_source")
            .expect("lava_source should be registered");
        assert_eq!(lava_source, BlockId::LAVA_SOURCE);
        let lava_source_props = registry.get_properties(lava_source);
        assert_eq!(lava_source_props.light_level, 14);
        assert!(!lava_source_props.solid);
        assert!(!lava_source_props.transparent);

        let obsidian = registry
            .get_by_name("obsidian")
            .expect("obsidian should be registered");
        assert_eq!(obsidian, BlockId::OBSIDIAN);
        let obsidian_props = registry.get_properties(obsidian);
        assert!(obsidian_props.solid);
        assert!(!obsidian_props.transparent);
        assert!(obsidian_props.hardness >= 40.0);

        let sapling = registry
            .get_by_name("sapling")
            .expect("sapling should be registered");
        assert_eq!(sapling, BlockId::SAPLING);
        let sapling_props = registry.get_properties(sapling);
        assert!(!sapling_props.solid);
        assert!(sapling_props.transparent);

        let sugar_cane = registry
            .get_by_name("sugar_cane")
            .expect("sugar_cane should be registered");
        assert_eq!(sugar_cane, BlockId::SUGAR_CANE);
        let sugar_cane_props = registry.get_properties(sugar_cane);
        assert!(!sugar_cane_props.solid);
        assert!(sugar_cane_props.transparent);

        let andesite = registry
            .get_by_name("andesite")
            .expect("andesite should be registered");
        assert_eq!(andesite, BlockId::ANDESITE);

        let dark_oak_planks = registry
            .get_by_name("dark_oak_planks")
            .expect("dark_oak_planks should be registered");
        assert_eq!(dark_oak_planks, BlockId::DARK_OAK_PLANKS);
        assert_eq!(registry.len(), 225);
    }

    #[test]
    fn transparency_and_solidity_checks_match_default_blocks() {
        let registry = register_default_blocks();

        let granite = registry
            .get_properties(registry.get_by_name("granite").expect("granite missing"));
        let leaves = registry
            .get_properties(registry.get_by_name("canopy_leaves").expect("leaves missing"));

        assert!(granite.solid);
        assert!(!granite.transparent);
        assert!(leaves.solid);
        assert!(leaves.transparent);
    }

    #[test]
    fn block_id_comparisons_work() {
        assert_eq!(BlockId(4), BlockId(4));
        assert_ne!(BlockId(4), BlockId(5));
        assert!(BlockId(4) < BlockId(5));
        assert!(BlockId(10) > BlockId(2));
    }

    #[test]
    fn water_helpers_map_source_and_flow_levels() {
        assert!(is_water_source_block(BlockId::STILL_WATER));
        assert_eq!(water_level_from_block(BlockId::STILL_WATER), Some(0));

        for level in 0..=7u8 {
            let block = water_flow_block_from_level(level);
            assert!(is_water_block(block));
            assert_eq!(water_level_from_block(block), Some(level));
        }

        assert!(!is_water_block(BlockId::AIR));
    }

    #[test]
    fn lava_helpers_map_source_and_flow_levels() {
        assert!(is_lava_source_block(BlockId::LAVA_SOURCE));
        assert_eq!(lava_level_from_block(BlockId::LAVA_SOURCE), Some(0));

        for level in 1..=7u8 {
            let block = lava_flow_block_from_level(level);
            assert!(is_lava_block(block));
            assert_eq!(lava_level_from_block(block), Some(level));
        }

        assert_eq!(lava_flow_block_from_level(0), BlockId::FLOWING_LAVA_LEVEL1);
        assert!(!is_lava_block(BlockId::AIR));
    }
}
