use ringbuf::RingBuffer;
use rubato::{FftFixedInOut, Resampler};
use webrtc_vad::{SampleRate, Vad, VadMode};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat};


pub mod play_target;
pub mod play_source;
pub mod realtime;

