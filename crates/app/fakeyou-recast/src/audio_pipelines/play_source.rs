use log::{info, debug};
use rust_i18n::t;
use std::sync::mpsc;
use crate::{audio, AppError};

/// Play back the input buffer
pub async fn play_source(source: Vec<f32>, output_device: audio::DeviceType, play_source_receiver: mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    info!("{}", t!("log.info.playing_source", dev = output_device.to_string().as_str()));
    debug!("source first 20 samples {:?}", source[0..20].to_vec());
    audio::play_audio_vec(output_device, source, play_source_receiver)
}
