use std::{path::PathBuf, env, sync::{Arc, Mutex}};
use iced::{executor, Application, Element, Command, Settings, window, Subscription, keyboard::{self, KeyCode}};
use iced_native::{event, subscription, Event};
use cpal::{SampleRate, BufferSize, SampleFormat};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::RingBuffer;
use native_dialog::{FileDialog, MessageDialog};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("The {0} device could not be detemined")]
    NoDevice(&'static str),
    #[error("The {0} stream could not be constructed")]
    BuildStreamError(&'static str, #[source] cpal::BuildStreamError),
    #[error("The {0} stream could not be played")]
    PlayStreamError(&'static str, #[source] cpal::PlayStreamError),
    #[error("Nothing was received from the channel")]
    ChannelRecvError(#[from] std::sync::mpsc::RecvError),
    #[error("Attempted to send to a channel that had no receiver")]
    ChannelSendError/*(#[from] std::sync::mpsc::SendError)*/,
    #[error("IO Error")]
    IoError(#[from] std::io::Error),
    #[error("WAV read/write error")]
    HoundError(#[from] hound::Error),
    #[error("Attempted to lock a mutex that is already locked")]
    LockError/*(#[from] std::sync::TryLockError)*/,
    #[error("Failed to load the model")]
    ModelLoadError,
    #[error("No model is loaded")]
    NoModel,
    #[error("Inference failed to produce an output")]
    ModelError,
    #[error("File picker dialog failed to open for path:\n{0}")]
    FileDialogError(PathBuf),
}

mod view;
mod audio;
mod softvc;

use softvc::Model;

pub fn main() -> iced::Result {
    let settings = Settings {
        window: window::Settings {
            size: (838, 400),
            resizable: true,
            decorations: true,
            ..Default::default()
        },
        ..Default::default()
    };
    App::run(settings)
}

const WAV_TENSOR_SIZE: usize = 319507;

//FIXME unsafe
static mut RECORD_BUF: [f32; WAV_TENSOR_SIZE] = [0.0; WAV_TENSOR_SIZE];
static mut RECORD_SAMPLE_COUNT: usize = 0;
static mut INPUT_VOLUME: f32 = 80.0;
static mut OUTPUT_VOLUME: f32 = 80.0;

struct App {
    hubert: softvc::Hubert,
    acoustic: softvc::Acoustic,
    hifigan: softvc::Hifigan,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    record_sender: Option<std::sync::mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<std::sync::mpsc::Sender<(bool, f32, f32)>>,
    recording: bool,
    realtime_recording: bool,
}

#[derive(Clone, Debug)]
pub enum Message {
    ModelBrowsePressed,
    HifiganBrowsePressed,
    HubertBrowsePressed,
    ShowDebugPressed,
    Slider1Changed(f32),
    Slider2Changed(f32),
    RecordPressed,
    PlayTargetPressed,
    PlaySourcePressed,
    RealtimeRecordPressed,
    InputDeviceChanged(String),
    OutputDeviceChanged(String),
    InputBrowsePressed,
    OutputBrowsePressed,
    None
}

fn handle_errors(res: Result<(), AppError>) -> Message {
    if let Some(e) = res.err() {
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    }
    Message::None
}

impl Application for App {
    type Executor = executor::Default;
    type Message = crate::Message;
    type Flags = ();

    fn new(_flags: ()) -> (App, Command<Self::Message>) {
        let hubert = softvc::Hubert::new();
        let acoustic = softvc::Acoustic::new();
        let hifigan = softvc::Hifigan::new();
        let view_state = view::ViewState::new();
        let recording = false;
        let record_sender = None;
        let realtime_record_sender = None;
        let realtime_recording = false;
        let mut input_device_list_options = audio::get_input_device_list();
        input_device_list_options.insert(0, String::from("From file"));
        let mut output_device_list_options: Vec<String> = audio::get_output_device_list();
        output_device_list_options.insert(0, String::from("To file"));

        tch::maybe_init_cuda();

        (
            App {
                hubert,
                acoustic,
                hifigan,
                view_state,
                input_device_list_options,
                output_device_list_options,
                recording,
                record_sender,
                realtime_recording,
                realtime_record_sender,
            },
            Command::none()
        )
    }

    /// Controls the title bar of the application window
    fn title(&self) -> String {
        String::from("FakeYou Recast")
    }

    /// UI setup and state changes
    fn view(&mut self) -> Element<Message> { view::view(&mut self.view_state, &self.input_device_list_options, &self.output_device_list_options, self.recording, self.realtime_recording) }

    /// Respond to messages from the UI with appropriate actions
    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        let mut command: Command<Self::Message> = Command::none();
        match msg {
            Message::ShowDebugPressed => {
                self.view_state.show_debug = !self.view_state.show_debug;
            }
            Message::Slider1Changed(new_value) => {
                self.view_state.slider_1_value = new_value;
                //FIXME unsafe and stupid
                unsafe { INPUT_VOLUME = new_value };
                if self.realtime_recording && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
                if self.record_sender.is_some() {
                    self.record_sender.as_ref().unwrap().send((self.recording.clone(), self.view_state.slider_1_value.clone())).unwrap();
                }
            }
            Message::Slider2Changed(new_value) => {
                self.view_state.slider_2_value = new_value;
                //FIXME unsafe and stupid
                unsafe { OUTPUT_VOLUME  = new_value };
                if self.realtime_recording && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            // Realtime record button => record and playback at the same time
            Message::RealtimeRecordPressed => {
                if self.realtime_recording {
                    if self.realtime_record_sender.is_some()
                    {
                        let _res = self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                    }
                } else {
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();
                    command = Command::perform(start_realtime(realtime_record_receiver, input_device_name, output_device_name, hubert, acoustic, hifigan), handle_errors);
                    let _res = self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                }
                self.realtime_recording = !self.realtime_recording;
            }
            Message::ModelBrowsePressed => { 
                let location = std::env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                    .add_filter("FakeYou Recast Model", &["recast"])
                    .set_location(&location)
                    .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            let acoustic = self.acoustic.clone();
                            let path = p.clone();
                            std::thread::spawn(move ||{
                                acoustic.load(path)}
                            );
                            self.view_state.model_browse_selected = Some(p.clone());

                            let hifigan = self.hifigan.clone();
                            std::thread::spawn(move || {
                                hifigan.load(PathBuf::from("./models/model1.recast_base"))
                            });
                            self.view_state.hifigan_browse_selected = Some(PathBuf::from("./models/model1.recast_base"));

                            let hubert = self.hubert.clone();
                            std::thread::spawn(move ||{
                                hubert.load(PathBuf::from("./models/model2.recast_base"))}
                            );
                            self.view_state.hubert_browse_selected = Some(PathBuf::from("./models/model2.recast_base"));
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                    //FIXME some kind of progress bar while all this crap loads
                }
            }
            Message::HifiganBrowsePressed => { 
                let location = std::env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                        // Hifigan shhhh
                        .add_filter("Base Model 1", &["pt"])
                        .set_location(&location)
                        .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            let hifigan = self.hifigan.clone();
                            let path = p.clone();
                            std::thread::spawn(move ||{
                                hifigan.load(path)}
                            );
                            self.view_state.hifigan_browse_selected = Some(p.clone());
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                }
            }
            Message::HubertBrowsePressed => { 
                let location = std::env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                        // Hubert shhhh
                        .add_filter("Base Model 2", &["pt"])
                        .set_location(&location)
                        .show_open_single_file();
                    match path {
                        Ok(Some(p)) => {
                            let hubert = self.hubert.clone();
                            let path = p.clone();
                            std::thread::spawn(move ||{
                                hubert.load(path)}
                            );
                            self.view_state.hubert_browse_selected = Some(p.clone());
                        },
                        Ok(None) => {}
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                }
            }
            Message::InputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_open_single_file();
                match path {
                    Ok(o) => {
                        self.view_state.input_browse_selected = o;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_open_single_file();
                match path {
                    Ok(o) => {
                        self.view_state.input_browse_selected = o;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::RecordPressed => { 
                if self.recording {
                    if self.record_sender.is_some()
                    {
                        let _res = self.record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone())).map_err(|_e| AppError::ChannelSendError);
                        //handle_errors(res); 
                        //This error is kind of pointless because it's usually
                        //the result of another error, ignoring it to avoid spam
                    }
                } else {
                    let (record_sender, record_receiver) = std::sync::mpsc::channel();
                    self.record_sender = Some(record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    command = Command::perform(start_recording(input_device_name, record_receiver), handle_errors); 
                    self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone())).unwrap();
                }

                self.recording = !self.recording;

            }
            Message::PlayTargetPressed => {
                let output_device_name = self.view_state.output_device_list_selected.clone();
                let hubert = self.hubert.clone();
                let acoustic = self.acoustic.clone();
                let hifigan = self.hifigan.clone();
                command = Command::perform(play_target(hubert, acoustic, hifigan, output_device_name), handle_errors)
            }
            Message::PlaySourcePressed => { 
                let output_device_name = self.view_state.output_device_list_selected.clone();
                command = Command::perform(play_source(output_device_name), handle_errors)
            }
            Message::InputDeviceChanged(new_device) => {
                self.view_state.input_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                //if self.realtime_recording {
                    //self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    //let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    //self.realtime_record_sender = Some(realtime_record_sender);
                    //let input_device_name = self.view_state.input_device_list_selected.clone();
                    //let output_device_name = self.view_state.output_device_list_selected.clone();
                    //std::thread::spawn(move || { start_realtime(realtime_record_receiver, input_device_name, output_device_name) }); 
                    //self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                //}
            }
            Message::OutputDeviceChanged(new_device) => {
                self.view_state.output_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                //if self.realtime_recording {
                    //self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    //let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    //self.realtime_record_sender = Some(realtime_record_sender);
                    //let input_device_name = self.view_state.input_device_list_selected.clone();
                    //let output_device_name = self.view_state.output_device_list_selected.clone();
                    //std::thread::spawn(move || { start_realtime(realtime_record_receiver, input_device_name, output_device_name) }); 
                    //self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                //}

            }
            Message::None => { }
        }
        command
    }

    /// F12 hotkey for secret options
    fn subscription(&self) -> Subscription<Message> {
        subscription::events_with(|event, status| {
            if let event::Status::Captured = status {
                return None;
            }

            match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    modifiers: _,
                    key_code,
                }) => match key_code {
                    KeyCode::F12 => Some(Message::ShowDebugPressed),
                    _ => None,
                }
                _ => None,
            }
        })
    }
}

const TENSOR_SIZE: usize = 65536;
const CHUNK_SIZE: usize = 8192;
const IN_BUFFER_SIZE: usize = CHUNK_SIZE;
const OUT_BUFFER_SIZE: usize = 2*CHUNK_SIZE;
const RINGBUFFER1_SIZE: usize = 2*CHUNK_SIZE;
const RINGBUFFER2_SIZE: usize = RINGBUFFER1_SIZE*6;



static mut SOFTVC_INPUT: [f32; TENSOR_SIZE] = [0.0f32; TENSOR_SIZE];
static mut INPUT_CHUNK: [f32; CHUNK_SIZE] = [0.0f32; CHUNK_SIZE];
static mut OUT_SAMPLES: [f32;CHUNK_SIZE*6] = [0.0f32; CHUNK_SIZE*6];


//FIXME break this up and move parts of it into audio.rs
/// Realtime record => record and playback at the same time
async fn start_realtime(realtime_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device_name: Option<String>, output_device_name: Option<String>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan) -> Result<(), AppError>
{
    let input_device = audio::get_input_device(input_device_name)?.0.ok_or(AppError::NoDevice("input"))?;
    let output_device = audio::get_output_device(output_device_name)?.0.ok_or(AppError::NoDevice("output"))?;

    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Fixed(IN_BUFFER_SIZE as u32) };
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Fixed(OUT_BUFFER_SIZE as u32) };
    let ring1: RingBuffer<f32> = RingBuffer::new(RINGBUFFER1_SIZE);
    let (mut producer1, mut consumer1) = ring1.split();
    let ring2: RingBuffer<f32> = RingBuffer::new(RINGBUFFER2_SIZE);
    let (mut producer2, mut consumer2) = ring2.split();



    let mut input_count = 0;
    let mut i = 0;
    let input_data_fn = move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
        let data_slice = data.as_slice().unwrap();
        //println!("input_data_len {}", data_slice.len());
        for (j, &sample) in data_slice.iter().enumerate() {
            // Drop 2/3 of the samples to make 16KHz
            if j % 3 == 0 {
                producer1.push(sample);
                i += 1;
           }
        }


        input_count +=1;
    };

    let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
        println!("consumer1_len {}", consumer1.len());
        if consumer1.len() >= CHUNK_SIZE {
            unsafe { consumer1.pop_slice({&mut INPUT_CHUNK}); }
            //consumer1.discard(CHUNK_SIZE);
            for i in 0..(TENSOR_SIZE/CHUNK_SIZE) {
                unsafe { SOFTVC_INPUT[i*CHUNK_SIZE..((i+1)*CHUNK_SIZE)].copy_from_slice(&INPUT_CHUNK); }
            }
            //println!("softvc_input len {}", softvc_input.len());
            //softvc_input.resize(TENSOR_SIZE, 0.0);
            println!("out_data_len {}", data.len());
            let res = softvc::inference((unsafe { &SOFTVC_INPUT }), hubert.clone(), acoustic.clone(), hifigan.clone());
            match res {
                Err(e) => {
                    //handle_errors(Err(e));
                    println!("{}", e);
                },
                Ok(samples) => {
                    // dupe 6x to take mono 16khz to stereo 48khz
                    samples.iter().flat_map(|f| std::iter::repeat(*f).take(6)).for_each(|sample| producer2.push(sample).unwrap_or(()));
                }
            }
        }


        if consumer2.len() > CHUNK_SIZE * 6
        {
            let data_slice: &mut [f32] = data.as_slice_mut().unwrap();
            consumer2.pop_slice(data_slice);
        }
    };

    let input_stream = input_device.build_input_stream_raw(&input_config, SampleFormat::F32, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildStreamError("input", e))?;
    let output_stream = output_device.build_output_stream_raw(&output_config, SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildStreamError("output", e))?;

    input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;
    output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
    loop {

        let (realtime_recording, ivol, ovol) = realtime_receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?;
        if realtime_recording {
            //FIXME unsafe
            unsafe { INPUT_VOLUME = ivol };
            unsafe { OUTPUT_VOLUME = ovol };
        } else {
            break
        }
    }
    Ok(())
}

