use std::fs::File;
use cpal::{self, traits::{DeviceTrait, HostTrait}};

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

