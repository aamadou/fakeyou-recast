// launch as a gui app on windows without a command prompt
#![windows_subsystem = "windows"]

use std::{path::PathBuf, env};
use iced::{executor, Application, Element, Command, Settings, window::{self, Icon}, Subscription, keyboard::{self, KeyCode}};
use iced_native::{event, subscription, Event};
use cpal::{SampleRate, BufferSize, SampleFormat};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::RingBuffer;
use native_dialog::{FileDialog, MessageDialog};
use thiserror::Error;
use log::{info, trace, warn, error};

mod view;
mod audio;
mod softvc;
mod vad;
mod consts;

use softvc::Model;
use audio::DeviceType;
use consts::*;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("The {0} device could not be determined")]
    NoDevice(&'static str),
    #[error("No {0} file selected")]
    NoFile(&'static str),
    #[error("The stream could not be constructed.\nInput Device must support mono audio at 48kHz sample rate")]
    BuildInputStreamError(#[source] cpal::BuildStreamError),
    #[error("The stream could not be constructed.\nOutput Device must support stereo audio at 48kHz sample rate")]
    BuildOutputStreamError(#[source] cpal::BuildStreamError),
    #[error("The {0} stream could not be played")]
    PlayStreamError(&'static str, #[source] cpal::PlayStreamError),
    #[error("Nothing was received from the channel")]
    ChannelRecvError(#[from] std::sync::mpsc::RecvError),
    #[error("Attempted to send to a channel that had no receiver")]
    ChannelSendError/*(#[from] std::sync::mpsc::SendError)*/,
    #[error("IO Error\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("WAV read/write error\n{0}\nInput wav files must be 16KHz mono in 32-bit float format")]
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

const ICON: &[u8; 262144] = include_bytes!("../assets/icon.data");

pub fn main() -> iced::Result {
    let _ = init_logging(); //nowhere to log an error if one happens here
    let settings = Settings {
        window: window::Settings {
            size: (WINDOW_WIDTH, WINDOW_HEIGHT),
            resizable: true,
            decorations: true,
            icon: Some(Icon::from_rgba(ICON.to_vec(), 256, 256).unwrap()),
            ..Default::default()
        },
        ..Default::default()
    };
    App::run(settings)
}

fn init_logging() -> Result<(), fern::InitError> {
    std::fs::create_dir_all("./logs")?;
    let date = chrono::Local::now();
    let date_string = date.format("%Y-%m-%d");
    let file_path_str = format!("./logs/{}.txt", date_string);
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%H:%M:%S.%3f]"),
                record.level(),
                message
            ))
        })
        .level_for("fakeyou_recast", log::LevelFilter::Info)
        .level_for("fakeyou_recast::softvc", log::LevelFilter::Info)
        .level_for("fakeyou_recast::audio", log::LevelFilter::Info)
        .level_for("fakeyou_recast::vad", log::LevelFilter::Info)
        // we only want logs from our app not from all our dependencies
        .level(log::LevelFilter::Off)
        .chain(fern::log_file(file_path_str)?)
        .apply()?;
    info!("Logging started");
    Ok(())
}

//FIXME unsafe
static mut INPUT_VOLUME: f32 = 80.0;
static mut OUTPUT_VOLUME: f32 = 80.0;

static mut SOFTVC_INPUT: [f32; TENSOR_SIZE] = [0.0f32; TENSOR_SIZE];
static mut INPUT_CHUNK: [f32; CHUNK_SIZE] = [0.0f32; CHUNK_SIZE];
static mut VAD_CHUNK: [f32; VAD_SIZE] = [0.0f32; VAD_SIZE];
static mut VAD_CONFIDENCE: [f32; CHUNK_SIZE_16K/VAD_SIZE] = [0.0; CHUNK_SIZE_16K/VAD_SIZE];


struct App {
    hubert: softvc::Hubert,
    acoustic: softvc::Acoustic,
    hifigan: softvc::Hifigan,
    vad: vad::Vad,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    record_sender: Option<std::sync::mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<std::sync::mpsc::Sender<(bool, f32, f32)>>,
    recording: bool,
    realtime_recording: bool,
    record_vec: Vec<f32>,
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
    RecordingFinished(Vec<f32>),
    None
}

fn handle_errors(res: Result<(), AppError>) -> Message {
    if let Some(e) = res.err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    }
    Message::None
}

