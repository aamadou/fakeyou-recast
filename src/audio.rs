use std::fs::File;
use cpal::{self, SampleRate, BufferSize, traits::{DeviceTrait, HostTrait, StreamTrait}};

pub fn get_input_device(input_device_name: Option<String>) -> (Option<cpal::Device>, Option<File>) {
    let input_device: Option<cpal::Device>;
    let file: Option<std::fs::File> = None;
    if let Some(input_device_name) = input_device_name {
        if input_device_name == "From file" {
            todo!() 
        }
        else {
            let host = cpal::default_host();
            //FIXME better unique identifier than the name of the device?
            //or maybe it's guaranteed unique already idk
            let input_device_foo = host.input_devices().unwrap().filter(|d| d.name().unwrap() == input_device_name).next().unwrap();
            if cfg!(debug_assertions)
            {
                for config in input_device_foo.supported_input_configs().unwrap() {
                    // This print can get pretty spammy, but it's helpful for debug sometimes, idk
                    println!("{:?}", config);
                }
            }
            input_device = Some(input_device_foo)
        }
    }
    else {
        input_device = Some(cpal::default_host().default_input_device().unwrap());
        if cfg!(debug_assertions)
        {
            for config in input_device.as_ref().unwrap().supported_input_configs().unwrap() {
                println!("{:?}", config);
            }
        }
    }
    (input_device, file)
}

pub fn get_output_device(output_device_name: Option<String>) -> (Option<cpal::Device>, Option<File>) {
    let output_device: Option<cpal::Device>;
    let file: Option<std::fs::File> = None;
    if let Some(output_device_name) = output_device_name {
        if output_device_name == "From file" {
            todo!() 
        }
        else {
            let host = cpal::default_host();
            //FIXME better unique identifier than the name of the device?
            //or maybe it's guaranteed unique already idk
            let output_device_foo = host.output_devices().unwrap().filter(|d| d.name().unwrap() == output_device_name).next().unwrap();
            if cfg!(debug_assertions)
            {
                for config in output_device_foo.supported_output_configs().unwrap() {
                    // This print can get pretty spammy, but it's helpful for debug sometimes, idk
                    println!("{:?}", config);
                }
            }
            output_device = Some(output_device_foo)

        }
    }
    else {
        output_device = Some(cpal::default_host().default_output_device().unwrap());
    }
    (output_device, file)
}

pub fn apply_volume(sample: f32, volume: f32) -> f32 {
    let db = (-90.0) + (30.0 - (-90.0)) * ( volume  / 100.0);
    let mut scale = (db/20.0 * (30.0f32).log10()).exp();
    if volume == 0.0 { scale = 0.0 };
    sample * scale
}

//FIXME maybe this should be more parameterizable and support resampling?
/// Play a 16KHz mono audio buffer on a 48KHz stereo target, applying the chosen volume
pub fn play_audio_buf(output_device_name: Option<String>, buf: &'static [f32], volume: f32) {
    let output_device = get_output_device(output_device_name).0.unwrap();
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
    // FIXME move this into main and pass it in to get rid of static mut RECORD_BUF?
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

    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, err_fn).unwrap();
    output_stream.play().unwrap();
    std::thread::sleep(core::time::Duration::from_secs_f32(playback_secs));
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
pub fn record_audio_vec(input_device_name: Option<String>, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Vec<f32>
{
    let input_device = get_input_device(input_device_name).0.unwrap();
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default};
    let record_vec: Vec<f32> = Vec::new();

    use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
    let arc_vec = Arc::new(Mutex::new(record_vec));
    let arc1 = arc_vec.clone();
    let arc2 = arc_vec.clone();

    let input_vol = receiver.recv().unwrap().1;
    let atomic_vol = AtomicU32::new(input_vol.to_bits());
    let arc_atomic_vol = Arc::new(atomic_vol);

    let atomic_vol1 = arc_atomic_vol.clone();
    let atomic_vol2 = arc_atomic_vol.clone();


    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let lock = arc1.lock().unwrap();
        let mut record_vec = lock;
        let input_vol = f32::from_bits(atomic_vol1.load(Ordering::Relaxed));
        for (j, &sample) in data.iter().enumerate() {
            // Drop 2/3 of the samples to make 16KHz
            if j % 3 == 0 {
                record_vec.push(apply_volume(sample,  input_vol));
           }
        }
    };

    let input_stream = input_device.build_input_stream(&input_config, input_data_fn, err_fn).unwrap();

    input_stream.play().unwrap();

    loop {
        let (recording, ivol) = receiver.recv().unwrap();
        if recording {
            atomic_vol2.store(ivol.to_bits(), std::sync::atomic::Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(50));
        } else {
            break;
        }
    }
    let lock = arc2.lock().unwrap();
    let record_vec = lock;

    record_vec.to_owned()
}

//TODO volume control?
pub fn record_audio_fixed_size(input_device_name: Option<String>, buf: &'static mut [f32]) {
    let input_device = get_input_device(input_device_name).0.unwrap();
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default};
    let buf_secs = buf.len() as f32 / 16000.0;

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

    let input_stream = input_device.build_input_stream(&input_config, input_data_fn, err_fn).unwrap();

    input_stream.play().unwrap();
    std::thread::sleep(core::time::Duration::from_secs_f32(buf_secs));
}



//FIXME unpub this when the realtime audio code is moved in here
pub fn err_fn(err: cpal::StreamError) {
    eprintln!("Error: {:?}", err);
}
