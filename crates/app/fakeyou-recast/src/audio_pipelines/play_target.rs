use std::sync::mpsc::{self, TryRecvError};

use ringbuf::RingBuffer;
use rubato::{FftFixedInOut, Resampler};
use webrtc_vad::{SampleRate, Vad, VadMode};
use cpal::{traits::{DeviceTrait, StreamTrait}, SampleFormat};
use log::{info, debug, error};
use rust_i18n::t;

use crate::{softvc, handle_errors, AppError, audio::{self, DeviceType}, consts::{MODEL_SAMPLE_RATE, OFFLINE_CHUNK_SIZE}, 
    audio_pipelines::common::{compute_vad, replace_inactivity_with_silence, dupe_n_channels, dupe_n_channels_vecdeque}
}; 

use crate::OUTPUT_VOLUME; //FIXME get rid of this or better encapsulate it (currently a mutable static)

/// Convert and play back the input buffer with the target voice
pub async fn play_target(source: Vec<f32>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, output_device: audio::DeviceType, play_target_receiver: mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    info!("{}", t!("log.info.playing_target", dev = output_device.to_string().as_str()));
    debug!("first 20 recorded samples: {:?}", source[..20].to_vec());

    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::VeryAggressive);

    match output_device {
        DeviceType::File((ref f, _)) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: MODEL_SAMPLE_RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(f, spec).map_err(|e| AppError::HoundError(e))?;
            for chunk in source.chunks(OFFLINE_CHUNK_SIZE) {
                // Find active 10ms chunks with WebRTC VAD
                let vad_bools = compute_vad(&mut vad, chunk);

                let mut softvc_input = Vec::new();
                softvc_input.extend_from_slice(chunk);

                let mut out_samples = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone())?;

                // nuke inactive chunks from the buffer
                replace_inactivity_with_silence(&vad_bools, &mut out_samples);

                for sample in out_samples.iter() {
                    writer.write_sample(audio::float_to_pcm_s16(audio::apply_volume(*sample, unsafe { OUTPUT_VOLUME }))).map_err(|e| AppError::HoundError(e))?;
                }
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(ref d) => {
            let playback_secs = source.len() as f32 / MODEL_SAMPLE_RATE as f32;

            let output_config = d.default_output_config().map_err(|e| AppError::StreamConfigError(e))?;
            let out_rate = output_config.sample_rate().0 as usize;
            let n_channels = output_config.channels() as usize;
            let sample_format = output_config.sample_format();
            info!("{}", t!("log.info.out_config", ch = n_channels.to_string().as_str(), out_rate = out_rate.to_string().as_str(), format = format!("{:?}", sample_format).as_str()));

            let mut resampler = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, OFFLINE_CHUNK_SIZE, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
            let mut resampler_input_buffer = resampler.input_buffer_allocate();
            let mut resampler_output_buffer = resampler.output_buffer_allocate();
            let resampler_input_size = resampler.input_frames_next();
            let resampler_output_size = resampler.output_frames_next();

            debug!("resampler_input_size: {}", resampler_input_size);
            debug!("resampler_output_size: {}", resampler_output_size);

            resampler_input_buffer[0].resize(resampler_input_size, 0.0);

            let output_chunk_size = out_rate * resampler_input_size / MODEL_SAMPLE_RATE * n_channels;
            let ring: RingBuffer<f32> = RingBuffer::new(output_chunk_size*2);
            let (mut producer, mut consumer) = ring.split();

            let arc_cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_token1 = arc_cancel_token.clone();
            let cancel_token2 = arc_cancel_token.clone();
            let cancel_token3 = arc_cancel_token.clone();

            std::thread::spawn(move || {
                let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::VeryAggressive);
                for chunk in source.chunks(resampler_input_size)
                {
                    let processing_start_time = std::time::Instant::now();
                    // Find active 10ms chunks with WebRTC VAD
                    let vad_bools = compute_vad(&mut vad, chunk);

                    let softvc_out = softvc::inference(chunk, hubert.clone(), acoustic.clone(), hifigan.clone());
                    match softvc_out {
                        Err(e) => {
                            // don't spam errors if there's already one known
                            if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                                handle_errors(Err(e));
                            }
                            cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        Ok(mut samples) => {
                            // pad for the last chunk so we don't chop off the end of the audio
                            if samples.len() < resampler_input_size {
                                samples.resize(resampler_input_size, 0.0);
                            }
                            debug!("first 20 samples from inference: {:?}", samples[..20].to_vec()); 
                            replace_inactivity_with_silence(&vad_bools, &mut samples);
                            resampler_input_buffer[0].copy_from_slice(&samples);
                            let resampler_res = resampler.process_into_buffer(&resampler_input_buffer, &mut resampler_output_buffer, None);
                            if let Err(e) = resampler_res {
                                error!("{}", t!("log.error.out_resampler", error = e.to_string().as_str()));
                            }
                            dupe_n_channels(n_channels, &mut resampler_output_buffer[0], &mut producer)
                        }
                    }

                let processing_end_time = std::time::Instant::now();
                if cancel_token3.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let processing_time = processing_end_time - processing_start_time;
                let sleep_time = chunk.len() as f32 / MODEL_SAMPLE_RATE as f32 - processing_time.as_secs_f32();
                if sleep_time > 0.0 {
                    std::thread::sleep(core::time::Duration::from_secs_f32(sleep_time));
                }
                }
            });
            

            let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
            match data.sample_format() {
                SampleFormat::F32 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [f32] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::apply_volume(consumer.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                    }
                }
                SampleFormat::I16 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [i16] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::float_to_pcm_s16(audio::apply_volume(consumer.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME }));
                    }                    }
                SampleFormat::U16 => {}
            }

        };


        let output_stream = match sample_format
        {
            SampleFormat::F32 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, t!("misc.default")))?)
            }
            SampleFormat::I16 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::I16, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, t!("misc.default")))?)
            }
            SampleFormat::U16 => {
                Err(AppError::GenericFormatError)
            }

        }?;


        output_stream.play().map_err(|e| {error!("{}", t!("log.error.output_stream", error = e.to_string().as_str())); AppError::PlayStreamError(e, t!("misc.output"))})?;
        //add a couple seconds in case we fall behind
        let stop_time = std::time::Instant::now() + core::time::Duration::from_secs_f32(playback_secs + 2.0);
        loop {
            if cancel_token2.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > stop_time {
                break;
            }

            // The receiver tells us whether the user pressed stop and the current volume
            // setting
            match play_target_receiver.try_recv() {
                Err(TryRecvError::Empty) => {},
                Err(TryRecvError::Disconnected) => { Err(AppError::ChannelRecvError)?; }
                Ok((playing_target, ovol)) => {
                    if playing_target {
                        //FIXME unsafe
                        unsafe { OUTPUT_VOLUME = ovol };
                    } else {
                        break;
                    }
                }
            }

            std::thread::sleep(core::time::Duration::from_millis(50));
        }
        cancel_token2.store(true, std::sync::atomic::Ordering::Relaxed);
        core::mem::drop(output_stream);
    }
    DeviceType::None => {
        Err(AppError::NoDevice(t!("misc.output")))?
    }
    }
    info!("{}", t!("log.info.stopped_play_target"));
    Ok(())
}

