use std::env;
use std::path::Path;

use veldspar_persist::region::RegionFile;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: region_inspector <path/to/file.vsr>");
        std::process::exit(2);
    };

    if let Err(err) = run(Path::new(&path)) {
        eprintln!("region_inspector error: {err}");
        std::process::exit(1);
    }
}

fn run(path: &Path) -> Result<(), String> {
    let region = RegionFile::open(path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;

    println!("Region: {}", path.display());
    println!("Magic: {:?}", RegionFile::MAGIC);
    println!("Chunk count: {}", region.chunk_count());

    for pos in region.chunk_positions() {
        println!("  chunk @ ({}, {}, {})", pos.x, pos.y, pos.z);
    }

    Ok(())
}
