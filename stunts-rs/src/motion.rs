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
