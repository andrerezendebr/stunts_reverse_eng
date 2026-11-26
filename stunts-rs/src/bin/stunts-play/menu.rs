//! Native front-end navigation. This is a new interface, not the DOS menu port.
use std::path::{Path, PathBuf};
use stunts_rs::{car::Car, track::Track};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Racing,
    Pause,
    Controls,
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Start,
    Resume,
    Restart,
    Quit,
}
pub struct Menu {
    pub screen: Screen,
    pub row: usize,
    pub track: usize,
    pub car: usize,
    pub error: Option<String>,
    tracks: usize,
    cars: usize,
}
impl Menu {
    pub fn new(tracks: usize, cars: usize, track: usize, car: usize) -> Self {
        assert!(tracks > 0 && cars > 0 && track < tracks && car < cars);
        Self {
            screen: Screen::Main,
            row: 0,
            track,
            car,
            error: None,
            tracks,
            cars,
        }
    }
    pub fn pause(&mut self) {
        self.screen = Screen::Pause;
        self.row = 0;
        self.error = None;
    }
    pub fn rows(&self) -> usize {
        match self.screen {
            Screen::Main => 5,
            Screen::Pause => 4,
            Screen::Controls => 1,
            Screen::Racing => 0,
        }
    }
    /// Navigation only requests race actions. The caller commits a new race
    /// after all selected files load successfully, so errors stay in the menu.
    pub fn act(&mut self, action: Action) -> Command {
        let count = self.rows();
        if count == 0 {
            return Command::None;
        }
        match action {
            Action::Up => self.row = (self.row + count - 1) % count,
            Action::Down => self.row = (self.row + 1) % count,
            Action::Left | Action::Right if self.screen == Screen::Main => {
                let backwards = matches!(action, Action::Left);
                if self.row == 1 {
                    self.track = cycle(self.track, self.tracks, backwards);
                }
                if self.row == 2 {
                    self.car = cycle(self.car, self.cars, backwards);
                }
                self.error = None;
            }
            Action::Back => {
                return match self.screen {
                    Screen::Main => Command::Quit,
                    Screen::Pause => Command::Resume,
                    Screen::Controls => {
                        self.screen = Screen::Main;
                        self.row = 3;
                        Command::None
                    }
                    _ => Command::None,
                }
            }
            Action::Confirm => {
                return match (self.screen, self.row) {
                    (Screen::Main, 0) => Command::Start,
                    (Screen::Main, 1) => {
                        self.track = cycle(self.track, self.tracks, false);
                        self.error = None;
                        Command::None
                    }
                    (Screen::Main, 2) => {
                        self.car = cycle(self.car, self.cars, false);
                        self.error = None;
                        Command::None
                    }
                    (Screen::Main, 3) => {
                        self.screen = Screen::Controls;
                        self.row = 0;
                        Command::None
                    }
                    (Screen::Main, 4) | (Screen::Pause, 3) => Command::Quit,
                    (Screen::Pause, 0) => Command::Resume,
                    (Screen::Pause, 1) => Command::Restart,
                    (Screen::Pause, 2) => {
                        self.screen = Screen::Main;
                        self.row = 0;
                        Command::None
                    }
                    (Screen::Controls, _) => {
                        self.screen = Screen::Main;
                        self.row = 3;
                        Command::None
                    }
                    _ => Command::None,
                }
            }
            _ => {}
        }
        Command::None
    }
    pub fn draw(&self, pixels: &mut [u32], width: usize, height: usize, catalog: &Catalog) {
        for y in 0..height {
            let shade = 12 + (y * 12 / height) as u32;
            pixels[y * width..(y + 1) * width]
                .fill((shade << 16) | ((shade + 8) << 8) | (shade + 20));
        }
        rect(pixels, width, 48, 45, width - 96, 5, 0xf4c34e);
        text(pixels, width, 55, 70, "STUNTS", 7, 0xf4c34e);
        text(pixels, width, 58, 131, "RUST EDITION", 2, 0xc0cbd8);
        let labels: &[&str] = match self.screen {
            Screen::Main => &["START RACE", "TRACK", "CAR", "CONTROLS", "EXIT"],
            Screen::Pause => &["RESUME", "RESTART RACE", "MAIN MENU", "EXIT"],
            Screen::Controls => &["BACK"],
            Screen::Racing => &[],
        };
        for (i, label) in labels.iter().enumerate() {
            let y = 210 + i * 55;
            if i == self.row {
                rect(pixels, width, 48, y - 12, 386, 44, 0x30455c);
                rect(pixels, width, 48, y - 12, 5, 44, 0xf4c34e);
            }
            text(
                pixels,
                width,
                72,
                y,
                label,
                3,
                if i == self.row { 0xffffff } else { 0xa4b4c6 },
            );
            if self.screen == Screen::Main && (i == 1 || i == 2) {
                text(pixels, width, 380, y, "< >", 2, 0xf4c34e);
            }
        }
        match self.screen {
            Screen::Main => {
                let choice = &catalog.tracks[self.track];
                let track = Track::parse(&choice.bytes).unwrap();
                text(
                    pixels,
                    width,
                    492,
                    190,
                    &choice.name.to_ascii_uppercase(),
                    3,
                    0xffffff,
                );
                // The preview borrows the same original track bytes used to start
                // a race. Filler cells belong to multi-tile pieces, not new roads.
                for y in 0..30 {
                    for x in 0..30 {
                        let id = track.element(x, y).unwrap();
                        let terrain = track.terrain(x, y).unwrap();
                        let color = if id != 0 && id < 0xfd {
                            0xcbd2d8
                        } else if terrain == 1 {
                            0x3977a3
                        } else if terrain >= 6 {
                            0x64824e
                        } else {
                            0x294b3b
                        };
                        rect(pixels, width, 492 + x * 8, 230 + y * 8, 7, 7, color);
                    }
                }
                text(
                    pixels,
                    width,
                    758,
                    235,
                    &format!("{} / {}", self.track + 1, self.tracks),
                    2,
                    0xa4b4c6,
                );
                text(
                    pixels,
                    width,
                    492,
                    489,
                    &catalog.cars[self.car].name.to_ascii_uppercase(),
                    2,
                    0xf4c34e,
                );
                text(
                    pixels,
                    width,
                    492,
                    515,
                    &format!("CAR {} / {}", self.car + 1, self.cars),
                    2,
                    0xa4b4c6,
                );
            }
            Screen::Pause => {
                text(pixels, width, 492, 225, "PAUSED", 5, 0xf4c34e);
                text(pixels, width, 492, 292, "YOUR RACE IS ON HOLD", 2, 0xc0cbd8);
                text(pixels, width, 492, 335, "ESC TO RESUME", 2, 0xc0cbd8);
            }
            Screen::Controls => {
                text(pixels, width, 492, 200, "DRIVING CONTROLS", 3, 0xf4c34e);
                for (i, line) in [
                    "W / UP     ACCELERATE",
                    "S / DOWN   BRAKE",
                    "A / LEFT   STEER LEFT",
                    "D / RIGHT  STEER RIGHT",
                    "R          RESET CAR",
                    "ESC        PAUSE MENU",
                ]
                .iter()
                .enumerate()
                {
                    text(pixels, width, 492, 255 + i * 36, line, 2, 0xc0cbd8);
                }
            }
            _ => {}
        }
        if let Some(error) = &self.error {
            text(
                pixels,
                width,
                55,
                551,
                &format!("LOAD FAILED: {error}").to_ascii_uppercase(),
                1,
                0xff8888,
            );
        }
        text(
            pixels,
            width,
            55,
            height - 25,
            "UP/DOWN SELECT   LEFT/RIGHT CHANGE   ENTER CONFIRM   ESC BACK",
            2,
            0x93a8be,
        );
    }
}
fn cycle(index: usize, len: usize, backwards: bool) -> usize {
    if backwards {
        (index + len - 1) % len
    } else {
        (index + 1) % len
    }
}

