//! Partial Stunts port: original resource readers and CPU-verified numeric kernels.
//! World integration, suspension, collisions, rendering, input devices and audio
//! are not implemented by this library yet. No original DOS execution is needed
//! at runtime; optional Python CPU oracles are development tools only.
pub mod car;
pub mod compression;
pub mod drivetrain;
pub mod executable;
pub mod fixed;
pub mod grip;
pub mod matrix;
pub mod motion;
pub mod random;
pub mod replay;
pub mod resource;
pub mod simulation;
mod sine_table;
pub mod steering;
pub mod track;
pub mod trig;
