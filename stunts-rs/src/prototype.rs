//! Experimental world simulation. These floating-point equations are new code,
//! not a reverse-engineered replacement for DOS update_player_state.
use crate::{
    car::Car,
    drivetrain::{self, State},
    grip, simulation,
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
    vertical_speed: f32,
    spawn: [f32; 3],
}

impl World {
    pub fn new(car: &Car<'_>, track: &Track<'_>) -> Result<Self, String> {
        let mut spawn = [TILE * 15.5, 0.7, TILE * 15.5];
        // Start at the first original start tile; unsupported start orientations
        // are deliberately not inferred from unrelated element IDs.
        for y in 0..30 {
            for x in 0..30 {
                if track.element(x, y) == Some(1) {
                    spawn = [(x as f32 + 0.5) * TILE, 0.7, (y as f32 + 0.5) * TILE];
                }
            }
        }
        Ok(Self {
            state: simulation::initial_state(car, 1)?,
            position: spawn,
            heading: 0.0,
            pitch: 0.0,
            roll: 0.0,
            compression: [0.0; 4],
            contacts: 4,
            collisions: 0,
            top_speed: 0,
            vertical_speed: 0.0,
            spawn,
        })
    }

    pub fn reset(&mut self, car: &Car<'_>) -> Result<(), String> {
        self.state = simulation::initial_state(car, 1)?;
        self.position = self.spawn;
        self.heading = 0.0;
        self.vertical_speed = 0.0;
        self.pitch = 0.0;
        self.roll = 0.0;
        self.compression = [0.0; 4];
        self.contacts = 4;
        Ok(())
    }

    /// The prototype treats terrain as flat and road tiles as square patches.
    /// Ramps, loops, bridges and original collision planes are not decoded here.
    pub fn tick(&mut self, car: &Car<'_>, track: &Track<'_>, input: u8) -> Result<(), String> {
        let x = (self.position[0] / TILE).clamp(0.0, 29.0) as usize;
        let z = (self.position[2] / TILE).clamp(0.0, 29.0) as usize;
        let surface = if track.element(x, z).unwrap_or(0) == 0 {
            0
        } else {
            1
        };
        self.state.0[grip::SURFACES..grip::SURFACES + 4].fill(surface);
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
        self.heading += speed / 2.7 * steer.tan() * STEP;
        self.position[0] += self.heading.sin() * speed * STEP;
        self.position[2] -= self.heading.cos() * speed * STEP;

        // Four independent spring/damper contacts support the body. Pitch and
        // roll follow load transfer; they do not implement original rigid-body physics.
        self.contacts = 0;
        let mut force = -9.81;
        for (i, compression) in self.compression.iter_mut().enumerate() {
            let front = if i < 2 { -1.3 } else { 1.3 };
            let side = if i % 2 == 0 { -0.75 } else { 0.75 };
            let height = self.position[1] + self.pitch.sin() * front + self.roll.sin() * side;
            *compression = (0.8 - height).clamp(0.0, 0.6);
            if *compression > 0.0 {
                self.contacts += 1;
                force += (95.0 * *compression - 6.0 * self.vertical_speed).max(0.0) / 4.0;
            }
        }
        self.vertical_speed += force * STEP;
        self.position[1] += self.vertical_speed * STEP;
        if self.position[1] < 0.25 {
            self.position[1] = 0.25;
            self.vertical_speed = self.vertical_speed.max(0.0);
        }
        self.roll += (-steer * speed * 0.008 - self.roll) * 0.15;
        self.pitch += ((if input & 2 != 0 {
            0.035
        } else if input & 1 != 0 {
            -0.02
        } else {
            0.0
        }) - self.pitch)
            * 0.12;
        self.state.0[0xbf] = self.contacts.min(2) as u8;
        self.state.0[0xc0] = self.contacts.saturating_sub(2) as u8;
        self.state.0[0xc1] = self.contacts as u8;

        // Swept endpoint clamping is sufficient for the enclosing axis-aligned
        // boundary. It is not collision handling for original track objects.
        let edge = 30.0 * TILE - 1.5;
        let before = [self.position[0], self.position[2]];
        self.position[0] = self.position[0].clamp(1.5, edge);
        self.position[2] = self.position[2].clamp(1.5, edge);
        if before != [self.position[0], self.position[2]] {
            self.collisions += 1;
            self.state.set_word(drivetrain::SPEED2, 0);
            self.state.set_word(drivetrain::SPEED, 0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
