//! Exact 1024-unit angle helpers. No floating-point approximation is involved.
use crate::sine_table::SINE;

pub fn sin(angle: u16) -> i16 {
    let low = usize::from(angle & 255);
    let quadrant = (angle >> 8) & 3;
    let index = if quadrant & 1 != 0 { 256 - low } else { low };
    if quadrant >= 2 {
        -SINE[index]
    } else {
        SINE[index]
    }
}

pub fn cos(angle: u16) -> i16 {
    sin(angle.wrapping_add(256))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_original_functions_and_all_word_angle_aliases() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/trig-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<i32> = line.split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(sin(fields[0] as u16), fields[1] as i16, "{line}");
            assert_eq!(cos(fields[0] as u16), fields[2] as i16, "{line}");
            count += 1;
        }
        assert_eq!(count, 1152);
        for angle in 0..=u16::MAX {
            assert_eq!(sin(angle), sin(angle & 1023));
            assert_eq!(cos(angle), cos(angle & 1023));
        }
    }
}
