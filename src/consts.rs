pub const MODEL_SAMPLE_RATE: usize = 16000;
pub const OUTPUT_SAMPLE_RATE: usize = 48000;
pub const OUTPUT_CHANNELS: usize = 2;
pub const OFFLINE_CHUNK_SECS: usize = 4;
pub const OFFLINE_CHUNK_SIZE: usize = MODEL_SAMPLE_RATE * OFFLINE_CHUNK_SECS;
//pub const OFFLINE_RINGBUFFER_SIZE: usize = OUTPUT_SAMPLE_RATE * OFFLINE_CHUNK_SECS * OUTPUT_CHANNELS * 2;
pub const CHUNK_SIZE: usize = 19200; // 400ms at 48KHz
pub const TENSOR_SIZE: usize = 32000; // 2 seconds at 16KHz
pub const VAD_SIZE: usize = 800; // 50ms at 16KHz
pub const CHUNK_SIZE_16K: usize = CHUNK_SIZE/3;
pub const IN_BUFFER_SIZE: usize = CHUNK_SIZE;
pub const OUT_BUFFER_SIZE: usize = CHUNK_SIZE;
pub const RINGBUFFER1_SIZE: usize = 2*CHUNK_SIZE;
pub const RINGBUFFER2_SIZE: usize = 2*CHUNK_SIZE*2; // 2 channels
pub const WINDOW_WIDTH: u32 = 838;
pub const WINDOW_HEIGHT: u32 = 400;
pub const SPEAKER_THRESHOLD: f32 = 0.5;

//TODO string table for i18n
