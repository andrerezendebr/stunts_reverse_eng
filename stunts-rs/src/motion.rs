//! First bounded prefix of update_player_state, not complete world motion.
use crate::{
    drivetrain::{State, ALL_CONTACT, SPEED2},
    matrix,
};
#[derive(Debug)]
pub struct Preparation {
    pub world_position: [i32; 3],
    pub rotation: matrix::Matrix,
    pub speed_scaled: u16,
    pub front_quarter: i16,
}

/// Copy previous position and calculate the original preparation quantities.
/// Stops at DOS 14ECE; no wheel/contact search or position integration occurs.
pub fn prepare(state: &mut State, fps: u16) -> Preparation {
    let world_position =
        std::array::from_fn(|i| i32::from_le_bytes(state.0[i * 4..i * 4 + 4].try_into().unwrap()));
    state.0.copy_within(0..12, 12);
    let heading = state.word(0x18);
    let pitch = state.word(0x1a);
    let roll = state.word(0x1c);
    let rotation = matrix::rotation_zxy(
        roll.wrapping_neg(),
        pitch.wrapping_neg(),
        heading.wrapping_neg(),
        0,
    );
    let gravity = if pitch != 0 || roll != 0 {
        matrix::transform(&rotation, &[0, 0, 130])[1].wrapping_neg()
    } else {
        0
    };
    state.set_word(0x1e, gravity as u16);
    Preparation {
        world_position,
        rotation,
        speed_scaled: ((u32::from(state.word(SPEED2)) * 0x580)
            / if fps == 10 { 0x1e00 } else { 0x3c00 }) as u16,
        front_quarter: if state.0[ALL_CONTACT] != 0 {
            (state.word(0x40) as i16) >> 2
        } else {
            0
        },
    }
}

/// Original wheel prediction through DOS 1513E, before terrain/contact queries.
/// `base` is the rotated attachment position; `predicted` also includes travel.
/// This does not commit wheel positions or advance the body's world position.
pub struct WheelPrediction {
    pub base: [[i32; 3]; 4],
    pub predicted: [[i32; 3]; 4],
    pub upside_down_adjustment: i16,
}

pub fn predict_wheels(state: &mut State, car: &crate::car::Car<'_>, fps: u16) -> WheelPrediction {
    let prep = prepare(state, fps);
    let angle = state.word(0x3e);
    let skid = matrix::rotation_zxy(0, 0, angle.wrapping_neg(), 0);
    let inverted =
        state.0[ALL_CONTACT] != 0 && matrix::transform(&prep.rotation, &[0, 30000, 0])[1] < 0;
    let adjustment = if inverted {
        if state.word(SPEED2) > 0x1e00 {
            192
        } else {
            -192
        }
    } else {
        0
    };
    let attachments = car.wheel_coordinates();
    let mut base = [[0; 3]; 4];
    let mut predicted = base;
    for i in 0..4 {
        let mut local = attachments[i];
        local[1] = (state.word(0x54 + i * 2) as i16)
            .wrapping_add(384)
            .wrapping_neg();
        if adjustment < 0 {
            local[1] = local[1].wrapping_sub(adjustment);
        }
        if angle & 1023 != 0 {
            local = matrix::transform(&skid, &local);
        }
        let offset = matrix::transform(&prep.rotation, &local);
        base[i] =
            std::array::from_fn(|a| prep.world_position[a].wrapping_add(i32::from(offset[a])));
        predicted[i] = base[i];
        if prep.speed_scaled != 0 {
            let mut wheel_angle = state.word(0x36);
            if i < 2 {
                wheel_angle = wheel_angle.wrapping_sub(prep.front_quarter as u16);
            }
            let travel = [0, 0, prep.speed_scaled as i16];
            // planindex_copy is -1 throughout this prefix. The plane-aware
            // branch of plane_rotate_op is only needed after contact search.
            let travel = if wheel_angle != 0 {
                matrix::transform(&matrix::rotation_y(wheel_angle.wrapping_neg()), &travel)
            } else {
                travel
            };
            let travel = matrix::transform(&prep.rotation, &travel);
            predicted[i] = std::array::from_fn(|a| base[i][a].wrapping_add(i32::from(travel[a])));
        }
    }
    WheelPrediction {
        base,
        predicted,
        upside_down_adjustment: adjustment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(hex: &str) -> State {
        State(
            hex.as_bytes()
                .chunks_exact(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
    }
    #[test]
    fn wheel_prediction_matches_original_for_all_eleven_cars() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut total = 0;
        for line in include_str!("../tests/fixtures/wheel-prediction-dos.csv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = line.split(',').collect();
            let bytes = std::fs::read(root.join(f[0])).unwrap();
            let car = crate::car::Car::parse(&bytes).unwrap();
            let mut actual = state(f[2]);
            let result = predict_wheels(&mut actual, &car, f[1].parse().unwrap());
            assert_eq!(actual, state(f[3]), "state case {total}");
            let parse = |field: &str| {
                field
                    .split(';')
                    .map(|n| n.parse::<i32>().unwrap())
                    .collect::<Vec<_>>()
            };
            assert_eq!(result.base.concat(), parse(f[4]), "base case {total}");
            assert_eq!(
                result.predicted.concat(),
                parse(f[5]),
                "predicted case {total}"
            );
            assert_eq!(result.upside_down_adjustment, f[6].parse::<i16>().unwrap());
            total += 1;
        }
        assert_eq!(total, 704);
    }
    #[test]
    fn preparation_matches_original_state_writes_and_stack_locals() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/motion-prefix-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let mut actual = state(fields[1]);
            let result = prepare(&mut actual, fields[0].parse().unwrap());
            assert_eq!(actual, state(fields[2]), "case={count}");
            assert_eq!(result.speed_scaled, fields[3].parse::<u16>().unwrap());
            assert_eq!(result.front_quarter, fields[4].parse::<i16>().unwrap());
            count += 1;
        }
        assert_eq!(count, 512);
    }
}
