mod app;
mod camera;
mod input;
pub mod mesh_worker;
mod net;
pub mod persistence;
mod player;
mod renderer;
mod ui;
mod world;

fn main() {
    app::run();
}
