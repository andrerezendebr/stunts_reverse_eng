//! Stunts 1.1 replay header, embedded track, and per-frame control stream.
use crate::track::{Track, FILE_SIZE};
pub const HEADER_SIZE: usize = 26;
pub const INPUT_OFFSET: usize = HEADER_SIZE + FILE_SIZE;

/// A lossless view of the original fields; identifiers are raw DOS byte strings.
#[derive(Debug)]
pub struct Replay<'a> {
    pub player_car: &'a [u8],
    pub player_material: u8,
    pub player_transmission: u8,
    pub opponent_type: u8,
    pub opponent_car: &'a [u8],
    pub opponent_material: u8,
    pub opponent_transmission: u8,
    pub track_name: &'a [u8],
    pub fps: u16,
    pub track: Track<'a>,
    pub inputs: &'a [u8],
}

impl<'a> Replay<'a> {
    /// Parse this version's exact layout. Reject truncation and extensions rather
    /// than interpreting an unknown format as valid recorded input.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < INPUT_OFFSET {
            return Err("Replay is missing its header or embedded track".into());
        }
        let fps = u16::from_le_bytes([bytes[22], bytes[23]]);
        let frames = u16::from_le_bytes([bytes[24], bytes[25]]) as usize;
        if fps == 0 {
            return Err("Replay frame rate must be nonzero".into());
        }
        if bytes.len() != INPUT_OFFSET + frames {
            return Err(format!(
                "Replay declares {frames} frames but contains {} input bytes",
                bytes.len() - INPUT_OFFSET
            ));
        }
        Ok(Self {
            player_car: &bytes[0..4],
            player_material: bytes[4],
            player_transmission: bytes[5],
            opponent_type: bytes[6],
            opponent_car: &bytes[7..11],
            opponent_material: bytes[11],
            opponent_transmission: bytes[12],
            track_name: &bytes[13..22],
            fps,
            track: Track::parse(&bytes[HEADER_SIZE..INPUT_OFFSET])?,
            inputs: &bytes[INPUT_OFFSET..],
        })
    }

    pub fn duration_seconds(&self) -> f64 {
        self.inputs.len() as f64 / f64::from(self.fps)
    }
}

/// Decode only masks confirmed in player_op and update_car_speed. Keep unknown
/// bits and steering codes raw until their remaining semantics are verified.
#[derive(Debug, PartialEq, Eq)]
pub struct Controls {
    pub throttle_code: u8,
    pub steering_code: u8,
    pub shift_up: bool,
    pub shift_down: bool,
    pub unknown_bits: u8,
}

impl From<u8> for Controls {
    fn from(raw: u8) -> Self {
        Self {
            throttle_code: raw & 3,
            steering_code: (raw >> 2) & 3,
            shift_up: raw & 0x10 != 0,
            shift_down: raw & 0x20 != 0,
            unknown_bits: raw & 0xc0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_replay_embeds_hell5_and_one_command_per_frame() {
        let data = include_bytes!("../../stunts_dos/DEFAULT.RPL");
        let replay = Replay::parse(data).unwrap();
        assert_eq!(replay.player_car, b"PMIN");
        assert_eq!(replay.opponent_car, b"\xffMIN");
        assert_eq!(replay.track_name, b"HELL5\0MP\0");
        assert_eq!(replay.fps, 20);
        assert_eq!(replay.inputs.len(), 10646);
        assert_eq!(replay.duration_seconds(), 532.3);
        assert_eq!(
            &data[HEADER_SIZE..INPUT_OFFSET],
            include_bytes!("../../stunts_dos/HELL5.TRK")
        );
        assert_eq!(replay.track.scenery_id(), 3);
        assert!(Replay::parse(&data[..data.len() - 1]).is_err());
    }

    #[test]
    fn checks_header_length_and_frame_rate() {
        assert!(Replay::parse(&[]).is_err());
        let mut bytes = vec![0; INPUT_OFFSET];
        assert!(Replay::parse(&bytes).is_err());
        bytes[22] = 20;
        assert!(Replay::parse(&bytes).is_ok());
        bytes.push(0);
        assert!(Replay::parse(&bytes).is_err());
        bytes[24] = 1;
        assert!(Replay::parse(&bytes).is_ok());
    }

    #[test]
    fn decodes_control_fields_without_losing_unknown_bits() {
        for raw in 0..=u8::MAX {
            let command = Controls::from(raw);
            let rebuilt = command.throttle_code
                | command.steering_code << 2
                | u8::from(command.shift_up) << 4
                | u8::from(command.shift_down) << 5
                | command.unknown_bits;
            assert_eq!(rebuilt, raw);
        }
    }
}
