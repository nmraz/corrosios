use crate::mm::types::PhysFrameNum;

// Place our boot heap at least at 16MiB to leave some low memory available for later.
pub const BOOTHEAP_BASE: PhysFrameNum = PhysFrameNum::new(0x1000);
