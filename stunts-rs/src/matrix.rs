//! Column-major 3x3 Q14 matrices as stored by the original DOS game.
//! Each product is shifted before summation. This differs from rounding the dot
//! product once, and from multiply_and_scale's rounding-bit behavior.
use crate::trig;
pub type Matrix = [i16; 9];
pub type Vector = [i16; 3];

fn product(a: i16, b: i16) -> i16 {
    ((i32::from(a) * i32::from(b)) >> 14) as i16
}

/// Transform with separate input/output storage, matching the non-aliasing DOS
/// call contract. In-place alias side effects are intentionally not reproduced.
pub fn transform(matrix: &Matrix, vector: &Vector) -> Vector {
    std::array::from_fn(|row| {
        (0..3).fold(0i16, |sum, column| {
            sum.wrapping_add(product(matrix[column * 3 + row], vector[column]))
        })
    })
}

/// Original mat_multiply(right,left,out) computes left * right. Return a fresh
/// array so partially overwritten inputs cannot affect subsequent products.
pub fn multiply(right: &Matrix, left: &Matrix) -> Matrix {
    std::array::from_fn(|index| {
        let row = index % 3;
        let column = index / 3;
        (0..3).fold(0i16, |sum, k| {
            sum.wrapping_add(product(right[column * 3 + k], left[k * 3 + row]))
        })
    })
}

/// The function named mat_invert in DOS transposes; it is not a general inverse.
pub fn transpose(matrix: &Matrix) -> Matrix {
    std::array::from_fn(|i| matrix[(i % 3) * 3 + i / 3])
}

pub fn rotation_x(angle: u16) -> Matrix {
    let c = trig::cos(angle);
    let s = trig::sin(angle);
    [16384, 0, 0, 0, c, s, 0, -s, c]
}
pub fn rotation_y(angle: u16) -> Matrix {
    let c = trig::cos(angle);
    let s = trig::sin(angle);
    [c, 0, -s, 0, 16384, 0, s, 0, c]
}
pub fn rotation_z(angle: u16) -> Matrix {
    let c = trig::cos(angle);
    let s = trig::sin(angle);
    [c, s, 0, -s, c, 0, 0, 0, 16384]
}

/// Pure result of the original optimized/cached mat_rot_zxy dispatcher.
/// Flags bit zero selects composition order; skipped identity axes and original
/// cache/pointer choices do not need representation in the returned value.
pub fn rotation_zxy(z: u16, x: u16, y: u16, flags: u8) -> Matrix {
    let rz = rotation_z(z);
    let rx = rotation_x(x);
    let ry = rotation_y(y);
    if flags & 1 != 0 {
        multiply(&multiply(&ry, &rx), &rz)
    } else {
        multiply(&multiply(&rz, &rx), &ry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(text: &str) -> Vec<i16> {
        text.split(';').map(|s| s.parse().unwrap()).collect()
    }
    #[test]
    fn composed_rotations_match_original_optimized_dispatcher() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/rotation-zxy-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let actual = rotation_zxy(
                fields[0].parse().unwrap(),
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
                fields[3].parse().unwrap(),
            );
            assert_eq!(&actual[..], &words(fields[4]), "{line}");
            count += 1;
        }
        assert_eq!(count, 756);
    }
    #[test]
    fn transforms_match_original_nonaliased_dos_functions() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/matrix-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let first: Matrix = words(fields[1]).try_into().unwrap();
            let expected = words(fields[3]);
            match fields[0] {
                "vector" => assert_eq!(
                    &transform(&first, &words(fields[2]).try_into().unwrap())[..],
                    &expected,
                    "{line}"
                ),
                "multiply" => assert_eq!(
                    &multiply(&first, &words(fields[2]).try_into().unwrap())[..],
                    &expected,
                    "{line}"
                ),
                "transpose" => assert_eq!(&transpose(&first)[..], &expected, "{line}"),
                _ => panic!("Unknown oracle operation"),
            }
            count += 1;
        }
        assert_eq!(count, 768);
    }
    #[test]
    fn all_three_rotation_builders_match_original() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/rotations-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let angle = fields[1].parse().unwrap();
            let actual = match fields[0] {
                "x" => rotation_x(angle),
                "y" => rotation_y(angle),
                "z" => rotation_z(angle),
                _ => panic!(),
            };
            assert_eq!(&actual[..], &words(fields[2]), "{line}");
            count += 1;
        }
        assert_eq!(count, 768);
    }
}