fn handle_recording(res: Result<Vec<f32>, AppError>) -> Message {
    if let Some(e) = res.as_ref().err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text(&format!("{}", e)).show_alert().unwrap();
    } else {
        let record_vec = res.unwrap();
        return Message::RecordingFinished(record_vec);
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
        let vad = vad::Vad::new();
        let view_state = view::ViewState::new();
        let recording = false;
        let record_sender = None;
        let realtime_record_sender = None;
        let realtime_recording = false;
        let mut input_device_list_options = audio::get_input_device_list();
        input_device_list_options.insert(0, String::from("From file"));
        let mut output_device_list_options: Vec<String> = audio::get_output_device_list();
        output_device_list_options.insert(0, String::from("To file"));
        let record_vec = Vec::new();

        tch::maybe_init_cuda();

        (
            App {
                hubert,
                acoustic,
                hifigan,
                vad,
                view_state,
                input_device_list_options,
                output_device_list_options,
                recording,
                record_sender,
                realtime_recording,
                realtime_record_sender,
                record_vec,
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
                if self.recording && self.record_sender.is_some() {
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
                        let res = self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                    }
                } else {
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);

                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let input_file = self.view_state.input_browse_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    let output_file = self.view_state.output_browse_selected.clone();
                    let input_device_res = audio::get_input_device(input_device_name, input_file);
                    let mut input_device = audio::DeviceType::None;
                    match input_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(inp) => { input_device = inp; }
                    };
                    let output_device_res = audio::get_output_device(output_device_name, output_file);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };

                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();
                    let vad = self.vad.clone();
                    command = Command::perform(start_realtime(realtime_record_receiver, input_device, output_device, hubert, acoustic, hifigan, vad), handle_errors);
                    let res = self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                    if let Some(e) = res.err() {
                        error!("{}", e);
                    }
                }
                self.realtime_recording = !self.realtime_recording;
            }
            Message::ModelBrowsePressed => { 
                let location = env::current_dir();
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
                            info!("Opening model: {}", p.clone().file_name().unwrap_or_default().to_string_lossy());
                            let acoustic = self.acoustic.clone();
                            let path = p.clone();
                            // TODO use async instead of threads and handle errors
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
                            let vad = self.vad.clone();
                            std::thread::spawn(move || {
                                vad.load(PathBuf::from("./models/silero_vad.jit"))
                            });
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                    //FIXME some kind of progress bar while all this crap loads
                }
            }
            Message::HifiganBrowsePressed => { 
                let location = env::current_dir();
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
                let location = env::current_dir();
                if location.is_err() {
                    // how does this even happen?
                    handle_errors(Err(AppError::FileDialogError(PathBuf::from(""))));
                }
                else {
                    let mut location = location.unwrap();
                    location.push("models");
                    let path = FileDialog::new()
                        // Hubert shhhh
                        .add_filter("Debug Model", &["pt"])
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
                    Ok(i) => {
                        info!("Selected file {} for input", i.clone().unwrap_or_default().file_name().unwrap_or_default().to_string_lossy());
                        self.view_state.input_browse_selected = i;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .set_filename("output.wav")
                    .show_save_single_file();
                match path {
                    Ok(o) => {
                        info!("Selected file {} for output", o.clone().unwrap_or_default().file_name().unwrap_or_default().to_string_lossy());
                        self.view_state.output_browse_selected = o;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::RecordPressed => { 
                let input_device_name = self.view_state.input_device_list_selected.clone();
                if self.recording && input_device_name != Some(String::from("From file")) {
                    if self.record_sender.is_some()
                    {
                        let res = self.record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                        //handle_errors(res); 
                        //This error is kind of pointless because it's usually
                        //the result of another error, ignoring it to avoid spam
                        self.recording = !self.recording;
                    }
                } else {
                    let (record_sender, record_receiver) = std::sync::mpsc::channel();
                    self.record_sender = Some(record_sender);
                    let input_file = self.view_state.input_browse_selected.clone();
                    let input_device_res = audio::get_input_device(input_device_name.clone(), input_file);
                    let mut input_device = audio::DeviceType::None;
                    match input_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(inp) => { input_device = inp; }
                    };

                    command = Command::perform(start_recording(input_device, record_receiver), handle_recording); 
                    if input_device_name != Some(String::from("From file")) {
                        self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone())).unwrap();
                        self.recording = !self.recording;
                    }
                }


            }
            Message::PlayTargetPressed => {
                let output_device_name = self.view_state.output_device_list_selected.clone();
                let output_file = self.view_state.output_browse_selected.clone();
                let hubert = self.hubert.clone();
                let acoustic = self.acoustic.clone();
                let hifigan = self.hifigan.clone();
                let vad = self.vad.clone();

                let output_device_res = audio::get_output_device(output_device_name, output_file);
                let mut output_device = audio::DeviceType::None;
                match output_device_res {
                    Err(e) => { handle_errors(Err(e)); },
                    Ok(out) => { output_device = out; }
                };
                //FIXME kind of a big clone
                command = Command::perform(play_target(self.record_vec.clone(), hubert, acoustic, hifigan, vad, output_device), handle_errors)
            }
            Message::PlaySourcePressed => { 
                let output_device_name = self.view_state.output_device_list_selected.clone();

                // pass None for file because playing source to file is pointless and confusing
                let output_device_res = audio::get_output_device(output_device_name, None);
                let mut output_device = audio::DeviceType::None;
                match output_device_res {
                    Err(e) => { handle_errors(Err(e)); },
                    Ok(out) => { output_device = out; }
                };
                //FIXME kind of a big clone
                command = Command::perform(play_source(self.record_vec.clone(), output_device), handle_errors)
            }
            Message::InputDeviceChanged(new_device) => {
                info!("Selected {} for input", new_device);
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
                info!("Selected {} for output", new_device);
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

            Message::RecordingFinished(vec) => {
                self.record_vec = vec;
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

//FIXME break this up and move parts of it into audio.rs
/// Realtime record => record and playback at the same time
async fn start_realtime(realtime_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device: audio::DeviceType, output_device: audio::DeviceType, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, vad: vad::Vad) -> Result<(), AppError>
{
    info!("Started realtime recording from {} with playback on {}", input_device, output_device);
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Fixed(IN_BUFFER_SIZE as u32) };
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Fixed(OUT_BUFFER_SIZE as u32) };
    let ring1: RingBuffer<f32> = RingBuffer::new(RINGBUFFER1_SIZE);
    let (mut producer1, mut consumer1) = ring1.split();
    let ring2: RingBuffer<f32> = RingBuffer::new(RINGBUFFER2_SIZE);
    let (mut producer2, mut consumer2) = ring2.split();

    let mut q = 0;

    let input_data_fn = move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
        let data_slice: &[f32] = data.as_slice().unwrap();
        for sample in data_slice.iter() {
            let _ = producer1.push(audio::apply_volume(*sample, unsafe { INPUT_VOLUME}));
            q += 1
        }
        if q == CHUNK_SIZE { 
            q = 0;
        }
    };

    let mut p = 0;
    let output_data_fn = move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
        if consumer1.len() >= CHUNK_SIZE {
            unsafe { consumer1.pop_slice(&mut INPUT_CHUNK); }
            let mut i = 0;
            // downsample to 16k
            for (j, &sample) in unsafe { INPUT_CHUNK }.iter().enumerate() {
                if j % 3 == 0 {
                   unsafe { SOFTVC_INPUT[i] = sample }; 
                   i+= 1;
                }
                if i >= unsafe { SOFTVC_INPUT }.len() {
                    i = 0;
                }
            }

            // Find active 50ms chunks with Silero VAD
            let mut v = 0;
            unsafe { &SOFTVC_INPUT[..CHUNK_SIZE_16K] }.chunks(VAD_SIZE).for_each(|chunk| {
                unsafe { VAD_CHUNK.copy_from_slice(chunk)};
                let vad_res = vad.forward(unsafe { &VAD_CHUNK});
                let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                match vad_res {
                    Err(e) => {
                        error!("{}", e);
                    },
                    Ok(t) => {
                        vad_tensor = t;
                    }
                }
                unsafe { VAD_CONFIDENCE[v] = vad_tensor.double_value(&[0]) as f32; }
                v += 1;
            });
            
            // Dupe SOFTVC_INPUT until it is TENSOR_SIZE long
            for k in 1..(TENSOR_SIZE/CHUNK_SIZE_16K) {
                unsafe { SOFTVC_INPUT.copy_within(0..CHUNK_SIZE_16K, CHUNK_SIZE_16K*k)};
            }
            let res = softvc::inference(unsafe { &SOFTVC_INPUT }, hubert.clone(), acoustic.clone(), hifigan.clone());
            match res {
                Err(e) => {
                    //handle_errors(Err(e));
                    error!("{}", e);
                },
                Ok(samples) => {
                    samples.iter()
                        .enumerate()
                        .map(|(e, sample)| {
                            let mut sa = 0.0;
                            for i in 0..CHUNK_SIZE_16K/VAD_SIZE {
                                if e < (i+1)*VAD_SIZE && e > i*VAD_SIZE &&  unsafe { VAD_CONFIDENCE[i] } > SPEAKER_THRESHOLD {
                                    sa = *sample;
                                    break;
                                }
                            }
                            sa
                        })
                        .map(|f| std::iter::repeat(f).take(6)) // upsample to 48khz stereo
                        .flatten()
                        .take(CHUNK_SIZE*2) // trim back down to CHUNK_SIZE (2 channels)
                        .for_each(|sample| { producer2.push(sample).unwrap_or(()); p += 1;});
                }
            }
        }

        let data_slice: &mut [f32] = data.as_slice_mut().unwrap();
        if consumer2.len() >= RINGBUFFER2_SIZE  / 2
        {
            // send output
            for sample in data_slice.iter_mut() {
                *sample = audio::apply_volume(consumer2.pop().unwrap_or(0.0), unsafe { OUTPUT_VOLUME });
            }
        } else {
            // send silence
            data_slice.fill(0.0);
        }
        if p == RINGBUFFER2_SIZE / 2 {
            p = 0;
        }
    };

    let mut cpal_input_device: Option<cpal::Device> = None;
    match input_device {
        DeviceType::File(_f) => {
            MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert().unwrap();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_input_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("input"))?; }
    }

    let mut cpal_output_device: Option<cpal::Device> = None;
    match output_device {
        DeviceType::File(_f) => {
            MessageDialog::new().set_type(native_dialog::MessageType::Error).set_title("FakeYou Recast Error").set_text("Cannot use files for realtime.").show_alert().unwrap();
        },
        DeviceType::Device(cpal_dev) => {
            cpal_output_device = Some(cpal_dev);
        }
        DeviceType::None => { Err(AppError::NoDevice("output"))?; }
    }


    let input_stream = cpal_input_device.ok_or(AppError::NoDevice("input"))?.build_input_stream_raw(&input_config, SampleFormat::F32, input_data_fn, audio::err_fn).map_err(|e| AppError::BuildInputStreamError(e))?;
    let output_stream = cpal_output_device.ok_or(AppError::NoDevice("output"))?.build_output_stream_raw(&output_config, SampleFormat::F32, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e))?;

    input_stream.play().map_err(|e| AppError::PlayStreamError("input", e))?;
    output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
    loop {

        let (realtime_recording, ivol, ovol) = realtime_receiver.recv().map_err(|e| AppError::ChannelRecvError(e))?;
        if realtime_recording {
            //FIXME unsafe
            unsafe { INPUT_VOLUME = ivol };
            unsafe { OUTPUT_VOLUME = ovol };
        } else {
            info!("Stopped realtime recording");
            break
        }
    }
    Ok(())
}

