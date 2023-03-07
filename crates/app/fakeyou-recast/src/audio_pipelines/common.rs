use std::collections::VecDeque;

use ringbuf::{Consumer, Producer};
use webrtc_vad::Vad;

use crate::{consts::VAD_SIZE, audio};

pub fn compute_vad<'a>(vad: &'a mut Vad, buf: &'a [f32]) -> Vec<bool> {
    let mut vad_bools = Vec::new();

    buf.chunks(VAD_SIZE).for_each(|vad_chunk| {
        vad_bools.push(
            vad.is_voice_segment(
                &vad_chunk.iter().map(|a: &f32| audio::float_to_pcm_s16(*a)).collect::<Vec<i16>>()
            ).unwrap_or(false)
        )
    });
    vad_bools
}

pub fn replace_inactivity_with_silence<'a>(vad_bools: &'a Vec<bool>, buf: &'a mut [f32]) {
    for (i, chunk) in buf.chunks_mut(VAD_SIZE).enumerate() {
        if i >= vad_bools.len() { break };
        for sample in chunk {
            if !vad_bools[i] {
                *sample = 0.0
            }
        }
    }
}

pub fn mix_n_channels<'a>(n_channels: usize, consumer: &mut Consumer<f32>, output: &mut Vec<f32>, size: usize)  {
    for _ in  0..size / n_channels {
        let mut mixed: f32 = 0.0;
        for _ in 0..n_channels {
            mixed += consumer.pop().unwrap_or(0.0);
        }
        output.push(mixed);
    }
}

pub fn dupe_n_channels<'a>(n_channels: usize, input: &'a[f32], producer: &mut Producer<f32>) {
    for sample in input {
        for _ in 0..n_channels {
            producer.push(*sample).unwrap_or(())
        }
    }
}

pub fn dupe_n_channels_vecdeque<'a>(n_channels: usize, input: &'a[f32], producer: &mut VecDeque<f32>) {
    for sample in input {
        for _ in 0..n_channels {
            producer.push_back(*sample);
        }
    }
}