/// For CPU inference, we process the entire buffer up front because we can't keep up with the
/// speed necessary for realtime
pub async fn play_target_cpu(source: Vec<f32>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, output_device: audio::DeviceType, play_target_receiver: mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    info!("{}", t!("log.info.playing_target", dev = output_device.to_string().as_str()));
    debug!("first 20 recorded samples: {:?}", source[..20].to_vec());
    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::VeryAggressive);

    match output_device {
        DeviceType::File((ref f, _)) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: MODEL_SAMPLE_RATE as u32,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::new(f, spec).map_err(|e| AppError::HoundError(e))?;
            for chunk in source.chunks(OFFLINE_CHUNK_SIZE) {
                // Find active 10ms chunks with WebRTC VAD
                let vad_bools = compute_vad(&mut vad, chunk);

                let mut softvc_input = Vec::new();
                softvc_input.extend_from_slice(chunk);

                let mut out_samples = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone())?;

                replace_inactivity_with_silence(&vad_bools, &mut out_samples);

                for sample in out_samples.iter() {
                    writer.write_sample(audio::float_to_pcm_s16(audio::apply_volume(*sample, unsafe { OUTPUT_VOLUME }))).map_err(|e| AppError::HoundError(e))?;
                }
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(ref d) => {
            let playback_secs = source.len() as f32 / MODEL_SAMPLE_RATE as f32;

            let output_config = d.default_output_config().map_err(|e| AppError::StreamConfigError(e))?;
            let out_rate = output_config.sample_rate().0 as usize;
            let n_channels = output_config.channels() as usize;
            let sample_format = output_config.sample_format();
            info!("{}", t!("log.info.out_config", ch = n_channels.to_string().as_str(), out_rate = out_rate.to_string().as_str(), format = format!("{:?}", sample_format).as_str()));

            let mut resampler = FftFixedInOut::<f32>::new(MODEL_SAMPLE_RATE, out_rate, OFFLINE_CHUNK_SIZE, 1).map_err(|e| AppError::ResamplerConstructionError(e))?;
            let mut resampler_input_buffer = resampler.input_buffer_allocate();
            let mut resampler_output_buffer = resampler.output_buffer_allocate();
            let resampler_input_size = resampler.input_frames_next();
            let resampler_output_size = resampler.output_frames_next();

            debug!("resampler_input_size: {}", resampler_input_size);
            debug!("resampler_output_size: {}", resampler_output_size);

            resampler_input_buffer[0].resize(resampler_input_size, 0.0);

            let mut producer = std::collections::VecDeque::new();

            let arc_cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_token1 = arc_cancel_token.clone();
            let cancel_token2 = arc_cancel_token.clone();
            let cancel_token3 = arc_cancel_token.clone();

            for chunk in source.chunks(resampler_input_size)
            {
                // Find active 10ms chunks with WebRTC VAD
                let vad_bools = compute_vad(&mut vad, chunk);

                let softvc_out = softvc::inference(chunk, hubert.clone(), acoustic.clone(), hifigan.clone());
                match softvc_out {
                    Err(e) => {
                        // don't spam errors if there's already one known
                        if !cancel_token1.load(std::sync::atomic::Ordering::Relaxed) {
                            handle_errors(Err(e));
                        }
                        cancel_token1.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(mut samples) => {
                        // pad for the last chunk so we don't chop off the end of the audio
                        if samples.len() < resampler_input_size {
                            samples.resize(resampler_input_size, 0.0);
                        }
                        debug!("first 20 samples from inference: {:?}", samples[..20].to_vec()); 

                        replace_inactivity_with_silence(&vad_bools, &mut samples);
                        resampler_input_buffer[0].copy_from_slice(&samples);
                        
                        let resampler_res = resampler.process_into_buffer(&resampler_input_buffer, &mut resampler_output_buffer, None);
                        if let Err(e) = resampler_res {
                            error!("{}", t!("log.error.out_resampler", error = e.to_string().as_str()));
                        }
                        dupe_n_channels_vecdeque(n_channels, &resampler_output_buffer[0], &mut producer)
                    }
                }
                if cancel_token3.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
        }

        let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
            match data.sample_format() {
                SampleFormat::F32 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [f32] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::apply_volume(producer.pop_front().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
                    }
                }
                SampleFormat::I16 => {
                    // this unwrap is safe because it only happens if the sample format doesn't
                    // match which we just checked 
                    let data_slice: &mut [i16] = data.as_slice_mut().unwrap(); 
                    for sample in data_slice.iter_mut() {
                        *sample = audio::float_to_pcm_s16(audio::apply_volume(producer.pop_front().unwrap_or(0.0), unsafe { OUTPUT_VOLUME }));
                    }                    }
                SampleFormat::U16 => {}
            }

        };


        let output_stream = match sample_format
        {
            SampleFormat::F32 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, t!("misc.default")))?)
            }
            SampleFormat::I16 => {
                Ok(d.build_output_stream_raw(&output_config.config(), SampleFormat::I16, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e, t!("misc.default")))?)
            }
            SampleFormat::U16 => {
                Err(AppError::GenericFormatError)
            }

        }?;


        output_stream.play().map_err(|e| {error!("{}", t!("log.error.output_stream", error = e.to_string().as_str())); AppError::PlayStreamError(e, t!("misc.output"))})?;
        let stop_time = std::time::Instant::now() + core::time::Duration::from_secs_f32(playback_secs + 2.0);
        loop {
            if cancel_token2.load(std::sync::atomic::Ordering::Relaxed) || std::time::Instant::now() > stop_time {
                break;
            }

            // The receiver tells us whether the user pressed stop and the current volume
            // setting
            match play_target_receiver.try_recv() {
                Err(TryRecvError::Empty) => {},
                Err(TryRecvError::Disconnected) => { Err(AppError::ChannelRecvError)?; }
                Ok((playing_target, ovol)) => {
                    if playing_target {
                        //FIXME unsafe
                        unsafe { OUTPUT_VOLUME = ovol };
                    } else {
                        break;
                    }
                }
            }

            std::thread::sleep(core::time::Duration::from_millis(50));
        }
        cancel_token2.store(true, std::sync::atomic::Ordering::Relaxed);
        core::mem::drop(output_stream);
    }
    DeviceType::None => {
        Err(AppError::NoDevice(t!("misc.output")))?
    }
    }
    info!("{}", t!("log.info.stopped_play_target"));
    Ok(())

}
