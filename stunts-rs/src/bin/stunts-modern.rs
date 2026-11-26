//! Modern GPU frontend. Resource readers and verified DOS kernels are shared.
#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]
#[path = "stunts-modern/gpu.rs"]
mod gpu;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use stunts_rs::{drivetrain, frontend::Catalog, session::Session, track::Track};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Main,
    Racing,
    Pause,
    Controls,
}
struct Options {
    root: PathBuf,
    track: String,
    car: String,
    quick: bool,
    frames: Option<usize>,
    screenshot: Option<PathBuf>,
    report: Option<PathBuf>,
    smoke: bool,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let value = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        Ok(Self {
            root: value("--data")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos")),
            track: value("--track").unwrap_or("HELL5.TRK".into()),
            car: value("--car").unwrap_or("CARPMIN.RES".into()),
            quick: args.iter().any(|a| a == "--quick-start"),
            frames: value("--frames")
                .map(|s| s.parse::<usize>().map_err(|e| e.to_string()))
                .transpose()?,
            screenshot: value("--screenshot").map(PathBuf::from),
            report: value("--report").map(PathBuf::from),
            smoke: args.iter().any(|a| a == "--smoke"),
        })
    }
}
#[derive(Default)]
struct Outcome {
    error: Option<String>,
}
struct App {
    options: Options,
    catalog: Catalog,
    track: usize,
    car: usize,
    screen: Screen,
    race: Option<Session>,
    gpu: gpu::Gpu,
    error: Option<String>,
    lighting: bool,
    previous: Instant,
    accumulator: Duration,
    frames: usize,
    capture_requested: bool,
    finished: bool,
    outcome: Arc<Mutex<Outcome>>,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        options: Options,
        outcome: Arc<Mutex<Outcome>>,
    ) -> Result<Self, String> {
        let catalog = Catalog::load(&options.root)?;
        let track = catalog
            .tracks
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(&options.track))
            .ok_or("Unknown track")?;
        let car = catalog
            .cars
            .iter()
            .position(|c| c.file.eq_ignore_ascii_case(&options.car))
            .ok_or("Unknown car")?;
        let gpu = gpu::Gpu::new(
            cc.wgpu_render_state
                .clone()
                .ok_or("GPU backend not initialized")?,
        );
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = Self {
            options,
            catalog,
            track,
            car,
            screen: Screen::Main,
            race: None,
            gpu,
            error: None,
            lighting: true,
            previous: Instant::now(),
            accumulator: Duration::ZERO,
            frames: 0,
            capture_requested: false,
            finished: false,
            outcome,
        };
        if app.options.quick || app.options.smoke {
            app.start()?;
        }
        Ok(app)
    }
    fn start(&mut self) -> Result<(), String> {
        let race = Session::load(&self.options.root, &self.catalog, self.track, self.car)?;
        self.gpu.load(&race);
        self.race = Some(race);
        self.screen = Screen::Racing;
        self.accumulator = Duration::ZERO;
        self.previous = Instant::now();
        self.error = None;
        Ok(())
    }
    fn main_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(25.0);
        ui.heading(
            egui::RichText::new("STUNTS")
                .size(54.0)
                .color(egui::Color32::from_rgb(244, 195, 78)),
        );
        ui.label("Rust Edition");
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(15.0);
        ui.columns(2, |columns| {
            let left = &mut columns[0];
            left.label("Track");
            egui::ComboBox::from_id_salt("track")
                .selected_text(&self.catalog.tracks[self.track].name)
                .width(280.0)
                .show_ui(left, |ui| {
                    for (i, t) in self.catalog.tracks.iter().enumerate() {
                        ui.selectable_value(&mut self.track, i, &t.name);
                    }
                });
            left.add_space(15.0);
            left.label("Car");
            egui::ComboBox::from_id_salt("car")
                .selected_text(&self.catalog.cars[self.car].name)
                .width(280.0)
                .show_ui(left, |ui| {
                    for (i, c) in self.catalog.cars.iter().enumerate() {
                        ui.selectable_value(&mut self.car, i, &c.name);
                    }
                });
            left.add_space(20.0);
            if left
                .add_sized([280.0, 42.0], egui::Button::new("Start race"))
                .clicked()
            {
                if let Err(e) = self.start() {
                    self.error = Some(e);
                }
            }
            if left
                .add_sized([280.0, 36.0], egui::Button::new("Controls"))
                .clicked()
            {
                self.screen = Screen::Controls;
            }
            if left
                .add_sized([280.0, 36.0], egui::Button::new("Exit"))
                .clicked()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            left.add_space(12.0);
            left.checkbox(&mut self.lighting, "Directional lighting");
            let right = &mut columns[1];
            right.label(&self.catalog.tracks[self.track].name);
            let size = right.available_width().min(300.0);
            let (rect, _) = right.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            let track = Track::parse(&self.catalog.tracks[self.track].bytes).unwrap();
            let step = size / 30.0;
            for y in 0..30 {
                for x in 0..30 {
                    let id = track.element(x, y).unwrap();
                    let terrain = track.terrain(x, y).unwrap();
                    let color = if id != 0 && id < 0xfd {
                        egui::Color32::LIGHT_GRAY
                    } else if terrain == 1 {
                        egui::Color32::from_rgb(40, 100, 150)
                    } else {
                        egui::Color32::from_rgb(40, 80, 55)
                    };
                    right.painter().rect_filled(
                        egui::Rect::from_min_size(
                            rect.min + egui::vec2(x as f32 * step, y as f32 * step),
                            egui::vec2(step - 0.5, step - 0.5),
                        ),
                        0.0,
                        color,
                    );
                }
            }
            right.label(format!(
                "{} tracks / {} cars",
                self.catalog.tracks.len(),
                self.catalog.cars.len()
            ));
        });
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    fn finish(&mut self, ctx: &egui::Context) {
        // Closing a native viewport is asynchronous; egui may update again
        // before it disappears. Commit reports once and never overwrite them.
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(path) = &self.options.report {
            let text = format!(
                "status={}\nadapter={}\nframes={}\ntrack={}\ncar={}\nscene_triangles={}\n",
                if self.outcome.lock().unwrap().error.is_none() {
                    "ok"
                } else {
                    "error"
                },
                self.gpu.adapter,
                self.frames,
                self.catalog.tracks[self.track].name,
                self.catalog.cars[self.car].file,
                self.race
                    .as_ref()
                    .map(|r| r.scene.triangles.len())
                    .unwrap_or(0)
            );
            if let Err(e) = write_new(path, text.as_bytes()) {
                self.outcome.lock().unwrap().error = Some(e);
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.finished {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let capture = ctx.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = capture {
            if let Some(path) = &self.options.screenshot {
                if let Err(e) = save_image(path, &image) {
                    self.outcome.lock().unwrap().error = Some(e);
                }
            }
            self.finish(ctx);
            return;
        }
        let previous_screen = self.screen;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.screen = match self.screen {
                Screen::Racing => Screen::Pause,
                Screen::Pause => Screen::Racing,
                Screen::Controls => Screen::Main,
                Screen::Main => Screen::Main,
            };
        }
        let now = Instant::now();
        let elapsed = now - self.previous;
        self.previous = now;
        if self.screen == Screen::Racing && previous_screen == Screen::Racing {
            self.accumulator += elapsed.min(Duration::from_millis(250));
            let mut input = 0;
            ctx.input(|i| {
                if i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp) {
                    input |= 1;
                }
                if i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown) {
                    input |= 2;
                }
                if i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft) {
                    input |= 8;
                }
                if i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight) {
                    input |= 4;
                }
            });
            if self.options.smoke {
                input = 1;
            }
            if let Some(race) = self.race.as_mut() {
                if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                    if let Err(e) = race.reset() {
                        self.error = Some(e);
                    }
                }
                while self.accumulator >= Duration::from_millis(50) {
                    if let Err(e) = race.tick(input) {
                        self.error = Some(e);
                        self.screen = Screen::Pause;
                        break;
                    }
                    self.accumulator -= Duration::from_millis(50);
                }
            }
        } else {
            self.accumulator = Duration::ZERO;
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.screen {
            Screen::Main => self.main_menu(ui, ctx),
            Screen::Controls => {
                ui.heading("Controls");
                for text in [
                    "W / Up: accelerate",
                    "S / Down: brake",
                    "A / Left and D / Right: steer",
                    "R: reset car",
                    "Escape: pause / resume",
                    "Use the mouse to select tracks, cars and menu options.",
                ] {
                    ui.label(text);
                }
                if ui.button("Back").clicked() {
                    self.screen = Screen::Main;
                }
            }
            Screen::Racing | Screen::Pause => {
                if let Some(race) = &self.race {
                    let size = ui.available_size();
                    let dpi = ctx.pixels_per_point();
                    self.gpu.render(
                        &race.world,
                        [(size.x * dpi) as u32, (size.y * dpi) as u32],
                        self.lighting,
                    );
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter().image(
                        self.gpu.texture,
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
        });
        if self.screen == Screen::Racing {
            egui::Area::new("telemetry".into())
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(20.0, 20.0))
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if let Some(race) = &self.race {
                            ui.label(format!(
                                "Speed {}    RPM {}    Gear {}",
                                race.world.state.word(drivetrain::SPEED2) / 256,
                                race.world.state.word(0x22),
                                race.world.state.0[0xbe]
                            ));
                        }
                        if ui.button("Pause").clicked() {
                            self.screen = Screen::Pause;
                            self.accumulator = Duration::ZERO;
                        }
                    });
                });
        }
        if self.screen == Screen::Pause {
            egui::Window::new("Paused")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if ui
                        .add_sized([220.0, 36.0], egui::Button::new("Resume"))
                        .clicked()
                    {
                        self.screen = Screen::Racing;
                        self.previous = Instant::now();
                    }
                    if ui
                        .add_sized([220.0, 36.0], egui::Button::new("Restart"))
                        .clicked()
                    {
                        if let Some(race) = self.race.as_mut() {
                            match race.reset() {
                                Ok(()) => self.screen = Screen::Racing,
                                Err(e) => self.error = Some(e),
                            }
                        }
                    }
                    if ui
                        .add_sized([220.0, 36.0], egui::Button::new("Main menu"))
                        .clicked()
                    {
                        self.screen = Screen::Main;
                    }
                    if ui
                        .add_sized([220.0, 36.0], egui::Button::new("Exit"))
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if let Some(e) = &self.error {
                        ui.colored_label(egui::Color32::LIGHT_RED, e);
                    }
                });
        }
        self.frames += 1;
        if self.options.frames.is_some_and(|n| self.frames >= n) && !self.capture_requested {
            if self.options.screenshot.is_some() {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            } else {
                self.finish(ctx);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())
}
fn save_image(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    let mut bytes = format!("P6\n{} {}\n255\n", image.size[0], image.size[1]).into_bytes();
    for pixel in &image.pixels {
        bytes.extend_from_slice(&pixel.to_array()[..3]);
    }
    write_new(path, &bytes)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse()?;
    let outcome = Arc::new(Mutex::new(Outcome::default()));
    let result = outcome.clone();
    let native = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([720.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Stunts Rust",
        native,
        Box::new(move |cc| Ok(Box::new(App::new(cc, options, result)?))),
    )?;
    if let Some(error) = outcome.lock().unwrap().error.take() {
        return Err(error.into());
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        // Startup errors remain diagnosable even when the Windows GUI has no
        // console. The optional report is also useful for unattended GPU smoke tests.
        let message = format!("Stunts Rust: {error}");
        if let Ok(options) = Options::parse() {
            if options.frames.is_none() {
                stunts_rs::frontend::show_error(&message);
            }
            if let Some(path) = options.report {
                let _ = write_new(&path, message.as_bytes());
            }
        }
        eprintln!("{message}");
        std::process::exit(1);
    }
}
