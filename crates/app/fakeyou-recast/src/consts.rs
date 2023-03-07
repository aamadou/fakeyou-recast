pub const MODEL_SAMPLE_RATE: usize = 16000;
pub const OFFLINE_CHUNK_SIZE: usize = 6*MODEL_SAMPLE_RATE;
pub const VAD_SIZE: usize = 160; 

/// Set up and configured via Discord developer panel.
/// The name of the application and thumbnail icons are configured from the web panel.
///   - docs: https://discord.com/rich-presence
///   - config: https://discord.com/developers/applications
//pub const FAKEYOU_DISCORD_APP_ID : &'static str = "1031772546841972806";
pub const FAKEYOU_DISCORD_APP_ID_INT : u64 = 1031772546841972806;

pub const WINDOW_WIDTH: u32 = 609;
#[cfg(not(feature="devmode"))]
pub const WINDOW_HEIGHT: u32 = 636;
#[cfg(feature="devmode")]
pub const WINDOW_HEIGHT: u32 = 750;
