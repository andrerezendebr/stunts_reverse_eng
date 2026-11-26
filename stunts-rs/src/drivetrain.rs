//! Verified stages of update_car_speed; this is not the complete speed update.
use crate::car::Car;

pub const STATE_SIZE: usize = 0xd0;
pub const RPM: usize = 0x22;
pub const LAST_RPM: usize = 0x24;
pub const SPEED_DIFF: usize = 0x28;
pub const SPEED2: usize = 0x2c;
pub const LAST_SPEED: usize = 0x2e;
pub const GEAR_RATIO: usize = 0x30;
pub const RATIO_HIGH: usize = 0x32;
pub const KNOB_X: usize = 0x34;
pub const KNOB_Y: usize = 0x38;
pub const TARGET_X: usize = 0x3a;
pub const TARGET_Y: usize = 0x3c;
pub const GEAR: usize = 0xbe;
pub const REAR_CONTACT: usize = 0xc0;
pub const LIMITER: usize = 0xc6;
pub const CHANGING: usize = 0xca;
pub const SHIFT_TIMER: usize = 0xcb;
pub const TRANSMISSION: usize = 0xcc;

/// Keep the whole packed CARSTATE so a staged port preserves unmapped fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State(pub [u8; STATE_SIZE]);

impl State {
    pub fn word(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.0[offset], self.0[offset + 1]])
    }
    pub fn set_word(&mut self, offset: usize, value: u16) {
        self.0[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn move_knob(&mut self, offset: usize, target: i16, step: i16) {
        let current = self.word(offset) as i16;
        let difference = target.wrapping_sub(current);
        // DOS ABS preserves 0x8000; keep its signed comparison for edge cases.
        let next = if difference.wrapping_abs() <= step {
            target
        } else if difference > 0 {
            current.wrapping_add(step)
        } else {
            current.wrapping_sub(step)
        };
        self.set_word(offset, next as u16);
    }
}

/// Execute only the entry-to-17CAC prefix: history, shift selection and lever
/// movement. RPM/speed integration after that boundary is deliberately separate.
pub fn gearbox_tick(state: &mut State, car: &Car<'_>, input: u8, fps: u16) -> Result<(), String> {
    if state.0[GEAR] > car.gears() || car.gears() > 6 {
        return Err("Gear lies outside the mapped car parameters".into());
    }
    let step = if fps == 20 { 6 } else { 12 };
    if state.0[LIMITER] != 0 {
        state.0[LIMITER] -= 1;
    }
    state.set_word(
        SPEED_DIFF,
        state.word(SPEED2).wrapping_sub(state.word(LAST_SPEED)),
    );
    state.set_word(LAST_SPEED, state.word(SPEED2));
    state.set_word(LAST_RPM, state.word(RPM));

    let manual = state.0[TRANSMISSION] == 0 && state.0[CHANGING] == 0;
    let direction = if manual {
        // Upshift takes precedence if both request bits are present.
        if input & 0x10 != 0 {
            1
        } else if input & 0x20 != 0 {
            -1
        } else {
            0
        }
    } else if state.0[GEAR] != 0 && state.0[CHANGING] == 0 && state.0[REAR_CONTACT] != 0 {
        if state.word(RPM) > car.upshift_rpm() as u16 {
            1
        } else if state.word(RPM) < car.downshift_rpm() as u16 {
            -1
        } else {
            0
        }
    } else {
        0
    };
    let next_gear = if direction > 0 && state.0[GEAR] != car.gears() {
        state.0[GEAR] + 1
    } else if direction < 0 && state.0[GEAR] > 1 {
        state.0[GEAR] - 1
    } else {
        state.0[GEAR]
    };
    if next_gear != state.0[GEAR] {
        state.0[GEAR] = next_gear;
        state.0[CHANGING] = 1;
        // Only AL participates in the addition after sign extension/SAR.
        let low = fps as u8;
        state.0[SHIFT_TIMER] = (((low as i8 as i16) >> 1) as u8).wrapping_add(low);
        let (x, y) = car.knob_point(next_gear)?;
        state.set_word(TARGET_X, x as u16);
        state.set_word(TARGET_Y, y as u16);
    }
    if state.0[CHANGING] == 0 {
        if state.0[SHIFT_TIMER] != 0 {
            state.0[SHIFT_TIMER] -= 1;
        }
    } else if state.word(KNOB_X) == state.word(TARGET_X) {
        if state.word(KNOB_Y) == state.word(TARGET_Y) {
            // The selected ratio changes only once the lever has already arrived.
            state.0[CHANGING] = 0;
            let ratio = car.gear_ratios()[usize::from(state.0[GEAR])];
            state.set_word(GEAR_RATIO, ratio);
            state.set_word(RATIO_HIGH, ratio >> 8);
        } else {
            state.move_knob(KNOB_Y, state.word(TARGET_Y) as i16, step);
        }
    } else {
        // H-pattern motion first reaches neutral Y, then moves horizontally.
        let (_, neutral_y) = car.knob_point(0)?;
        if state.word(KNOB_Y) as i16 == neutral_y {
            state.move_knob(KNOB_X, state.word(TARGET_X) as i16, step);
        } else {
            state.move_knob(KNOB_Y, neutral_y, step);
        }
    }
    Ok(())
}

/// Complete unsigned RPM helper at 19DC:0006. MUL returns the high product word;
/// shifting keeps the old RPM, and both paths enforce the idle lower bound.
pub fn rpm_from_speed(current: u16, speed: u16, ratio: u16, changing: u16, idle: u16) -> u16 {
    let rpm = if changing == 0 {
        ((u32::from(speed) * u32::from(ratio)) >> 16) as u16
    } else {
        current
    };
    rpm.max(idle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_state(hex: &str) -> State {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        State(bytes.try_into().unwrap())
    }

    #[test]
    fn gearbox_prefix_matches_every_byte_of_original_carstate() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for line in include_str!("../tests/fixtures/gearbox-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let bytes = std::fs::read(directory.join(fields[0])).unwrap();
            let car = Car::parse(&bytes).unwrap();
            let mut state = decode_state(fields[3]);
            gearbox_tick(
                &mut state,
                &car,
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
            )
            .unwrap();
            assert_eq!(
                state,
                decode_state(fields[4]),
                "car={}, input={}, fps={}, case={count}",
                fields[0],
                fields[1],
                fields[2]
            );
            count += 1;
        }
        assert_eq!(count, 704);
    }

    #[test]
    fn rpm_matches_unsigned_original_helper() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/rpm-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<u16> = line.split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(
                rpm_from_speed(fields[0], fields[1], fields[2], fields[3], fields[4]),
                fields[5],
                "{line}"
            );
            count += 1;
        }
        assert_eq!(count, 520);
    }

    #[test]
    fn torque_selection_matches_original_block() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for line in include_str!("../tests/fixtures/torque-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let bytes = std::fs::read(directory.join(fields[0])).unwrap();
            let car = Car::parse(&bytes).unwrap();
            assert_eq!(
                car.selected_torque(
                    fields[1].parse().unwrap(),
                    fields[2].parse().unwrap(),
                    fields[3].parse().unwrap()
                )
                .unwrap(),
                fields[4].parse::<u8>().unwrap(),
                "{line}"
            );
            count += 1;
        }
        assert_eq!(count, 1540);
    }

    #[test]
    fn rejects_unmapped_gear_and_torque_indices() {
        let car = Car::parse(include_bytes!("../../stunts_dos/CARPMIN.RES")).unwrap();
        assert!(car.selected_torque(6, 3000, 0).is_err());
        assert!(car.selected_torque(2, 13312, 0).is_err());
        let mut state = State([0; STATE_SIZE]);
        state.0[GEAR] = 6;
        let before = state.clone();
        assert!(gearbox_tick(&mut state, &car, 16, 20).is_err());
        assert_eq!(state, before);
    }
}
