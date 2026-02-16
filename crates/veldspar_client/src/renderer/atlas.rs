use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use veldspar_shared::block::{register_default_blocks, BlockId, BlockRegistry};
use veldspar_shared::inventory::{ItemId, FIRST_NON_BLOCK_ITEM_ID};

const ATLAS_SIZE: u32 = 512;
const TILE_SIZE: u32 = 16;
const TILES_PER_ROW: u32 = ATLAS_SIZE / TILE_SIZE;

#[derive(Debug, Clone, Default)]
pub struct AtlasMapping {
    offsets: Vec<Option<[f32; 2]>>,
    item_offsets: Vec<Option<[f32; 2]>>,
    texture_offsets: HashMap<String, [f32; 2]>,
}

impl AtlasMapping {
    pub fn set_offset(&mut self, block_id: BlockId, u_offset: f32, v_offset: f32) {
        let idx = usize::from(block_id.0);
        if self.offsets.len() <= idx {
            self.offsets.resize(idx + 1, None);
        }
        self.offsets[idx] = Some([u_offset, v_offset]);
    }

    pub fn set_texture_offset(&mut self, texture_name: &str, u_offset: f32, v_offset: f32) {
        self.texture_offsets
            .insert(texture_name.to_string(), [u_offset, v_offset]);
    }

    pub fn set_item_offset(&mut self, item_id: ItemId, u_offset: f32, v_offset: f32) {
        let idx = usize::from(item_id.0);
        if self.item_offsets.len() <= idx {
            self.item_offsets.resize(idx + 1, None);
        }
        self.item_offsets[idx] = Some([u_offset, v_offset]);
    }

    pub fn offset(&self, block_id: BlockId) -> Option<[f32; 2]> {
        self.offsets
            .get(usize::from(block_id.0))
            .and_then(|entry| *entry)
    }

    pub fn item_offset(&self, item_id: ItemId) -> Option<[f32; 2]> {
        self.item_offsets
            .get(usize::from(item_id.0))
            .and_then(|entry| *entry)
    }

    pub fn offset_for_item(&self, item_id: ItemId) -> Option<[f32; 2]> {
        self.item_offset(item_id)
            .or_else(|| item_id.as_block_id().and_then(|block| self.offset(block)))
    }
}

#[derive(Debug)]
pub enum AtlasBuildError {
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    DecodeImage {
        path: PathBuf,
        source: image::ImageError,
    },
    InvalidFileStem {
        path: PathBuf,
    },
    MissingTexture {
        block_name: String,
        texture_name: String,
    },
}

impl fmt::Display for AtlasBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadDir { path, source } => {
                write!(f, "failed to read texture directory {}: {source}", path.display())
            }
            Self::ReadFile { path, source } => {
                write!(f, "failed to read texture file {}: {source}", path.display())
            }
            Self::DecodeImage { path, source } => {
                write!(f, "failed to decode png {}: {source}", path.display())
            }
            Self::InvalidFileStem { path } => {
                write!(f, "texture path has no valid file stem: {}", path.display())
            }
            Self::MissingTexture {
                block_name,
                texture_name,
            } => write!(
                f,
                "missing texture '{texture_name}.png' for block '{block_name}'"
            ),
        }
    }
}

impl std::error::Error for AtlasBuildError {}

pub fn build_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    textures_dir: impl AsRef<Path>,
) -> Result<(wgpu::Texture, wgpu::TextureView, wgpu::Sampler, AtlasMapping), AtlasBuildError> {
    let textures_dir = textures_dir.as_ref();
    let registry = register_default_blocks();
    let required_texture_names = collect_required_texture_names(&registry);
    let loaded_tiles = load_and_resize_tiles(textures_dir, Some(&required_texture_names))?;

    let mut atlas_image =
        RgbaImage::from_pixel(ATLAS_SIZE, ATLAS_SIZE, Rgba([0_u8, 0_u8, 0_u8, 0_u8]));
    let mut mapping = AtlasMapping::default();

    for raw_id in 0..registry.len() {
        let block_id = BlockId(
            u16::try_from(raw_id).expect("default block registry index should fit in u16"),
        );
        let props = registry.get_properties(block_id);
        let Some(texture_name) = resolve_texture_name_for_block(&props.name, &loaded_tiles) else {
            continue;
        };

        let tile = loaded_tiles
            .get(texture_name.as_str())
            .ok_or_else(|| AtlasBuildError::MissingTexture {
                block_name: props.name.clone(),
                texture_name: texture_name.clone(),
            })?;

        blit_block_tile(&mut atlas_image, block_id, tile);

        let slot = u32::from(block_id.0);
        let tile_x = slot % TILES_PER_ROW;
        let tile_y = slot / TILES_PER_ROW;
        let u_offset = (tile_x * TILE_SIZE) as f32 / ATLAS_SIZE as f32;
        let v_offset = (tile_y * TILE_SIZE) as f32 / ATLAS_SIZE as f32;
        mapping.set_offset(block_id, u_offset, v_offset);
        mapping.set_texture_offset(texture_name.as_str(), u_offset, v_offset);
    }

    append_extra_texture_slots(&mut atlas_image, &loaded_tiles, &mut mapping);
    append_item_texture_slots(textures_dir, &registry, &mut atlas_image, &mut mapping)?;

    let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Chunk Atlas Texture"),
        size: wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &atlas_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        atlas_image.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(ATLAS_SIZE * 4),
            rows_per_image: Some(ATLAS_SIZE),
        },
        wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
    );

    let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Chunk Atlas Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    Ok((atlas_texture, atlas_view, atlas_sampler, mapping))
}

