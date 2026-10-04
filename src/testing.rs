//! Deterministic processors, audio assertions and synthesized buffers.
mod assertions;
mod fixtures;
mod mock_vst;
pub use assertions::*;
pub use fixtures::*;
pub use mock_vst::{FailingProcessor, MockVstPlugin};
