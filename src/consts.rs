pub const MODEL_SAMPLE_RATE: usize = 16000;
pub const OFFLINE_CHUNK_SIZE: usize = 6*MODEL_SAMPLE_RATE;
pub const CHUNK_SIZE: usize = 17640; // 400ms at 44.1KHz
pub const TENSOR_SIZE: usize = 32000; // 2 seconds at 16KHz
pub const VAD_SIZE: usize = 800; // 50ms at 16KHz
pub const IN_BUFFER_SIZE: usize = CHUNK_SIZE;
pub const OUT_BUFFER_SIZE: usize = CHUNK_SIZE;
pub const WINDOW_WIDTH: u32 = 838;
pub const WINDOW_HEIGHT: u32 = 400;
pub const SPEAKER_THRESHOLD: f32 = 0.5;

//TODO string table for i18n
