#![allow(dead_code)]
#![allow(unused_imports)]

mod apps;
mod models;
use apps::P9SceneTool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = P9SceneTool::new();
    scene.run()
}