fn append_item_texture_slots(
    block_textures_dir: &Path,
    registry: &BlockRegistry,
    atlas: &mut RgbaImage,
    mapping: &mut AtlasMapping,
) -> Result<(), AtlasBuildError> {
    let Some(item_textures_dir) = block_textures_dir.parent().map(|dir| dir.join("item")) else {
        return Ok(());
    };
    if !item_textures_dir.is_dir() {
        return Ok(());
    }

    let required_names = collect_required_item_texture_names();
    let loaded_tiles = load_and_resize_tiles(&item_textures_dir, Some(&required_names))?;
    if loaded_tiles.is_empty() {
        return Ok(());
    }

    let atlas_capacity = TILES_PER_ROW * TILES_PER_ROW;
    let mut used_slots: HashSet<u32> = HashSet::new();
    for raw_id in 0..registry.len() {
        used_slots.insert(raw_id as u32);
    }
    for (_, slot) in extra_texture_slots() {
        used_slots.insert(slot);
    }

    let mut texture_names: Vec<String> = loaded_tiles.keys().cloned().collect();
    texture_names.sort();

    let mut texture_offsets: HashMap<String, [f32; 2]> = HashMap::new();
    let mut next_slot = registry.len() as u32;
    for texture_name in texture_names {
        while used_slots.contains(&next_slot) {
            next_slot += 1;
        }
        if next_slot >= atlas_capacity {
            break;
        }

        let Some(tile) = loaded_tiles.get(texture_name.as_str()) else {
            continue;
        };
        blit_tile_at_slot(atlas, next_slot, tile);
        let tile_x = next_slot % TILES_PER_ROW;
        let tile_y = next_slot / TILES_PER_ROW;
        let offset = [
            (tile_x * TILE_SIZE) as f32 / ATLAS_SIZE as f32,
            (tile_y * TILE_SIZE) as f32 / ATLAS_SIZE as f32,
        ];
        mapping.set_texture_offset(texture_name.as_str(), offset[0], offset[1]);
        texture_offsets.insert(texture_name, offset);
        used_slots.insert(next_slot);
        next_slot += 1;
    }

    for raw_id in FIRST_NON_BLOCK_ITEM_ID..=ItemId::GOLD_BOOTS.0 {
        let item_id = ItemId(raw_id);
        let Some(candidates) = item_texture_name_candidates(item_id) else {
            continue;
        };
        if let Some(offset) = candidates
            .iter()
            .find_map(|name| texture_offsets.get(*name).copied())
        {
            mapping.set_item_offset(item_id, offset[0], offset[1]);
        }
    }

    Ok(())
}

fn load_and_resize_tiles(
    textures_dir: &Path,
    required_texture_names: Option<&HashSet<String>>,
) -> Result<HashMap<String, RgbaImage>, AtlasBuildError> {
    let read_dir = std::fs::read_dir(textures_dir).map_err(|source| AtlasBuildError::ReadDir {
        path: textures_dir.to_path_buf(),
        source,
    })?;

    let mut png_paths = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|source| AtlasBuildError::ReadDir {
            path: textures_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let is_png = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("png"))
            .unwrap_or(false);
        if is_png {
            png_paths.push(path);
        }
    }
    png_paths.sort();

    let mut resized_tiles = HashMap::with_capacity(png_paths.len());
    for path in png_paths {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| AtlasBuildError::InvalidFileStem { path: path.clone() })?;
        if required_texture_names
            .is_some_and(|required| !required.contains(stem))
        {
            continue;
        }

        let bytes = std::fs::read(&path).map_err(|source| AtlasBuildError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let decoded = image::load_from_memory(&bytes)
            .map_err(|source| AtlasBuildError::DecodeImage {
                path: path.clone(),
                source,
            })?;

        let resized = image::imageops::resize(
            &decoded.to_rgba8(),
            TILE_SIZE,
            TILE_SIZE,
            FilterType::Nearest,
        );

        resized_tiles.insert(stem.to_string(), resized);
    }

    Ok(resized_tiles)
}

