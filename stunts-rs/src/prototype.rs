//! Experimental world simulation. These floating-point equations are new code,
//! not a reverse-engineered replacement for DOS update_player_state.
use crate::{
    car::Car,
    drivetrain::{self, State},
    grip,
    scene::{self, Contact, Scene},
    simulation,
    track::Track,
};

pub const TILE: f32 = 60.0;
pub const STEP: f32 = 0.05;

pub struct World {
    pub state: State,
    pub position: [f32; 3],
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    pub compression: [f32; 4],
    pub contacts: usize,
    pub collisions: u32,
    pub top_speed: u16,
    /// Ground travel for visual wheel rotation; advances only with simulation.
    pub wheel_travel: f32,
    vertical_speed: f32,
    spawn: [f32; 3],
    spawn_heading: f32,
}

impl World {
    pub fn new(car: &Car<'_>, track: &Track<'_>) -> Result<Self, String> {
        let mut spawn = [TILE * 15.5, 0.7, TILE * 15.5];
        let mut heading = 0.0;
        'find_start: for y in 0..30 {
            for x in 0..30 {
                if let Some(angle) = scene::start_heading(track.element(x, y).unwrap()) {
                    spawn = [(x as f32 + 0.5) * TILE, 0.7, (y as f32 + 0.5) * TILE];
                    heading = angle;
                    break 'find_start;
                }
            }
        }
        Ok(Self {
            state: simulation::initial_state(car, 1)?,
            position: spawn,
            heading,
            pitch: 0.0,
            roll: 0.0,
            compression: [0.0; 4],
            contacts: 4,
            collisions: 0,
            top_speed: 0,
            wheel_travel: 0.0,
            vertical_speed: 0.0,
            spawn,
            spawn_heading: heading,
        })
    }

    pub fn reset(&mut self, car: &Car<'_>) -> Result<(), String> {
        self.state = simulation::initial_state(car, 1)?;
        self.position = self.spawn;
        self.wheel_travel = 0.0;
        self.heading = self.spawn_heading;
        self.vertical_speed = 0.0;
        self.pitch = 0.0;
        self.roll = 0.0;
        self.compression = [0.0; 4];
        self.contacts = 4;
        Ok(())
    }

    /// Adjust the saved start height to the loaded original track geometry.
    pub fn place_on_scene(&mut self, scene: &Scene) {
        self.spawn[1] = scene
            .contact(self.spawn[0], self.spawn[2], f32::INFINITY)
            .height
            + 0.7;
        self.position = self.spawn;
    }

    /// Flat-plane fallback retained for control and solver regression tests.
    pub fn tick(&mut self, car: &Car<'_>, track: &Track<'_>, input: u8) -> Result<(), String> {
        self.step(car, track, None, input)
    }

    /// Experimental contacts against original scene triangles, not DOS planes.
    pub fn tick_scene(
        &mut self,
        car: &Car<'_>,
        track: &Track<'_>,
        scene: &Scene,
        input: u8,
    ) -> Result<(), String> {
        self.step(car, track, Some(scene), input)
    }

    fn step(
        &mut self,
        car: &Car<'_>,
        track: &Track<'_>,
        scene: Option<&Scene>,
        input: u8,
    ) -> Result<(), String> {
        let x = (self.position[0] / TILE).clamp(0.0, 29.0) as usize;
        let z = (self.position[2] / TILE).clamp(0.0, 29.0) as usize;
        for i in 0..4 {
            let (_, hit) = self.wheel_contact(i, scene);
            self.state.0[grip::SURFACES + i] = hit.surface;
        }
        // Keep verified grip map indices in bounds. DOS positions use a tile
        // index in the high byte of each 32-bit horizontal coordinate.
        self.state.0[2] = x as u8;
        self.state.0[10] = z as u8;
        let mut elements = [0; 900];
        for row in 0..30 {
            for col in 0..30 {
                elements[row * 30 + col] = track.element(col, row).unwrap();
            }
        }
        simulation::control_tick(
            &mut self.state,
            car,
            input,
            20,
            &elements,
            &mut self.top_speed,
        )?;
        // Speed-to-world scaling and bicycle steering are provisional, not
        // recovered DOS unit conversions. Wheelbase is expressed in metres.
        let speed = self.state.word(drivetrain::SPEED2) as f32 / 256.0 * 0.44704;
        let steer = self.state.word(grip::STEERING) as i16 as f32 / 240.0 * 0.48;
        if self.contacts > 0 {
            self.heading += speed / 2.7 * steer.tan() * STEP;
        }
        let previous_position = self.position;
        self.position[0] += self.heading.sin() * speed * STEP;
        self.position[2] -= self.heading.cos() * speed * STEP;
        if scene.is_some_and(|s| s.obstructed(previous_position, self.position)) {
            self.position = previous_position;
            self.collisions += 1;
            self.state.set_word(drivetrain::SPEED2, 0);
            self.state.set_word(drivetrain::SPEED, 0);
        }

        // Four independent spring/damper contacts support the body. Pitch and
        // roll follow load transfer; they do not implement original rigid-body physics.
        self.contacts = 0;
        let mut front_contacts = 0;
        let mut rear_contacts = 0;
        let mut heights = [0.0; 4];
        let mut force = -9.81;
        for i in 0..4 {
            let front = if i < 2 { -1.3 } else { 1.3 };
            let side = if i % 2 == 0 { -0.75 } else { 0.75 };
            let (_, hit) = self.wheel_contact(i, scene);
            heights[i] = hit.height;
            let height =
                self.position[1] + self.pitch.sin() * front + self.roll.sin() * side - hit.height;
            self.compression[i] = (0.8 - height).clamp(0.0, 0.6);
            if self.compression[i] > 0.0 {
                self.contacts += 1;
                if i < 2 {
                    front_contacts += 1;
                } else {
                    rear_contacts += 1;
                }
                force += (95.0 * self.compression[i] - 6.0 * self.vertical_speed).max(0.0) / 4.0;
            }
        }
        self.vertical_speed += force * STEP;
        self.position[1] += self.vertical_speed * STEP;
        let floor = heights.iter().copied().fold(0.0, f32::max) + 0.25;
        if self.position[1] < floor {
            self.position[1] = floor;
            self.vertical_speed = self.vertical_speed.max(0.0);
        }
        let ground_pitch = ((heights[2] + heights[3] - heights[0] - heights[1]) / 5.2).atan();
        let ground_roll = ((heights[1] + heights[3] - heights[0] - heights[2]) / 3.0).atan();
        if self.contacts > 0 {
            self.roll += (ground_roll - steer * speed * 0.008 - self.roll) * 0.15;
            self.pitch += ((ground_pitch
                + if input & 2 != 0 {
                    0.035
                } else if input & 1 != 0 {
                    -0.02
                } else {
                    0.0
                })
                - self.pitch)
                * 0.12;
        }
        self.state.0[0xbf] = front_contacts;
        self.state.0[0xc0] = rear_contacts;
        self.state.0[0xc1] = self.contacts as u8;

        // Swept endpoint clamping is sufficient for the enclosing axis-aligned
        // boundary, independently of the experimental scene mesh sweep above.
        let edge = 30.0 * TILE - 1.5;
        let before = [self.position[0], self.position[2]];
        self.position[0] = self.position[0].clamp(1.5, edge);
        self.position[2] = self.position[2].clamp(1.5, edge);
        if before != [self.position[0], self.position[2]] {
            self.collisions += 1;
            self.state.set_word(drivetrain::SPEED2, 0);
            self.state.set_word(drivetrain::SPEED, 0);
        }
        self.wheel_travel += (self.position[0] - previous_position[0])
            .hypot(self.position[2] - previous_position[2]);
        Ok(())
    }

    fn wheel_contact(&self, index: usize, scene: Option<&Scene>) -> ([f32; 2], Contact) {
        let front = if index < 2 { -1.3 } else { 1.3 };
        let side = if index % 2 == 0 { -0.75 } else { 0.75 };
        let x = self.position[0] + side * self.heading.cos() - front * self.heading.sin();
        let z = self.position[2] + side * self.heading.sin() + front * self.heading.cos();
        // Reach grows with one tick of travel so fast cars can follow a slope
        // instead of slipping underneath it. A distant bridge remains excluded.
        let reach = 0.85 + self.state.word(drivetrain::SPEED2) as f32 / 256.0 * 0.44704 * STEP;
        let hit = scene
            .map(|s| s.contact(x, z, self.position[1] + reach))
            .unwrap_or(Contact {
                height: 0.0,
                surface: 0,
            });
        ([x, z], hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suspension_follows_an_original_inclined_road_at_speed() {
        let car = Car::parse(include_bytes!("../../stunts_dos/CARPMIN.RES")).unwrap();
        let mut bytes = [0u8; 1802];
        bytes[15 * 30 + 15] = 4;
        bytes[901 + (29 - 15) * 30 + 15] = 7;
        let track = Track::parse(&bytes).unwrap();
        let scene = Scene::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos"),
            &track,
        )
        .unwrap();
        let mut world = World::new(&car, &track).unwrap();
        world.position = [15.5 * TILE, 0.0, 15.0 * TILE + 10.0];
        world.position[1] = scene
            .contact(world.position[0], world.position[2], 100.0)
            .height
            + 0.7;
        world.heading = std::f32::consts::PI;
        world.state.set_word(drivetrain::SPEED2, 20000);
        world.state.set_word(drivetrain::SPEED, 20000);
        let height = world.position[1];
        for _ in 0..18 {
            world.tick_scene(&car, &track, &scene, 1).unwrap();
        }
        assert!(
            world.position[1] > height + 8.0,
            "position {:?}, start height {height}",
            world.position
        );
        assert!(world.pitch < 0.0);
        assert_eq!(world.collisions, 0);
    }
    #[test]
    fn original_straight_track_has_no_spurious_walls_and_start_direction_is_preserved() {
        let car = Car::parse(include_bytes!("../../stunts_dos/CARPMIN.RES")).unwrap();
        let mut bytes = [0u8; 1802];
        for z in 0..30 {
            bytes[z * 30 + 15] = 4;
        }
        bytes[10 * 30 + 15] = 1;
        let track = Track::parse(&bytes).unwrap();
        let scene = Scene::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos"),
            &track,
        )
        .unwrap();
        let mut world = World::new(&car, &track).unwrap();
        world.place_on_scene(&scene);
        let start = world.position;
        for _ in 0..120 {
            world.tick_scene(&car, &track, &scene, 1).unwrap();
        }
        assert!(world.position[2] > start[2] + 60.0);
        assert_eq!(world.collisions, 0);
        assert_eq!(world.contacts, 4);
        assert!(world.state.0[grip::SURFACES..grip::SURFACES + 4]
            .iter()
            .all(|&s| s == 1));
        bytes[10 * 30 + 15] = 0xb4;
        let track = Track::parse(&bytes).unwrap();
        let mut world = World::new(&car, &track).unwrap();
        let heading = world.heading;
        world.heading += 1.0;
        world.reset(&car).unwrap();
        assert_eq!(world.heading, heading);
        assert_ne!(heading, 0.0);
    }
    #[test]
    fn driving_settles_and_boundary_collision_stops_the_car() {
        let car = Car::parse(include_bytes!("../../stunts_dos/CARPMIN.RES")).unwrap();
        let bytes = [0u8; 1802];
        let track = Track::parse(&bytes).unwrap();
        let mut world = World::new(&car, &track).unwrap();
        let start = world.position;
        for _ in 0..400 {
            world.tick(&car, &track, 1).unwrap();
        }
        assert!(world.position[2] < start[2]);
        assert!(world.position.iter().all(|n| n.is_finite()));
        assert!((0.25..0.9).contains(&world.position[1]));
        world.position[2] = 1.5;
        world.state.set_word(drivetrain::SPEED2, 10000);
        world.state.set_word(drivetrain::SPEED, 10000);
        world.tick(&car, &track, 1).unwrap();
        assert!(world.collisions > 0);
        assert_eq!(world.state.word(drivetrain::SPEED2), 0);
        world.reset(&car).unwrap();
        assert_eq!(world.position, start);
    }
}
