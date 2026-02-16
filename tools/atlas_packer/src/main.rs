use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AtlasManifest {
    output: String,
    textures: Vec<String>,
}

fn main() {
    let manifest_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "manifest.toml".to_string());

    if let Err(err) = run(Path::new(&manifest_path)) {
        eprintln!("atlas_packer error: {err}");
        std::process::exit(1);
    }
}

fn run(manifest_path: &Path) -> Result<(), String> {
    let manifest_src = fs::read_to_string(manifest_path)
        .map_err(|err| format!("failed to read {}: {err}", manifest_path.display()))?;

    let manifest: AtlasManifest = toml::from_str(&manifest_src)
        .map_err(|err| format!("failed to parse {}: {err}", manifest_path.display()))?;

    let mut loaded = 0usize;
    let mut max_width = 0u32;
    let mut max_height = 0u32;

    for rel_path in &manifest.textures {
        let path = PathBuf::from(rel_path);
        match image::open(&path) {
            Ok(img) => {
                loaded += 1;
                max_width = max_width.max(img.width());
                max_height = max_height.max(img.height());
            }
            Err(err) => {
                return Err(format!("failed to load {}: {err}", path.display()));
            }
        }
    }

    println!(
        "Stub atlas pack: {} textures -> {} (largest tile {}x{})",
        loaded, manifest.output, max_width, max_height
    );

    Ok(())
}