fn collect_required_texture_names(registry: &BlockRegistry) -> HashSet<String> {
    let mut required = HashSet::new();
    required.insert("stone".to_string());
    required.insert("water_still".to_string());
    required.insert("water_flow".to_string());
    required.insert("lava_still".to_string());
    required.insert("lava_flow".to_string());

    for raw_id in 0..registry.len() {
        let block_id = BlockId(
            u16::try_from(raw_id).expect("default block registry index should fit in u16"),
        );
        let block_name = registry.get_properties(block_id).name.as_str();
        for candidate in texture_name_candidates(block_name) {
            required.insert(candidate);
        }
    }

    for (name, _) in [
        ("oak_log_top", 960u32),
        ("oak_log", 961),
        ("crafting_table_top", 962),
        ("crafting_table_front", 963),
        ("crafting_table_side", 964),
        ("torch", 965),
        ("torch_flame", 966),
        ("water_still", 967),
        ("water_flow", 968),
        ("lava_still", 969),
        ("lava_flow", 970),
    ] {
        required.insert(name.to_string());
    }

    required
}

fn collect_required_item_texture_names() -> HashSet<String> {
    let mut required = HashSet::new();
    for raw_id in FIRST_NON_BLOCK_ITEM_ID..=ItemId::GOLD_BOOTS.0 {
        if let Some(candidates) = item_texture_name_candidates(ItemId(raw_id)) {
            for name in candidates {
                required.insert((*name).to_string());
            }
        }
    }
    required
}

fn resolve_texture_name_for_block(
    block_name: &str,
    loaded_tiles: &HashMap<String, RgbaImage>,
) -> Option<String> {
    if block_name == "air" {
        return None;
    }

    for candidate in texture_name_candidates(block_name) {
        if loaded_tiles.contains_key(candidate.as_str()) {
            return Some(candidate);
        }
    }

    Some("stone".to_string())
}