/// Record to a buffer for later conversion (non-realtime)
async fn start_recording(input_device: audio::DeviceType, receiver: std::sync::mpsc::Receiver<(bool, f32)>) -> Result<Vec<f32>, AppError> {
    info!("Recording started from {}", input_device);
    let vec = audio::record_audio_vec(input_device, receiver)?;
    Ok(vec)
}

/// Play back the input buffer
async fn play_source(source: Vec<f32>, output_device: audio::DeviceType) -> Result<(), AppError> {
    info!("Playing source audio on {}", output_device);
    audio::play_audio_buf(output_device, source, unsafe { OUTPUT_VOLUME })
}

// FIXME something in this function is failing on small inputs
// FIXME also something (probably vad) is spamming inference failed but the output seems fine
/// Convert and play back the input buffer with the target voice
async fn play_target(source: Vec<f32>, hubert: softvc::Hubert, acoustic: softvc::Acoustic, hifigan: softvc::Hifigan, vad: vad::Vad, output_device: audio::DeviceType) -> Result<(), AppError> {
    //TODO some kind of loading spinner or something while this is happening
    info!("Running inference and playing target audio on {}", output_device);

    match output_device {
        DeviceType::File((ref f, _)) => {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: MODEL_SAMPLE_RATE as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut writer = hound::WavWriter::new(f, spec).map_err(|e| AppError::HoundError(e))?;
            for chunk in source.chunks(OFFLINE_CHUNK_SIZE) {

                // Find active 50ms chunks with Silero VAD
                let mut vad_confidence: Vec<f32> = Vec::new();
                chunk.chunks(VAD_SIZE).for_each(|vad_chunk| {
                    let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                    let vad_res = vad.forward(&vad_chunk);
                    match vad_res {
                        Err(e) => {
                            handle_errors(Err(e));
                        },
                        Ok(t) => {
                            vad_tensor = t;
                        }
                    }
                    vad_confidence.push(vad_tensor.double_value(&[0]) as f32)
                });

                let mut softvc_input = Vec::new();
                softvc_input.extend_from_slice(chunk);


                let mut out_samples = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone())?;

                // nuke inactive chunks from the buffer
                for v in 0..chunk.len()/VAD_SIZE {
                    if vad_confidence[v] < SPEAKER_THRESHOLD {
                        out_samples[v*VAD_SIZE..(v+1)*VAD_SIZE].fill(0.0);
                    }
                }
                for sample in out_samples.iter() {
                    writer.write_sample(audio::apply_volume(*sample, unsafe { OUTPUT_VOLUME })).map_err(|e| AppError::HoundError(e))?;
                }
            }
            writer.finalize().map_err(|e| AppError::HoundError(e))?;
        },
        DeviceType::Device(ref d) => {
            let ring: RingBuffer<f32> = RingBuffer::new(OFFLINE_RINGBUFFER_SIZE);
            let (mut producer, mut consumer) = ring.split();

            let playback_secs = source.len() as f32 / 16000.0;

            std::thread::spawn(move || {
            for chunk in source.chunks(OFFLINE_CHUNK_SIZE)
            {
            
                // Find active 50ms chunks with Silero VAD
                let mut vad_confidence: Vec<f32> = Vec::new();
                chunk.chunks(VAD_SIZE).for_each(|vad_chunk| {
                    let mut vad_tensor = tch::Tensor::of_slice(&[0.0]);
                    let vad_res = vad.forward(&vad_chunk);
                    match vad_res {
                        Err(e) => {
                            handle_errors(Err(e));
                        },
                        Ok(t) => {
                            vad_tensor = t;
                        }
                    }
                    vad_confidence.push(vad_tensor.double_value(&[0]) as f32)
                });

                let mut softvc_input = Vec::new();
                softvc_input.extend_from_slice(chunk);

                let softvc_out = softvc::inference(&softvc_input, hubert.clone(), acoustic.clone(), hifigan.clone());
                match softvc_out {
                    Ok(samples) => {
                        samples.iter()
                        .enumerate()
                        .map(|(e, sample)| {
                            let mut sa = 0.0;
                            for i in 0..OFFLINE_CHUNK_SIZE/VAD_SIZE {
                                if e < (i+1)*VAD_SIZE && e > i*VAD_SIZE && vad_confidence[i] > SPEAKER_THRESHOLD {
                                    sa = *sample;
                                    break;
                                }
                            }
                            sa
                        })
                        .map(|f| std::iter::repeat(f).take(6)) // upsample to 48khz stereo
                        .flatten()
                        .for_each(|sample| { producer.push(sample).unwrap_or(()); })
                    },
                    Err(e) => {
                        handle_errors(Err(e));
                    }
                }
                std::thread::sleep(core::time::Duration::from_secs_f32(OFFLINE_CHUNK_SECS as f32 - 0.5));
            }
            });
            
            let output_config = cpal::StreamConfig { channels: OUTPUT_CHANNELS as u16, sample_rate: SampleRate(OUTPUT_SAMPLE_RATE as u32), buffer_size: BufferSize::Default };

            let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if consumer.len() >= OFFLINE_RINGBUFFER_SIZE / 2 {
                    for sample in data.iter_mut() {
                        match consumer.pop() {
                            Some(s) => {
                                *sample = audio::apply_volume(s, unsafe { OUTPUT_VOLUME });
                            },
                            None => { *sample = 0.0; }
                        }
                    }
                }
                else { data.fill(0.0); }
            };

            let output_stream = d.build_output_stream(&output_config, output_data_fn, audio::err_fn).map_err(|e| AppError::BuildOutputStreamError(e))?;
            output_stream.play().map_err(|e| AppError::PlayStreamError("output", e))?;
            //add a second in case we fall behind
            std::thread::sleep(core::time::Duration::from_secs_f32(playback_secs + 1.0));
        }
        DeviceType::None => {
            Err(AppError::NoDevice("output"))?
        }
    }
    info!("Finished playing target audio");
    Ok(())
}
