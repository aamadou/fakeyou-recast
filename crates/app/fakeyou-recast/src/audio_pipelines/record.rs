use std::sync::mpsc;
use rust_i18n::t;
use log::info;
use crate::{audio, error::AppError};

/// Record to a buffer for later conversion (non-realtime)
pub async fn start_recording(input_device: audio::DeviceType, receiver: mpsc::Receiver<(bool, f32)>) -> Result<Vec<f32>, AppError> {
    info!("{}", t!("log.info.started_recording", dev = input_device.to_string().as_str()));
    let vec = audio::record_audio_vec(input_device, receiver)?;
    Ok(vec)
}
