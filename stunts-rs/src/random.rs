//! Six-byte generator used by the game's saved simulation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KevinRandom(pub [u8; 6]);
impl KevinRandom {
    pub fn next_byte(&mut self) -> u8 {
        // Descending cumulative additions use the newly written neighboring byte.
        for index in (0..5).rev() {
            self.0[index] = self.0[index].wrapping_add(self.0[index + 1]);
        }
        // Increment the transformed seed from byte five, propagating byte carries.
        for index in (0..6).rev() {
            self.0[index] = self.0[index].wrapping_add(1);
            if self.0[index] != 0 {
                break;
            }
        }
        self.0[0]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn seed(hex: &str) -> [u8; 6] {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }
    #[test]
    fn output_and_all_seed_bytes_match_original_generator() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/random-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let mut rng = KevinRandom(seed(fields[0]));
            assert_eq!(rng.next_byte(), fields[1].parse::<u8>().unwrap(), "{line}");
            assert_eq!(rng.0, seed(fields[2]), "{line}");
            count += 1;
        }
        assert_eq!(count, 1280);
        let mut carry = KevinRandom([0, 0, 0, 0, 0, 255]);
        assert_eq!(carry.next_byte(), 0);
        assert_eq!(carry.0, [0; 6]);
    }
}
