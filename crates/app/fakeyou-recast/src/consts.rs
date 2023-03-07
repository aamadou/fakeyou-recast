pub const MODEL_SAMPLE_RATE: usize = 16000;
pub const OFFLINE_CHUNK_SIZE: usize = 6*MODEL_SAMPLE_RATE;
pub const VAD_SIZE: usize = 160; 
pub const WINDOW_WIDTH: u32 = 609;
#[cfg(not(feature="devmode"))]
pub const WINDOW_HEIGHT: u32 = 636;
#[cfg(feature="devmode")]
pub const WINDOW_HEIGHT: u32 = 750;
