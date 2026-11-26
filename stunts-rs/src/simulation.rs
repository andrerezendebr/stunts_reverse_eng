//! Integrate the three verified control kernels, without world/suspension updates.
use crate::{
    car::Car,
    drivetrain::{self, State},
    grip, steering,
};

pub fn initial_state(car: &Car<'_>, surface: u8) -> Result<State, String> {
    car.surface_factor(surface)?;
    let mut state = drivetrain::stationary_state(car)?;
    state.0[2] = 3;
    state.0[10] = 4;
    state.0[grip::SURFACES..grip::SURFACES + 4].fill(surface);
    Ok(state)
}

/// Player-side speed -> steering -> detailed grip, in the order called by
/// player_op. This deliberately excludes its other logic and update_player_state.
/// No position integration, suspension, collision, or replay initialization occurs.
pub fn control_tick(
    state: &mut State,
    car: &Car<'_>,
    input: u8,
    fps: u16,
    elements: &[u8; 900],
    top_speed: &mut u16,
) -> Result<(), String> {
    let mut next = state.clone();
    let mut top = *top_speed;
    drivetrain::speed_tick(&mut next, car, input, fps, false, 200, &mut top)?;
    let angle = steering::update(
        next.word(grip::STEERING) as i16,
        next.word(drivetrain::SPEED2),
        (input >> 2) & 3,
        fps,
    )?;
    next.set_word(grip::STEERING, angle as u16);
    grip::tick(&mut next, car, true, elements)?;
    *state = next;
    *top_speed = top;
    Ok(())
}

/// Deterministic scenario used by both the CLI and independent DOS fixtures.
pub fn scenario_input(frame: usize) -> u8 {
    match frame {
        0..=49 => 1,
        50..=79 => 5,
        80..=99 => 9,
        100..=119 => 0,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_control_tick_preserves_state_and_top_speed() {
        let car = Car::parse(include_bytes!("../../stunts_dos/CARPMIN.RES")).unwrap();
        let mut state = initial_state(&car, 1).unwrap();
        // Speed calculation would modify state/top before grip rejects this code.
        state.0[grip::SURFACES] = 255;
        let before = state.clone();
        let mut top = 77;
        assert!(control_tick(&mut state, &car, 1, 20, &[0; 900], &mut top).is_err());
        assert_eq!(state, before);
        assert_eq!(top, 77);
    }
    #[test]
    fn combined_kernels_match_original_across_6600_sequential_frames() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut key = String::new();
        let mut bytes = Vec::new();
        let mut state = State([0; 208]);
        let mut top = 0;
        let mut frame = 0;
        let mut total = 0;
        for line in include_str!("../tests/fixtures/control-traces-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let group = format!("{}:{}:{}", fields[0], fields[1], fields[2]);
            if group != key {
                bytes = std::fs::read(directory.join(fields[0])).unwrap();
                state = initial_state(&Car::parse(&bytes).unwrap(), fields[2].parse().unwrap())
                    .unwrap();
                top = 0;
                frame = 0;
                key = group;
            }
            assert_eq!(fields[3].parse::<usize>().unwrap(), frame);
            assert_eq!(fields[4].parse::<u8>().unwrap(), scenario_input(frame));
            control_tick(
                &mut state,
                &Car::parse(&bytes).unwrap(),
                scenario_input(frame),
                fields[1].parse().unwrap(),
                &[0; 900],
                &mut top,
            )
            .unwrap();
            let expected: Vec<u8> = fields[6]
                .as_bytes()
                .chunks_exact(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(&state.0[..], &expected, "{key}, frame={frame}");
            assert_eq!(top, fields[5].parse::<u16>().unwrap());
            total += 1;
            frame += 1;
        }
        assert_eq!(total, 6600);
    }
}
