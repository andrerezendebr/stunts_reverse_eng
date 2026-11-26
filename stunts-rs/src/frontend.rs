//! Shared original-resource catalog for native frontends.
use crate::{car::Car, track::Track};
use std::path::{Path, PathBuf};

/// Report failures even when a Windows GUI application has no console window.
#[cfg(target_os = "windows")]
pub fn show_error(message: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            window: *mut std::ffi::c_void,
            text: *const u16,
            title: *const u16,
            flags: u32,
        ) -> i32;
    }
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Stunts Rust".encode_utf16().chain(Some(0)).collect();
    // These terminated UTF-16 buffers remain alive throughout the modal call.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
    }
}
#[cfg(not(target_os = "windows"))]
pub fn show_error(message: &str) {
    eprintln!("{message}");
}

pub struct TrackChoice {
    pub name: String,
    pub bytes: Vec<u8>,
}
pub struct CarChoice {
    pub file: String,
    pub model: PathBuf,
    pub name: String,
}
pub struct Catalog {
    pub tracks: Vec<TrackChoice>,
    pub cars: Vec<CarChoice>,
}
impl Catalog {
    pub fn load(root: &Path) -> Result<Self, String> {
        let mut files = std::fs::read_dir(root)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        files.sort_by_key(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_uppercase()
        });
        let mut tracks = Vec::new();
        let mut cars = Vec::new();
        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let upper = name.to_ascii_uppercase();
            if upper.ends_with(".TRK") {
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                Track::parse(&bytes)?;
                tracks.push(TrackChoice { name, bytes });
            } else if upper.starts_with("CAR") && upper.ends_with(".RES") {
                let model_name = format!("ST{}.P3S", &upper[3..upper.len() - 4]);
                let model = files
                    .iter()
                    .find(|p| {
                        p.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&model_name)
                    })
                    .ok_or_else(|| format!("Missing car model {model_name}"))?;
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                let car = Car::parse(&bytes)?;
                let label = String::from_utf8_lossy(car.name)
                    .trim_end_matches('\0')
                    .to_string();
                cars.push(CarChoice {
                    file: name,
                    model: model.clone(),
                    name: label,
                });
            }
        }
        if tracks.is_empty() || cars.is_empty() {
            return Err("No playable tracks or cars found in the data directory".into());
        }
        Ok(Self { tracks, cars })
    }
}
