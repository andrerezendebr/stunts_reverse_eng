//! One steering-response tick, ported from MCGA 1471:40A2.
//! The tables and signed branches were checked against the original machine code.
const RESPONSE_20: [i16; 16] = [8, 7, 6, 5, 4, 4, 3, 3, 2, 2, 2, 1, 1, 1, 1, 1];
const RESPONSE_10: [i16; 16] = [16, 14, 12, 10, 8, 8, 6, 6, 4, 4, 4, 2, 2, 1, 1, 1];

/// Return the next signed steering angle from the existing angle, raw speed,
/// two-bit steering code, and frame rate. Other car state is unchanged.
/// Code 1 adds angle, code 2 subtracts it; 0 and 3 request centering while moving.
/// As in the game, any frame rate other than 10 uses the 20-FPS response path.
pub fn update(angle: i16, speed: u16, input: u8, fps: u16) -> Result<i16, String> {
    if input > 3 {
        return Err("Steering input must be a two-bit code".into());
    }
    // The DOS index is ((speed >> 10) & 0xfc) + input. Each four-byte row
    // contains [0, +response, -response, 0], equivalent to speed >> 12 here.
    let response = if fps == 10 { RESPONSE_10 } else { RESPONSE_20 }[(speed >> 12) as usize];
    let raw_delta = match input {
        1 => response,
        2 => -response,
        _ => 0,
    };
    let mut delta = raw_delta;
    // Counter-steering is four times faster until the angle crosses near zero.
    if (delta > 0 && angle < -1) || (delta < 0 && angle > 1) {
        delta *= 4;
    }
    if delta == 0 && speed != 0 && angle != 0 {
        let centering = response * 2;
        delta = if angle.wrapping_abs() <= centering {
            angle.wrapping_neg()
        } else if angle > 0 {
            -centering
        } else {
            centering
        };
    }
    let limit = if fps == 10 { 160 } else { 80 };
    delta = delta.clamp(-limit, limit);
    let mut result = angle.wrapping_add(delta).clamp(-240, 240);
    // The small-angle dead zone applies when the table entry is zero, even if
    // speed is zero and the earlier moving-car centering branch was skipped.
    if raw_delta == 0 && result.abs() < 8 {
        result = 0;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::update;

    #[test]
    fn matches_original_relocated_dos_routine() {
        let fixture = include_str!("../tests/fixtures/steering-dos.csv");
        let mut count = 0;
        for line in fixture.lines().skip(1) {
            let values: Vec<i32> = line.split(',').map(|s| s.parse().unwrap()).collect();
            let result = update(
                values[0] as i16,
                values[1] as u16,
                values[2] as u8,
                values[3] as u16,
            )
            .unwrap();
            assert_eq!(result, values[4] as i16, "{line}");
            count += 1;
        }
        assert_eq!(count, 1488);
    }

    #[test]
    fn response_countersteering_centering_and_limits() {
        assert_eq!(update(0, 0, 1, 20).unwrap(), 8);
        assert_eq!(update(0, 0, 1, 10).unwrap(), 16);
        assert_eq!(update(-100, 0, 1, 20).unwrap(), -68);
        assert_eq!(update(100, 1, 0, 20).unwrap(), 84);
        assert_eq!(update(100, 0, 0, 20).unwrap(), 100);
        assert_eq!(update(7, 0, 0, 20).unwrap(), 0);
        assert_eq!(update(239, 0, 1, 20).unwrap(), 240);
        assert_eq!(update(0, 65535, 2, 10).unwrap(), -1);
        assert!(update(0, 0, 4, 20).is_err());
    }
}
