mod deinterleave;
mod devices;
mod engine;
mod generator;
mod realtime;

// CoreAudio backend for lower latency on macOS
#[cfg(target_os = "macos")]
pub mod coreaudio_backend;

pub use deinterleave::*;
pub use devices::*;
pub use engine::*;
pub use generator::*;
// realtime is only used internally by engine (imports directly from super::realtime)