fn rect(pixels: &mut [u32], width: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    let height = pixels.len() / width;
    for row in y.min(height)..y.saturating_add(h).min(height) {
        let start = x.min(width);
        let end = x.saturating_add(w).min(width);
        pixels[row * width + start..row * width + end].fill(color);
    }
}
pub fn text(
    pixels: &mut [u32],
    width: usize,
    x: usize,
    y: usize,
    label: &str,
    scale: usize,
    color: u32,
) {
    for (i, c) in label.chars().enumerate() {
        let px = x + i * 6 * scale;
        if px + 5 * scale > width {
            break;
        }
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    rect(
                        pixels,
                        width,
                        px + col * scale,
                        y + row * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}
/// A tiny built-in bitmap font keeps menus independent of OS fonts and assets.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '<' => [2, 4, 8, 16, 8, 4, 2],
        '>' => [8, 4, 2, 1, 2, 4, 8],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        '&' => [12, 18, 20, 8, 21, 18, 13],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        ' ' => [0; 7],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_wraps_and_race_actions_do_not_commit_until_load_succeeds() {
        let mut menu = Menu::new(41, 11, 0, 0);
        menu.act(Action::Down);
        menu.act(Action::Left);
        assert_eq!(menu.track, 40);
        menu.act(Action::Right);
        assert_eq!(menu.track, 0);
        menu.act(Action::Down);
        menu.act(Action::Left);
        assert_eq!(menu.car, 10);
        menu.act(Action::Up);
        menu.act(Action::Up);
        assert_eq!(menu.act(Action::Confirm), Command::Start);
        assert_eq!(menu.screen, Screen::Main);
        menu.screen = Screen::Racing;
        menu.pause();
        assert_eq!(menu.act(Action::Back), Command::Resume);
        menu.act(Action::Down);
        assert_eq!(menu.act(Action::Confirm), Command::Restart);
        menu.act(Action::Down);
        menu.act(Action::Confirm);
        assert_eq!(menu.screen, Screen::Main);
    }
    #[test]
    fn original_catalog_contains_all_tracks_and_cars_and_menu_draws() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let catalog = Catalog::load(&root).unwrap();
        assert_eq!(catalog.tracks.len(), 41);
        assert_eq!(catalog.cars.len(), 11);
        let mut menu = Menu::new(41, 11, 0, 0);
        let mut pixels = vec![0; 960 * 600];
        menu.draw(&mut pixels, 960, 600, &catalog);
        assert!(pixels.contains(&0xf4c34e));
        menu.row = 3;
        menu.act(Action::Confirm);
        assert_eq!(menu.screen, Screen::Controls);
        menu.draw(&mut pixels, 960, 600, &catalog);
        menu.act(Action::Back);
        assert_eq!(menu.screen, Screen::Main);
    }
}
