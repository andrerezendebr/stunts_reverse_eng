//! Complete update_grip port. Raw track cells are used only by banked-tile lookup.
use crate::{
    car::Car,
    drivetrain::{State, ALL_CONTACT, SPEED, SPEED2},
    fixed, trig,
};
pub const STEERING: usize = 0x20;
pub const SLIP: usize = 0x36;
pub const ANGLE_Z: usize = 0x3e;
pub const FRONT_ANGLE: usize = 0x40;
pub const RESIDUAL: usize = 0x42;
pub const DEMAND: usize = 0x44;
pub const AVAILABLE: usize = 0x46;
pub const SURFACES: usize = 0xc2;
pub const SLIDING: usize = 0xc7;
pub const CRASH: usize = 0xc9;

/// The third DOS argument selects the detailed path when nonzero; the reference
/// name "isOpponent" is misleading because player_op passes one here.
pub fn tick(
    state: &mut State,
    car: &Car<'_>,
    detailed: bool,
    elements: &[u8; 900],
) -> Result<(), String> {
    let mut next = state.clone();
    apply(&mut next, car, detailed, elements)?;
    *state = next;
    Ok(())
}

fn apply(s: &mut State, car: &Car<'_>, detailed: bool, elements: &[u8; 900]) -> Result<(), String> {
    if s.0[ALL_CONTACT] == 0 {
        s.set_word(FRONT_ANGLE, 0);
        s.0[SLIDING] = 0;
        return Ok(());
    }
    let grass = s.0[SURFACES..SURFACES + 4]
        .iter()
        .filter(|&&v| v == 4)
        .count();
    if grass != 0 {
        let divisor = [255, 256, 192, 128, 64][grass];
        let speed = s.word(SPEED2) - s.word(SPEED2) / divisor;
        s.set_word(SPEED2, speed);
        s.set_word(SPEED, speed);
    }
    let initial = (s.word(STEERING) as i16).wrapping_add(s.word(SLIP) as i16);
    let speed_high = s.word(SPEED) >> 8;
    let demand = ((speed_high.wrapping_mul(speed_high) >> 6)
        .wrapping_mul((initial.wrapping_abs() >> 3) as u16)) as i16;
    let mut surface_sum = 0i16;
    for &surface in &s.0[SURFACES..SURFACES + 4] {
        surface_sum = surface_sum.wrapping_add(car.surface_factor(surface)?);
    }
    let available =
        (i32::from(car.grip().wrapping_mul(2)).wrapping_mul(i32::from(surface_sum)) >> 10) as i16;
    s.set_word(DEMAND, demand as u16);
    s.set_word(AVAILABLE, available as u16);
    let mut current = initial;
    if detailed {
        let low_rotation = s.word(0x18) as u8 as i8;
        if s.word(STEERING) == 0 && low_rotation != 0 && i16::from(low_rotation).abs() < 8 {
            s.set_word(
                0x18,
                s.word(0x18)
                    .wrapping_add(if low_rotation > 0 { u16::MAX } else { 1 }),
            );
        }
        if demand > available {
            s.0[SLIDING] = 1;
            let denominator = i32::from(speed_high) * i32::from(speed_high);
            if denominator == 0 {
                return Err("Original grip division would divide by zero".into());
            }
            current = ((i32::from(available) << 8) / denominator) as i16;
            if initial < 0 {
                current = current.wrapping_neg();
            }
            current = current.wrapping_mul(3).wrapping_add(initial) >> 2;
            s.set_word(RESIDUAL, initial.wrapping_sub(current) as u16);
        } else {
            s.0[SLIDING] = 0;
            let residual = s.word(RESIDUAL) as i16;
            if residual != 0 {
                let mut reduced = residual.wrapping_sub(residual >> 4);
                if reduced.wrapping_abs() < 16 {
                    reduced >>= 1;
                }
                s.set_word(RESIDUAL, reduced as u16);
            }
        }
        s.set_word(
            FRONT_ANGLE,
            if s.word(ANGLE_Z) == 0 && s.0[CRASH] != 1 {
                current as u16
            } else {
                0
            },
        );
        let bank = s.word(0x1c) as i16;
        if bank != 0 && bank.wrapping_abs() > 4 {
            let mut x = s.0[2];
            let mut z = s.0[10];
            let tile = |x: u8, z: u8| -> Result<u8, String> {
                if x >= 30 || z >= 30 {
                    return Err("Banked-tile lookup lies outside the mapped track".into());
                }
                Ok(elements[usize::from(z) * 30 + usize::from(x)])
            };
            match tile(x, z)? {
                0xfd => {
                    x = x.wrapping_sub(1);
                    z = z.wrapping_add(1);
                }
                0xfe => {
                    z = z.wrapping_add(1);
                }
                0xff => {
                    x = x.wrapping_sub(1);
                }
                _ => {}
            }
            if (0x34..=0x37).contains(&tile(x, z)?) {
                s.set_word(
                    FRONT_ANGLE,
                    (s.word(FRONT_ANGLE) as i16).wrapping_add(bank / 5) as u16,
                );
            }
        }
        if available.wrapping_add(1000) < demand {
            let angle = (s.word(ANGLE_Z) as i16).wrapping_add(current.wrapping_sub(initial) / 14);
            s.set_word(ANGLE_Z, (angle / 2) as u16);
        } else if s.word(ANGLE_Z) != 0 {
            let angle =
                (s.word(ANGLE_Z) as i16).wrapping_add(current.wrapping_sub(initial) / 14) / 2;
            s.set_word(ANGLE_Z, angle as u16);
            if angle == 0 {
                let cosine = trig::cos(s.word(SLIP));
                let projected = fixed::multiply_and_scale(cosine, s.word(SPEED2) as i16);
                s.set_word(SPEED2, if cosine < 0 { 0 } else { projected as u16 });
                s.set_word(SLIP, 0);
            }
        }
    } else {
        s.set_word(FRONT_ANGLE, s.word(STEERING).wrapping_mul(4));
        if s.word(ANGLE_Z) != 0 {
            s.set_word(
                ANGLE_Z,
                ((s.word(ANGLE_Z) as i16).wrapping_mul(15) >> 4) as u16,
            );
        }
    }
    if s.word(SLIP) != 0 && s.word(ANGLE_Z) == 0 {
        s.set_word(SLIP, ((s.word(SLIP) as i16).wrapping_mul(15) >> 4) as u16);
    }
    if s.word(ANGLE_Z) != 0 {
        s.set_word(SLIP, s.word(SLIP).wrapping_sub(s.word(ANGLE_Z)));
    }
    if s.0[SLIDING] != 0 {
        let loss = (s.word(RESIDUAL) as i16).wrapping_abs().wrapping_mul(2) as u16;
        if s.word(SPEED) > loss {
            if s.word(SPEED2) > loss {
                s.set_word(SPEED, s.word(SPEED).wrapping_sub(loss));
                s.set_word(SPEED2, s.word(SPEED2).wrapping_sub(loss));
            } else {
                s.set_word(SPEED, 0);
                s.set_word(SPEED2, 0);
            }
            if s.0[CRASH] == 0 {
                s.0[0xcf] |= if s.0[SURFACES..SURFACES + 4].contains(&1) {
                    2
                } else {
                    4
                };
            }
        } else {
            s.set_word(SPEED, 0);
            s.set_word(SPEED2, 0);
        }
    }
    s.set_word(RESIDUAL, 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(hex: &str) -> State {
        State(
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
    }
    #[test]
    fn complete_grip_matches_original_dos_including_banked_tile_markers() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for line in include_str!("../tests/fixtures/grip-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let bytes = std::fs::read(directory.join(fields[0])).unwrap();
            let car = Car::parse(&bytes).unwrap();
            let mut cells = [0; 900];
            match fields[2] {
                "1" => cells[4 * 30 + 3] = 0x34,
                "2" => {
                    cells[4 * 30 + 3] = 0xfd;
                    cells[5 * 30 + 2] = 0x35;
                }
                "3" => {
                    cells[4 * 30 + 3] = 0xfe;
                    cells[5 * 30 + 3] = 0x36;
                }
                "4" => {
                    cells[4 * 30 + 3] = 0xff;
                    cells[4 * 30 + 2] = 0x37;
                }
                _ => {}
            }
            let mut actual = state(fields[3]);
            tick(&mut actual, &car, fields[1] == "1", &cells).unwrap();
            assert_eq!(actual, state(fields[4]), "case={count}, car={}", fields[0]);
            count += 1;
        }
        assert_eq!(count, 1584);
    }
}
