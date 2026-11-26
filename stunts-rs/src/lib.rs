//! Partial Stunts port: original resource readers and CPU-verified numeric kernels.
//! Verified wheel prediction stops before original terrain/contact integration.
//! The separate prototype module and optional desktop binary provide experimental
//! world physics and graphics; they do not reproduce full DOS physics or audio.
//! No original DOS execution is needed
//! at runtime; optional Python CPU oracles are development tools only.
pub mod car;
pub mod compression;
pub mod drivetrain;
pub mod executable;
pub mod fixed;
pub mod grip;
pub mod matrix;
pub mod motion;
pub mod prototype;
pub mod random;
pub mod replay;
pub mod resource;
pub mod shape;
pub mod simulation;
mod sine_table;
pub mod steering;
pub mod track;
pub mod trig;