/// Record to a buffer for later conversion (non-realtime)
async fn start_recording(input_device_name: Option<String>, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Result<(), AppError> {
    let mut vec = audio::record_audio_vec(input_device_name, receiver)?;
    //FIXME unsafe and bad
    unsafe { RECORD_SAMPLE_COUNT = vec.len() };
    vec.resize(WAV_TENSOR_SIZE, 1.0);
    unsafe { RECORD_BUF.copy_from_slice(&vec.as_slice()) };
    Ok(())
}

/// Play back the input buffer
async fn play_source(output_device_name: Option<String>) -> Result<(), AppError> {
    audio::play_audio_buf(output_device_name, unsafe { &RECORD_BUF[..RECORD_SAMPLE_COUNT] }, unsafe { OUTPUT_VOLUME })
}

/// Convert and play back the input buffer with the target voice
/// Still undecided if we should separate the conversion to another button
/// If the conversion is fast enough, we don't need to worry too much about
/// redoing it when the user wants to hear the same input twice.
/// If we are too slow, it could be a nice time saver for that case though.
async fn play_target(hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, output_device_name: Option<String>) -> Result<(), AppError> {
    //TODO some kind of loading spinner or something while this is happening
    let mut wav_data = unsafe {RECORD_BUF}.to_vec();
    wav_data.resize(WAV_TENSOR_SIZE, 1.0);
    let out_samples = softvc::inference(&wav_data, hubert, acoustic, hifigan)?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create("test.wav", spec).map_err(|e| AppError::HoundError(e))?;
    for sample in out_samples.iter() {
        writer.write_sample(*sample).map_err(|e| AppError::HoundError(e))?;
    }

    let mut i = 0;
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        for (j, sample) in data.iter_mut().enumerate() {
            let s = out_samples[i];
            *sample = audio::apply_volume(s, unsafe { OUTPUT_VOLUME});
            // expand one sample to 6 to convert 16khz mono to 48khz stereo
            if j%6 == 0 && i < unsafe { RECORD_SAMPLE_COUNT } as usize
             {
                i += 1;
            }
        }
    };

    let output_device = audio::get_output_device(output_device_name)?.0.ok_or(AppError::NoDevice("output"))?;
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };

    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildStreamError("output", e))?;

    output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
    // FIXME ¯\_(ツ)_/¯
    // the goal of this is to stop playback before we run out of samples but it's super kludgey
    std::thread::sleep(core::time::Duration::from_secs_f32(unsafe { RECORD_SAMPLE_COUNT } as f32 / 16000.0));
    Ok(())
}