fn texture_name_candidates(block_name: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let normalized = block_name
        .trim_end_matches("_north")
        .trim_end_matches("_east")
        .trim_end_matches("_south")
        .trim_end_matches("_west")
        .trim_end_matches("_open")
        .trim_end_matches("_closed")
        .trim_end_matches("_on")
        .trim_end_matches("_off");

    match block_name {
        "bedstone" => candidates.push("bedrock".to_string()),
        "granite" => {
            candidates.push("stone".to_string());
            candidates.push("andesite1".to_string());
        }
        "loam" => candidates.push("dirt".to_string()),
        "verdant_turf" => candidates.push("grass_block_top".to_string()),
        "dune_sand" | "sand" => candidates.push("sand".to_string()),
        "timber_log" => candidates.push("oak_log".to_string()),
        "hewn_plank" | "fence" => candidates.push("oak_planks".to_string()),
        "canopy_leaves" => candidates.push("oak_leaves".to_string()),
        "snowcap" => candidates.push("snow".to_string()),
        "still_water" => {
            candidates.push("water_still".to_string());
            candidates.push("water_flow".to_string());
        }
        name if name.starts_with("flowing_water_") => {
            candidates.push("water_flow".to_string());
            candidates.push("water_still".to_string());
        }
        "rubblestone" => candidates.push("cobblestone".to_string()),
        "iron_vein" => candidates.push("iron_ore".to_string()),
        "coal_vein" => candidates.push("coal_ore".to_string()),
        "copper_vein" => candidates.push("copper_ore".to_string()),
        "gold_vein" => candidates.push("gold_ore".to_string()),
        "diamond_vein" => candidates.push("diamond_ore".to_string()),
        "tall_grass" => candidates.push("short_grass1".to_string()),
        "wildflower" => candidates.push("dandelion1".to_string()),
        "clay_deposit" => candidates.push("clay".to_string()),
        "kiln_brick" => candidates.push("stonebrick1".to_string()),
        "gravel_bed" => {
            candidates.push("gravel3".to_string());
            candidates.push("gravel".to_string());
        }
        "mossy_rubble" => candidates.push("mossy_cobblestone".to_string()),
        "ice" => candidates.push("ice".to_string()),
        "packed_ice" => candidates.push("packed_ice".to_string()),
        "blue_ice" => candidates.push("blue_ice".to_string()),
        "hardened_clay" => candidates.push("terracotta".to_string()),
        "magma_block" => {
            candidates.push("magma_block".to_string());
            candidates.push("magma".to_string());
        }
        "tuff" => candidates.push("tuff".to_string()),
        "netherite_block" => candidates.push("netherite_block".to_string()),
        "polished_blackstone" => candidates.push("polished_blackstone".to_string()),
        "dripstone_block" => candidates.push("dripstone_block".to_string()),
        "torch" => candidates.push("torch".to_string()),
        "wooden_door" => candidates.push("oak_door_side".to_string()),
        name if name.starts_with("door_") => candidates.push("oak_door_side".to_string()),
        "ladder" | "ladder_east" | "ladder_south" | "ladder_west" => {
            candidates.push("ladder".to_string())
        }
        "crafting_table" => candidates.push("crafting_table_side".to_string()),
        "furnace" => candidates.push("furnace_side".to_string()),
        "chest" => candidates.push("oak_planks".to_string()),
        "sapling" => candidates.push("oak_sapling".to_string()),
        "sugar_cane" => candidates.push("sugar_cane".to_string()),
        "lava_source" => {
            candidates.push("lava_still".to_string());
            candidates.push("lava_flow".to_string());
        }
        name if name.starts_with("flowing_lava_") => {
            candidates.push("lava_flow".to_string());
            candidates.push("lava_still".to_string());
        }
        "obsidian" => candidates.push("obsidian".to_string()),
        "tnt" => candidates.push("tnt_side".to_string()),
        name if name.starts_with("trapdoor_") => candidates.push("oak_trapdoor".to_string()),
        name if name.starts_with("bed_") => {
            candidates.push("white_wool".to_string());
            candidates.push("oak_planks".to_string());
        }
        "farmland" => {
            candidates.push("farmland".to_string());
            candidates.push("farmland_side".to_string());
        }
        name if name.starts_with("wheat_stage_") => {
            let stage = name.trim_start_matches("wheat_stage_");
            candidates.push(format!("wheat_stage{stage}"));
            candidates.push(format!("wheat_stage_full_{stage}"));
        }
        "fire" => {
            candidates.push("fire_0".to_string());
            candidates.push("fire_1".to_string());
        }
        "stone_pressure_plate" => {
            candidates.push("plate_stone".to_string());
            candidates.push("button_stone".to_string());
        }
        "stone_slab_bottom" | "stone_slab_top" => {
            candidates.push("cobblestone_slab".to_string());
            candidates.push("stone".to_string());
        }
        "wooden_slab_bottom" | "wooden_slab_top" => {
            candidates.push("oak_planks".to_string());
            candidates.push("acacia_slab".to_string());
        }
        "glass_pane" => {
            candidates.push("glass_simple".to_string());
            candidates.push("glass".to_string());
        }
        "crystal_pane" => {
            candidates.push("glass_simple".to_string());
            candidates.push("glass".to_string());
        }
        "lever_off" | "lever_on" => candidates.push("button_stone".to_string()),
        "stone_button_off" | "stone_button_on" => candidates.push("button_stone".to_string()),
        name if name.starts_with("stone_stairs_") => {
            candidates.push("cobblestone".to_string());
            candidates.push("stone".to_string());
        }
        name if name.starts_with("wooden_stairs_") => candidates.push("oak_planks".to_string()),
        name if name.starts_with("sign_") => candidates.push("oak_planks".to_string()),
        name if name.starts_with("wool_") => {
            let color = name.trim_start_matches("wool_");
            candidates.push(format!("{color}_wool"));
        }
        name if name.starts_with("carpet_") => {
            let color = name.trim_start_matches("carpet_");
            candidates.push(format!("{color}_carpet"));
            candidates.push(format!("{color}_wool"));
        }
        "cobweb" => {
            candidates.push("cobweb".to_string());
            candidates.push("white_wool".to_string());
        }
        name if name.starts_with("vine_") => candidates.push("vine".to_string()),
        "cactus" => candidates.push("cactus_side".to_string()),
        "pumpkin" => candidates.push("pumpkin_side".to_string()),
        "jack_o_lantern" => candidates.push("jack_o_lantern".to_string()),
        "melon" => candidates.push("melon_side".to_string()),
        "hay_bale" => candidates.push("hay_block_side".to_string()),
        "bookshelf" => candidates.push("bookshelf".to_string()),
        "red_mushroom" => candidates.push("red_mushroom".to_string()),
        "brown_mushroom" => candidates.push("brown_mushroom".to_string()),
        "soul_sand" => candidates.push("soul_sand".to_string()),
        "honey_block" => candidates.push("honey_block_side".to_string()),
        other => candidates.push(other.to_string()),
    }

    if normalized != block_name {
        candidates.push(normalized.to_string());
    }

    if block_name.contains("door") {
        candidates.push("oak_door_side".to_string());
    }
    if block_name.contains("trapdoor") {
        candidates.push("oak_trapdoor".to_string());
    }
    if block_name.contains("stairs") {
        if block_name.contains("stone") {
            candidates.push("cobblestone".to_string());
        } else {
            candidates.push("oak_planks".to_string());
        }
    }
    if block_name.contains("slab") {
        if block_name.contains("stone") {
            candidates.push("cobblestone_slab".to_string());
            candidates.push("stone".to_string());
        } else {
            candidates.push("oak_planks".to_string());
        }
    }
    if block_name.contains("button")
        || block_name.contains("lever")
        || block_name.contains("pressure_plate")
    {
        candidates.push("button_stone".to_string());
    }
    if block_name.contains("log") {
        candidates.push("oak_log".to_string());
    }
    if block_name.contains("plank") {
        candidates.push("oak_planks".to_string());
    }
    if block_name.contains("leaf") {
        candidates.push("oak_leaves".to_string());
    }
    if block_name.contains("ore") || block_name.ends_with("_vein") {
        candidates.push("iron_ore".to_string());
    }
    if block_name.contains("water") {
        candidates.push("water_still".to_string());
        candidates.push("water_flow".to_string());
    }
    if block_name.contains("lava") {
        candidates.push("lava_still".to_string());
        candidates.push("lava_flow".to_string());
    }

    candidates.push("stone".to_string());
    candidates
}

