use std::{fs::File, path::PathBuf};
use cpal::{self, SampleRate, BufferSize, traits::{DeviceTrait, HostTrait, StreamTrait}};
use crate::AppError;

pub fn get_input_device_list() -> Vec<String> {
    let input_devices = cpal::default_host().input_devices();
    if input_devices.is_err() {
        Vec::new()
    } else {
        input_devices.unwrap().map(|d| d.name().unwrap_or_default()).collect()
    }
}

pub fn get_output_device_list() -> Vec<String> {
    let output_devices = cpal::default_host().output_devices();
    if output_devices.is_err() {
        Vec::new()
    } else {
        output_devices.unwrap().map(|d| d.name().unwrap_or_default()).collect()
    }
}

pub enum DeviceType {
    File(File),
    Device(cpal::Device),
    None,
}

pub fn get_input_device(input_device_name: Option<String>, input_file_path: Option<PathBuf>) -> Result<DeviceType, AppError> {
    let mut input_device = DeviceType::None;
    if let Some(input_device_name) = input_device_name {
        if input_device_name == "From file" {
            let file = File::open(input_file_path.ok_or(AppError::NoFile("input"))?).map_err(|e| AppError::IoError(e))?;
            input_device = DeviceType::File(file);
        }
        else {
            let host = cpal::default_host();
            //FIXME better unique identifier than the name of the device?
            //or maybe it's guaranteed unique already idk
            let input_device_foo = host.input_devices()
                .map_err(|_e| AppError::NoDevice("input"))?
                .filter(|d| d.name().unwrap_or(String::from("")) == input_device_name)
                .next().ok_or(AppError::NoDevice("input"))?;

            input_device = DeviceType::Device(input_device_foo);
        }
    }
    else {
        input_device = DeviceType::Device(cpal::default_host().default_input_device().ok_or(AppError::NoDevice("input"))?);
    }
    Ok(input_device)
}

pub fn get_output_device(output_device_name: Option<String>, output_file_path: Option<PathBuf>) -> Result<DeviceType, AppError> {
    let mut output_device = DeviceType::None;
    let file: Option<std::fs::File> = None;
    if let Some(output_device_name) = output_device_name {
        if output_device_name == "To file" {
            let file = File::create(output_file_path.ok_or(AppError::NoFile("output"))?).map_err(|e| AppError::IoError(e))?;
            output_device = DeviceType::File(file);
        }
        else {
            let host = cpal::default_host();
            //FIXME better unique identifier than the name of the device?
            //or maybe it's guaranteed unique already idk
            let output_device_foo = host.output_devices()
                .map_err(|_e| AppError::NoDevice("output"))?
                .filter(|d| d.name().unwrap_or(String::from("")) == output_device_name)
                .next().ok_or(AppError::NoDevice("output"))?;
            output_device = DeviceType::Device(output_device_foo)
        }
    }
    else {
        output_device = DeviceType::Device(cpal::default_host().default_output_device().ok_or(AppError::NoDevice("output"))?);
    }
    Ok(output_device)
}

pub fn apply_volume(sample: f32, volume: f32) -> f32 {
    let db = (-90.0) + (30.0 - (-90.0)) * ( volume  / 100.0);
    let mut scale = (db/20.0 * (30.0f32).log10()).exp();
    if volume == 0.0 { scale = 0.0 };
    sample * scale
}

//FIXME maybe this should be more parameterizable and support resampling?
/// Play a 16KHz mono audio buffer on a 48KHz stereo target, applying the chosen volume
pub fn play_audio_buf(output_device: DeviceType, buf: &'static [f32], volume: f32) -> Result<(), AppError> {
    match output_device {
        DeviceType::File(f) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut writer = hound::WavWriter::new(f, spec).map_err(|e| AppError::HoundError(e))?;
            for sample in buf.iter() {
                writer.write_sample(apply_volume(*sample, volume)).map_err(|e| AppError::HoundError(e))?;
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(d) =>
        {
            let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
            let playback_secs = buf.len() as f32 / 16000.0;

            let mut i = 0;
            let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for (j, sample) in data.iter_mut().enumerate() {
                    let s = buf[i];
                    *sample = apply_volume(s, volume);
                    // Expand one sample to 6 to convert 16khz mono to 48khz stereo
                    if j%6 == 5 && i < buf.len() - 1
                    {
                        i += 1;
                    }
                }
            };

            let output_stream = d.build_output_stream(&output_config, output_data_fn, err_fn).map_err(|e| AppError::BuildOutputStreamError(e))?;
            output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
            std::thread::sleep(core::time::Duration::from_secs_f32(playback_secs));
        }
        DeviceType::None => {
            Err(AppError::NoDevice("output"))?
        }
    }
    Ok(())
}

