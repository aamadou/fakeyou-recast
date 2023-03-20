use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;

use native_dialog::MessageDialog;
use ringbuf::RingBuffer;
use rubato::{FftFixedInOut, Resampler};
use webrtc_vad::{SampleRate, Vad, VadMode};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat};
use log::{info, debug, error};
use rust_i18n::t;

use crate::audio_pipelines::common::{compute_vad, replace_inactivity_with_silence, mix_n_channels, dupe_n_channels};
use crate::{softvc, handle_errors, AppError, audio::{self, DeviceType}, consts::MODEL_SAMPLE_RATE};

use crate::{INPUT_VOLUME, OUTPUT_VOLUME};// FIXME (refactor this better)


/// Realtime record => record and playback at the same time
pub async fn start_realtime(realtime_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device: audio::DeviceType, output_device: audio::DeviceType, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, chunk_size_secs: f32, duplicate_5x: bool) -> Result<(), AppError>
{
    info!("{}", t!("log.info.start_realtime", in = input_device.to_string().as_str(), out = output_device.to_string().as_str()));
    let arc_cancel_token = std::sync::Arc::new(AtomicBool::new(false));
    let cancel_token1 = arc_cancel_token.clone();
    let cancel_token2 = arc_cancel_token.clone();
    let cancel_token3 = arc_cancel_token.clone();
    let cancel_token4 = arc_cancel_token.clone();

    let mut cpal_input_device = None;
    match input_device {
        DeviceType::File(_f) => {
            let _ = MessageDialog::new().set_type(native_dialog::MessageType::Error)
                .set_title((t!("app_name") + t!("misc.error").as_str()).as_str())
                .set_text(t!("log.error.realtime_file").as_str()).show_alert();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_input_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice(t!("misc.input")))?; }
    }

    let mut cpal_output_device = None;
    match output_device {
        DeviceType::File(_f) => {
            let _ = MessageDialog::new().set_type(native_dialog::MessageType::Error)
                .set_title((t!("app_name") + t!("misc.error").as_str()).as_str())
                .set_text(t!("log.error.realtime_file").as_str()).show_alert();

        },
        DeviceType::Device(cpal_dev) => {
            cpal_output_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice(t!("misc.output")))?; }
    }

    let input_config = cpal_input_device.as_ref().ok_or(AppError::NoDevice(t!("misc.input")))?.default_input_config().map_err(|_e| AppError::NoDevice(t!("misc.input")))?;
    let output_config = cpal_output_device.as_ref().ok_or(AppError::NoDevice(t!("misc.output")))?.default_output_config().map_err(|_e| AppError::NoDevice(t!("misc.output")))?;

    let in_rate = input_config.sample_rate().0 as usize;
    let in_n_channels = input_config.channels() as usize;
    let in_format = input_config.sample_format();
    
    let out_rate = output_config.sample_rate().0 as usize;
    let out_n_channels = output_config.channels() as usize;
    let out_format = output_config.sample_format();

    let in_chunk_size = (chunk_size_secs * in_rate as f32 * in_n_channels as f32) as usize;
    let out_chunk_size = (chunk_size_secs * out_rate as f32 * out_n_channels as f32) as usize;
                                                                                   
    info!("{}", t!("log.info.in_config", ch = in_n_channels.to_string().as_str(), freq = in_rate.to_string().as_str(), format = format!("{:?}", in_format).as_str()));
    info!("{}", t!("log.info.out_config", ch = out_n_channels.to_string().as_str(), freq = out_rate.to_string().as_str(), format = format!("{:?}", out_format).as_str()));

    let mut resampler1 = FftFixedInOut::<f32>::new(in_rate, MODEL_SAMPLE_RATE, (in_chunk_size as f32 / in_n_channels as f32 * MODEL_SAMPLE_RATE as f32 / in_rate as f32) as usize, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
    let mut resampler1_input_buffer = resampler1.input_buffer_allocate();
    let mut resampler1_output_buffer = resampler1.output_buffer_allocate();
    let resampler1_input_size = resampler1.input_frames_next();
    let resampler1_output_size = resampler1.output_frames_next();
    debug!("resampler1_input_size: {}", resampler1_input_size);
    debug!("resampler1_output_size: {}", resampler1_output_size);
    resampler1_input_buffer[0].resize(resampler1_input_size, 0.0);

    let mut resampler2 = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, out_chunk_size/out_n_channels, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
    let mut resampler2_input_buffer = resampler2.input_buffer_allocate();
    let mut resampler2_output_buffer = resampler2.output_buffer_allocate();
    let resampler2_input_size = resampler2.input_frames_next();
    let resampler2_output_size = resampler2.output_frames_next();
    debug!("resampler2_input_size: {}", resampler2_input_size);
    debug!("resampler2_output_size: {}", resampler2_output_size);
    resampler2_input_buffer[0].resize(resampler2_input_size, 0.0);

    let ring1: RingBuffer<f32> = RingBuffer::new(in_chunk_size*2);
    let (mut producer1, mut consumer1) = ring1.split();
    let ring2: RingBuffer<f32> = RingBuffer::new(out_chunk_size*2);
    let (mut producer2, mut consumer2) = ring2.split();


    let input_data_fn = move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
        match in_format {
            SampleFormat::F32 => {
                let data_slice: &[f32] = data.as_slice().unwrap();
                for sample in data_slice.iter() {
                    let _ = producer1.push(audio::apply_volume(*sample, unsafe { INPUT_VOLUME}));
                }
            }
            SampleFormat::I16 => {
                let data_slice: &[i16] = data.as_slice().unwrap();
                for sample in data_slice.iter() {
                    let _ = producer1.push(audio::apply_volume(audio::pcm_s16_to_float(*sample), unsafe { INPUT_VOLUME}));
                }
            }
            SampleFormat::U16 => {}
        }
    };

    let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
        match out_format {
            SampleFormat::F32 => {
                let data_slice: &mut [f32] = data.as_slice_mut().unwrap();
                for sample in data_slice.iter_mut() {
                    *sample = audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                }
            }
            SampleFormat::I16 => {
                let data_slice: &mut [i16] = data.as_slice_mut().unwrap();
                for sample in data_slice.iter_mut() {
                    *sample = audio::float_to_pcm_s16(audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME }));
                }
            }
            SampleFormat::U16 => {}
        }
    };


    std::thread::spawn(move || {
        let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive); 
        loop {
            if consumer1.len() >= resampler1_input_size {
                let processing_start_time = std::time::Instant::now();
                resampler1_input_buffer[0].clear();

                // mix stereo (or more channels) to mono
                mix_n_channels(in_n_channels, &mut consumer1, &mut resampler1_input_buffer[0], in_chunk_size);

                // downsample to 16k
                let res = resampler1.process_into_buffer(&resampler1_input_buffer, &mut resampler1_output_buffer, None);
                match res {
                    Ok(_) => {}
                    Err(e) => { error!("{}", t!("log.error.in_resampler", error = e.to_string().as_str())) }
                };


                let mut softvc_input = resampler1_output_buffer[0].clone();

                let original_len = softvc_input.len();

                if duplicate_5x {
                    while softvc_input.len() < original_len * 5 {
                        softvc_input.extend_from_within(0..softvc_input.len());
                    }
                }

                // Find active 10ms chunks with WebRTC VAD
                let vad_bools = compute_vad(&mut vad, &softvc_input);
                
                let res = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone());
                match res {
                    Err(e) => {
                        // don't spam errors if there's already one known
                        if !cancel_token1.load(Ordering::Relaxed) {
                            handle_errors(Err(e));
                        }
                        cancel_token1.store(true, Ordering::Relaxed);
                    },
                    Ok(mut samples) => {
                        replace_inactivity_with_silence(&vad_bools, &mut samples[..original_len]);
                        resampler2_input_buffer[0].copy_from_slice(&samples[..original_len]);
                        let res = resampler2.process_into_buffer(&resampler2_input_buffer, &mut resampler2_output_buffer, None);
                        match res {
                            Ok(_) => {}
                            Err(e) => { error!("{}", t!("log.error.out_resampler", error = e.to_string().as_str())) }
                        };

                        // mono to stereo etc.
                        dupe_n_channels(out_n_channels, &resampler2_output_buffer[0], &mut producer2)
                    }
                }
                let processing_end_time = std::time::Instant::now();
                if cancel_token3.load(Ordering::Relaxed) {
                    break;
                }
                let processing_time = processing_end_time - processing_start_time;
                let sleep_time = resampler1_output_size as f32 / MODEL_SAMPLE_RATE as f32 - processing_time.as_secs_f32();
                if sleep_time > 0.0 {
                    std::thread::sleep(core::time::Duration::from_secs_f32(sleep_time));
                }
            }
        }
    });

    let in_dev = cpal_input_device.ok_or(AppError::NoDevice(t!("misc.input")))?;
    let out_dev = cpal_output_device.ok_or(AppError::NoDevice(t!("misc.output")))?;

    let mut input_config = input_config.config();
    input_config.buffer_size = BufferSize::Fixed(in_chunk_size as u32);
    let mut output_config = output_config.config();
    output_config.buffer_size = BufferSize::Fixed(out_chunk_size as u32);
    
    let input_stream = match in_format {
        SampleFormat::F32 => {
            in_dev.build_input_stream_raw(&input_config, SampleFormat::F32, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e, format!("{}", in_chunk_size)))
        }
        SampleFormat::I16 => {
            in_dev.build_input_stream_raw(&input_config, SampleFormat::I16, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e, format!("{}", in_chunk_size)))
        }
        SampleFormat::U16 => {
            Err(AppError::GenericFormatError)
        }
    }?;

    let output_stream = match out_format {
        SampleFormat::F32 => {
            out_dev.build_output_stream_raw(&output_config, SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, format!("{}", out_chunk_size)))
        }
        SampleFormat::I16 => {
            out_dev.build_output_stream_raw(&output_config, SampleFormat::I16, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, format!("{}", out_chunk_size)))
        }
        SampleFormat::U16 => {
            Err(AppError::GenericFormatError)
        }
    }?;


    input_stream.play().map_err(|e| AppError::PlayStreamError(e, t!("misc.input")))?;
    output_stream.play().map_err(|e| AppError::PlayStreamError(e, t!("misc.output")))?;
    loop {
        if cancel_token2.load(Ordering::Relaxed) {
            break;
        }

        // The receiver tells us whether the user pressed stop and the current volume
        // setting
        match realtime_receiver.try_recv() {
            Err(TryRecvError::Empty) => {},
            Err(TryRecvError::Disconnected) => { Err(AppError::ChannelRecvError)?; }
            Ok((realtime_recording, ivol, ovol)) => {
                if realtime_recording {
                    //FIXME unsafe
                    unsafe { INPUT_VOLUME = ivol };
                    unsafe { OUTPUT_VOLUME = ovol };
                } else {
                    info!("Stopped realtime recording");
                    break;
                }
            }
        }
    }
    cancel_token4.store(true, Ordering::Relaxed);
    Ok(())
}