fn append_extra_texture_slots(
    atlas: &mut RgbaImage,
    loaded_tiles: &HashMap<String, RgbaImage>,
    mapping: &mut AtlasMapping,
) {
    for (texture_name, slot) in extra_texture_slots() {
        let Some(tile) = loaded_tiles.get(texture_name) else {
            continue;
        };
        if slot >= TILES_PER_ROW * TILES_PER_ROW {
            break;
        }

        blit_tile_at_slot(atlas, slot, tile);
        let tile_x = slot % TILES_PER_ROW;
        let tile_y = slot / TILES_PER_ROW;
        mapping.set_texture_offset(
            texture_name,
            (tile_x * TILE_SIZE) as f32 / ATLAS_SIZE as f32,
            (tile_y * TILE_SIZE) as f32 / ATLAS_SIZE as f32,
        );
    }
}

fn extra_texture_slots() -> [(&'static str, u32); 11] {
    [
        ("oak_log_top", 960),
        ("oak_log", 961),
        ("crafting_table_top", 962),
        ("crafting_table_front", 963),
        ("crafting_table_side", 964),
        ("torch", 965),
        ("torch_flame", 966),
        ("water_still", 967),
        ("water_flow", 968),
        ("lava_still", 969),
        ("lava_flow", 970),
    ]
}

fn item_texture_name_candidates(item: ItemId) -> Option<&'static [&'static str]> {
    let names = match item {
        ItemId::STICK => &["stick"][..],
        ItemId::WOODEN_PICKAXE => &["wooden_pickaxe"],
        ItemId::WOODEN_SWORD => &["wooden_sword"],
        ItemId::STONE_PICKAXE => &["stone_pickaxe"],
        ItemId::STONE_SWORD => &["stone_sword"],
        ItemId::STONE_SHOVEL => &["stone_shovel"],
        ItemId::STONE_AXE => &["stone_axe"],
        ItemId::IRON_PICKAXE => &["iron_pickaxe"],
        ItemId::IRON_SWORD => &["iron_sword"],
        ItemId::IRON_SHOVEL => &["iron_shovel"],
        ItemId::IRON_AXE => &["iron_axe"],
        ItemId::DIAMOND_PICKAXE => &["diamond_pickaxe"],
        ItemId::DIAMOND_SWORD => &["diamond_sword"],
        ItemId::DIAMOND_SHOVEL => &["diamond_shovel"],
        ItemId::DIAMOND_AXE => &["diamond_axe"],
        ItemId::WOODEN_SHOVEL => &["wooden_shovel"],
        ItemId::WOODEN_AXE => &["wooden_axe"],
        ItemId::WOODEN_HOE => &["wooden_hoe"],
        ItemId::STONE_HOE => &["stone_hoe"],
        ItemId::IRON_HOE => &["iron_hoe"],
        ItemId::DIAMOND_HOE => &["diamond_hoe"],
        ItemId::IRON_INGOT => &["iron_ingot"],
        ItemId::GOLD_INGOT => &["gold_ingot"],
        ItemId::DIAMOND_GEM => &["diamond"],
        ItemId::COAL => &["coal"],
        ItemId::WHEAT_ITEM => &["wheat"],
        ItemId::WHEAT_SEEDS => &["wheat_seeds"],
        ItemId::BREAD => &["bread"],
        ItemId::COPPER_INGOT => &["copper_ingot"],
        ItemId::IRON_HELMET => &["iron_helmet"],
        ItemId::IRON_CHESTPLATE => &["iron_chestplate"],
        ItemId::IRON_LEGGINGS => &["iron_leggings"],
        ItemId::IRON_BOOTS => &["iron_boots"],
        ItemId::DIAMOND_HELMET => &["diamond_helmet"],
        ItemId::DIAMOND_CHESTPLATE => &["diamond_chestplate"],
        ItemId::DIAMOND_LEGGINGS => &["diamond_leggings"],
        ItemId::DIAMOND_BOOTS => &["diamond_boots"],
        ItemId::BONE_MEAL => &["bone_meal"],
        ItemId::EMPTY_BUCKET => &["bucket"],
        ItemId::WATER_BUCKET => &["water_bucket"],
        ItemId::LAVA_BUCKET => &["lava_bucket"],
        ItemId::FLINT_AND_STEEL => &["flint_and_steel"],
        ItemId::SHEARS_ITEM => &["shears"],
        ItemId::BOW => &["bow"],
        ItemId::ARROW => &["arrow"],
        ItemId::LEATHER => &["leather"],
        ItemId::FEATHER => &["feather"],
        ItemId::BONE_ITEM => &["bone"],
        ItemId::STRING => &["string"],
        ItemId::RAW_BEEF => &["beef"],
        ItemId::COOKED_BEEF => &["cooked_beef"],
        ItemId::RAW_PORKCHOP => &["porkchop"],
        ItemId::COOKED_PORKCHOP => &["cooked_porkchop"],
        ItemId::RAW_CHICKEN => &["chicken"],
        ItemId::COOKED_CHICKEN => &["cooked_chicken"],
        ItemId::RAW_MUTTON => &["mutton"],
        ItemId::COOKED_MUTTON => &["cooked_mutton"],
        ItemId::LEATHER_HELMET => &["leather_helmet"],
        ItemId::LEATHER_CHESTPLATE => &["leather_chestplate"],
        ItemId::LEATHER_LEGGINGS => &["leather_leggings"],
        ItemId::LEATHER_BOOTS => &["leather_boots"],
        ItemId::GOLD_PICKAXE => &["golden_pickaxe"],
        ItemId::GOLD_SWORD => &["golden_sword"],
        ItemId::GOLD_SHOVEL => &["golden_shovel"],
        ItemId::GOLD_AXE => &["golden_axe"],
        ItemId::GOLD_HOE => &["golden_hoe"],
        ItemId::GOLD_HELMET => &["golden_helmet"],
        ItemId::GOLD_CHESTPLATE => &["golden_chestplate"],
        ItemId::GOLD_LEGGINGS => &["golden_leggings"],
        ItemId::GOLD_BOOTS => &["golden_boots"],
        _ => return None,
    };
    Some(names)
}

fn blit_block_tile(atlas: &mut RgbaImage, block_id: BlockId, tile: &RgbaImage) {
    blit_tile_at_slot(atlas, u32::from(block_id.0), tile);
}

fn blit_tile_at_slot(atlas: &mut RgbaImage, slot: u32, tile: &RgbaImage) {
    let x = (slot % TILES_PER_ROW) * TILE_SIZE;
    let y = (slot / TILES_PER_ROW) * TILE_SIZE;

    for ty in 0..TILE_SIZE {
        for tx in 0..TILE_SIZE {
            let pixel = *tile.get_pixel(tx, ty);
            atlas.put_pixel(x + tx, y + ty, pixel);
        }
    }
}