//FIXME receiver is super hacky and gross. It's intended to make the stop button work and also
//allow volume to be adjustable during recording, find a better solution for this later
//do we even need volume to be adjustable during recording?
//
//Also we might want another version of this for stereo sources, virtual inputs are often stereo
//Figure out how to mix stereo down to mono when this is done, dropping half of it (ie using only
//one channel with no mixing) would sound terrible
//
//Another idea: maybe allow passing in a fixed-size buffer for the consumer to receive samples in 
//rather than returning a vec
///Record from a 48khz mono audio buffer into a 16khz mono format
pub fn record_audio_vec(input_device: DeviceType, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Result<Vec<f32>, AppError>
{
    match input_device {
        DeviceType::File(f) => {
            let mut reader = hound::WavReader::new(f).map_err(|e| AppError::HoundError(e))?;
            let record_vec: Result<Vec<f32>, AppError> = reader.samples::<f32>().map(|s| s.map_err(|e| AppError::HoundError(e))).collect();
            record_vec
        },
        DeviceType::Device(d) =>
        {

            let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default};
            let record_vec: Vec<f32> = Vec::new();

            use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
            let arc_vec = Arc::new(Mutex::new(record_vec));
            let arc1 = arc_vec.clone();
            let arc2 = arc_vec.clone();

            let input_vol = receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?.1;
            let atomic_vol = AtomicU32::new(input_vol.to_bits());
            let arc_atomic_vol = Arc::new(atomic_vol);

            let atomic_vol1 = arc_atomic_vol.clone();
            let atomic_vol2 = arc_atomic_vol.clone();


            let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let maybe_lock = arc1.try_lock();
                if maybe_lock.is_err() { println!("Failed to grab the lock in the input data fn") }
                else {
                    let lock = maybe_lock.unwrap();
                    let mut record_vec = lock;
                    let input_vol = f32::from_bits(atomic_vol1.load(Ordering::Relaxed));
                    for (j, &sample) in data.iter().enumerate() {
                        // Drop 2/3 of the samples to make 16KHz
                        if j % 3 == 0 {
                            record_vec.push(apply_volume(sample,  input_vol));
                        }
                    }
                }

            };

            let input_stream = d.build_input_stream(&input_config, input_data_fn, err_fn).map_err(|e| AppError::BuildInputStreamError(e))?;

            input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;

            loop {
                let (recording, ivol) = receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?;
                if recording {
                    atomic_vol2.store(ivol.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                } else {
                    break;
                }
            }
            let lock = arc2.try_lock().map_err(|_e| AppError::LockError)?;
            let record_vec = lock;

            Ok(record_vec.to_owned())
        },
        DeviceType::None => {
            Err(AppError::NoDevice("input"))
        }
    }
}

//TODO volume control?
#[allow(unused)]
pub fn record_audio_fixed_size(input_device: DeviceType, buf: &'static mut [f32]) -> Result<(), AppError> {
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default};
    let buf_secs = buf.len() as f32 / 16000.0;


    match input_device {
        DeviceType::File(f) => {
            let mut reader = hound::WavReader::new(f).map_err(|e| AppError::HoundError(e))?;
            reader.samples::<f32>().enumerate().try_for_each(|(i, sample)| 
            {
                if i < buf.len() {
                    buf[i] = sample.map_err(|e| AppError::HoundError(e))?;
                }
                Ok::<(), AppError>(())
            })?;
        },
        DeviceType::Device(d) => {
            let mut i = 0;
            let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for (j, &sample) in data.iter().enumerate() {
                    // Drop 2/3 of the samples to make 16KHz
                    if j % 3 == 0 {
                        buf[i] = sample;
                        i += 1;
                }
                if i >= buf.len() { break; }
                }
            };
            let input_stream = d.build_input_stream(&input_config, input_data_fn, err_fn).map_err(|e| AppError::BuildInputStreamError(e))?;
            input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;
            std::thread::sleep(core::time::Duration::from_secs_f32(buf_secs));
        },
        DeviceType::None => {
            Err(AppError::NoDevice("input"))?;
        }
    }
    Ok(())
}



//FIXME unpub this when the realtime audio code is moved in here
pub fn err_fn(err: cpal::StreamError) {
    eprintln!("Error: {:?}", err);
}
