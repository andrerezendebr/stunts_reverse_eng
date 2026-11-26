//! Read confirmed SIMD car parameters from the original resource archives.
use crate::resource::Archive;

pub const SIMD_SIZE: usize = 0x308;

#[derive(Debug)]
pub struct Car<'a> {
    pub name: &'a [u8],
    simd: &'a [u8],
}

impl<'a> Car<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let archive = Archive::parse(bytes)?;
        let simd = archive.get(b"simd").ok_or("Car has no simd resource")?;
        if simd.len() != SIMD_SIZE {
            return Err(format!(
                "Expected a {SIMD_SIZE}-byte SIMD block, got {}",
                simd.len()
            ));
        }
        Ok(Self {
            name: archive.get(b"gnam").unwrap_or_default(),
            simd,
        })
    }

    fn signed_word(&self, offset: usize) -> i16 {
        i16::from_le_bytes(self.simd[offset..offset + 2].try_into().unwrap())
    }

    pub fn gears(&self) -> u8 {
        self.simd[0]
    }
    pub fn mass(&self) -> i16 {
        self.signed_word(2)
    }
    pub fn braking(&self) -> i16 {
        self.signed_word(4)
    }
    pub fn idle_rpm(&self) -> i16 {
        self.signed_word(6)
    }
    pub fn downshift_rpm(&self) -> i16 {
        self.signed_word(8)
    }
    pub fn upshift_rpm(&self) -> i16 {
        self.signed_word(10)
    }
    pub fn maximum_rpm(&self) -> i16 {
        self.signed_word(12)
    }
    pub fn aero_resistance(&self) -> i16 {
        self.signed_word(0x38)
    }
    pub fn idle_torque(&self) -> u8 {
        self.simd[0x3a]
    }
    pub fn torque_curve(&self) -> &'a [u8] {
        &self.simd[0x3b..0xa3]
    }
    pub fn grip(&self) -> i16 {
        self.signed_word(0xa4)
    }
    pub fn sliding(&self) -> i16 {
        self.signed_word(0xb4)
    }

    /// Original grip lookup starts at the sliding word, then indexes by surface.
    /// Code 5 (water) reaches the following word, retained raw rather than renamed.
    pub fn surface_factor(&self, surface: u8) -> Result<i16, String> {
        if surface > 5 {
            return Err("Unknown wheel surface code".into());
        }
        Ok(self.signed_word(0xb4 + usize::from(surface) * 2))
    }

    /// Ratios retain their unsigned on-disk representation; index zero is included.
    pub fn gear_ratios(&self) -> [u16; 7] {
        std::array::from_fn(|index| {
            u16::from_le_bytes(
                self.simd[0x0e + index * 2..0x10 + index * 2]
                    .try_into()
                    .unwrap(),
            )
        })
    }

    /// Raw shift-lever coordinates, including neutral at index zero.
    pub fn knob_point(&self, gear: u8) -> Result<(i16, i16), String> {
        if gear > self.gears() || gear > 6 {
            return Err("Gear lies outside the car's table".into());
        }
        let offset = 0x1c + usize::from(gear) * 4;
        Ok((self.signed_word(offset), self.signed_word(offset + 2)))
    }

    /// Torque selection block at DOS 17DE6..17E34, before ratio/mass scaling.
    /// This does not apply the caller's overspeed, airborne, or shifting branches.
    pub fn selected_torque(&self, gear: u8, rpm: u16, limiter_timer: u8) -> Result<u8, String> {
        self.knob_point(gear)?;
        let mut torque = if gear <= 1 && (rpm as i16) < 2600 {
            self.idle_torque()
        } else {
            *self
                .torque_curve()
                .get(usize::from(rpm >> 7))
                .ok_or("RPM lies outside the mapped torque curve")?
        };
        if limiter_timer != 0 && (rpm as i16) < 5000 {
            torque = ((u16::from(self.idle_torque()) + u16::from(torque)) >> 1) as u8;
        }
        Ok(torque)
    }

    /// setup_aero_trackdata builds 64 signed words: (aero * index²) >> 9.
    /// Explicit wrapping and narrowing match the DOS long multiply and word store.
    /// This table is a game quantity; physical units have not been established.
    pub fn aero_table(&self) -> [i16; 64] {
        std::array::from_fn(|index| {
            let index = index as i32;
            (i32::from(self.aero_resistance())
                .wrapping_mul(index)
                .wrapping_mul(index)
                >> 9) as i16
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aero_tables_match_original_dos_for_both_sides_of_every_car() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut tables = std::collections::HashMap::new();
        let mut checked = std::collections::HashSet::new();
        for line in include_str!("../tests/fixtures/car-aero-dos.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let table = tables.entry(fields[0].to_owned()).or_insert_with(|| {
                let bytes = std::fs::read(directory.join(fields[0])).unwrap();
                Car::parse(&bytes).unwrap().aero_table()
            });
            let index: usize = fields[2].parse().unwrap();
            let expected: i16 = fields[3].parse().unwrap();
            assert_eq!(table[index], expected, "{line}");
            assert!(checked.insert((fields[0], fields[1], index)));
        }
        assert_eq!(tables.len(), 11);
        assert_eq!(checked.len(), 1408);
    }

    #[test]
    fn reads_all_eleven_original_cars() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("CAR")
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("res"))
            {
                let bytes = std::fs::read(&path).unwrap();
                let car = Car::parse(&bytes).unwrap();
                assert!((1..=6).contains(&car.gears()));
                assert_eq!(car.torque_curve().len(), 104);
                assert_eq!(car.aero_table()[0], 0);
                count += 1;
            }
        }
        assert_eq!(count, 11);
    }
}
