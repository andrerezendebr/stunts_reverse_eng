//! Shared race ownership and resource loading, independent of graphics backend.
use crate::{
    car::Car, compression, frontend::Catalog, prototype::World, resource::Archive, scene::Scene,
    shape::Shape, track::Track,
};
use std::path::Path;

pub struct Session {
    pub world: World,
    pub scene: Scene,
    pub track_bytes: Vec<u8>,
    pub car_bytes: Vec<u8>,
    pub shape: Shape,
}
impl Session {
    /// Finish loading a replacement before the UI swaps out its active race.
    pub fn load(
        root: &Path,
        catalog: &Catalog,
        track_index: usize,
        car_index: usize,
    ) -> Result<Self, String> {
        let track_bytes = catalog
            .tracks
            .get(track_index)
            .ok_or("Invalid track selection")?
            .bytes
            .clone();
        let track = Track::parse(&track_bytes)?;
        let choice = catalog.cars.get(car_index).ok_or("Invalid car selection")?;
        let car_bytes = std::fs::read(root.join(&choice.file)).map_err(|e| e.to_string())?;
        let car = Car::parse(&car_bytes)?;
        let model =
            compression::decompress(&std::fs::read(&choice.model).map_err(|e| e.to_string())?)?;
        let archive = Archive::parse(&model)?;
        let shape = Shape::parse(archive.get(b"car0").ok_or("Missing car0 mesh")?)?;
        let mut world = World::new(&car, &track)?;
        let scene = Scene::load(root, &track)?;
        world.place_on_scene(&scene);
        Ok(Self {
            world,
            scene,
            track_bytes,
            car_bytes,
            shape,
        })
    }
    pub fn tick(&mut self, input: u8) -> Result<(), String> {
        self.world.tick_scene(
            &Car::parse(&self.car_bytes)?,
            &Track::parse(&self.track_bytes)?,
            &self.scene,
            input,
        )
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.world.reset(&Car::parse(&self.car_bytes)?)
    }
}
