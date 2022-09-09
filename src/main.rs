use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use iced::{executor, Application, Element, Command, Settings, window, Subscription, keyboard::{self, KeyCode}};
use iced_native::{event, subscription, Event};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, BufferSize};
use ringbuf::RingBuffer;
use native_dialog::FileDialog;
use tch::{CModule, Tensor};

mod view;
mod audio;

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


//FIXME unsafe
static mut RECORD_BUF: [f32; 319507] = [0.0; 319507];
static mut RECORD_SAMPLE_COUNT: usize = 0;
static mut INPUT_VOLUME: f32 = 80.0;
static mut OUTPUT_VOLUME: f32 = 80.0;

struct App {
    models: Models,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    record_sender: Option<std::sync::mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<std::sync::mpsc::Sender<(bool, f32, f32)>>,
    recording: bool,
    realtime_recording: bool,
}

#[derive(Clone)]
struct Models {
    // FIXME provide getter functions and unpub these
    pub hubert_model: Arc<Mutex<Option<CModule>>>,
    pub acoustic_model: Arc<Mutex<Option<CModule>>>,
    pub hifigan_model: Arc<Mutex<Option<CModule>>>,
}

impl Models {
    fn new() -> Models {
        Models {
            hubert_model: Arc::new(Mutex::new(None)),
            acoustic_model: Arc::new(Mutex::new(None)),
            hifigan_model: Arc::new(Mutex::new(None)),
        }
    }
    fn load_hubert(&self, path: PathBuf) 
    {
        let mut lock = (*self.hubert_model).lock().unwrap();
        let mut hubert = CModule::load(path).unwrap();
        hubert.set_eval();
        *lock = Some(hubert);
    }
    fn load_hifigan(&self, path: PathBuf) 
    {
        let mut lock = (*self.hifigan_model).lock().unwrap();
        let mut hifigan = CModule::load(path).unwrap();
        hifigan.set_eval();
        *lock = Some(hifigan);

    }
    fn load_acoustic(&self, path: PathBuf) 
    {
        let mut lock = (*self.acoustic_model).lock().unwrap();
        let mut acoustic = CModule::load(path).unwrap();
        acoustic.set_eval();
        *lock = Some(acoustic);
    }
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
}

impl Application for App {
    type Executor = executor::Default;
    type Message = crate::Message;
    type Flags = ();

    fn new(_flags: ()) -> (App, Command<Self::Message>) {
        let models = Models::new();
        let view_state = view::ViewState::new();
        let recording = false;
        let record_sender = None;
        let realtime_record_sender = None;
        let realtime_recording = false;
        let mut input_device_list_options: Vec<String> = cpal::default_host().input_devices().unwrap().map(|d|d.name().unwrap()).collect();
        input_device_list_options.insert(0, String::from("From file"));
        let mut output_device_list_options: Vec<String> = cpal::default_host().output_devices().unwrap().map(|d| d.name().unwrap()).collect();
        output_device_list_options.insert(0, String::from("To file"));

        tch::maybe_init_cuda();

        (
            App {
                models,
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

    fn title(&self) -> String {
        String::from("FakeYou Recast")
    }

    fn view(&mut self) -> Element<Message> { view::view(&mut self.view_state, &self.input_device_list_options, &self.output_device_list_options, self.recording, self.realtime_recording) }

    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        match msg {
            Message::ShowDebugPressed => {
                self.view_state.show_debug = !self.view_state.show_debug;
            }
            Message::Slider1Changed(new_value) => {
                self.view_state.slider_1_value = new_value;
                if self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            Message::Slider2Changed(new_value) => {
                self.view_state.slider_2_value = new_value;
                if self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            // Realtime record button => record and playback at the same time
            Message::RealtimeRecordPressed => {
                if self.realtime_recording {
                    if self.realtime_record_sender.is_some()
                    {
                        self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    }
                } else {
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone(); std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }

                self.realtime_recording = !self.realtime_recording;
            }
            Message::ModelBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("FakeYou Recast Model", &["recast"])
                    .set_location("./models")
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        //TODO some kind of loading spinner or something while this is happening
                        let models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_acoustic(path)}
                        );
                        self.view_state.model_browse_selected = Some(p.clone());

                        let models = self.models.clone();
                        std::thread::spawn(move || {
                            models.load_hifigan(PathBuf::from("./models/model1.recast_base"))
                        });
                        self.view_state.hifigan_browse_selected = Some(PathBuf::from("./models/model1.recast_base"));

                        let models = self.models.clone();
                        std::thread::spawn(move ||{
                            models.load_hubert(PathBuf::from("./models/model2.recast_base"))}
                        );
                        self.view_state.hubert_browse_selected = Some(PathBuf::from("./models/model2.recast_base"));
                    },
                    None => {}
                }
                //FIXME some kind of progress bar while all this crap loads
            }
            Message::HifiganBrowsePressed => { 
                let path = FileDialog::new()
                    // Hifigan shhhh
                    .add_filter("Base Model 1", &["pt"])
                    .set_location("./models")
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        let models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_hifigan(path)}
                        );
                        self.view_state.hifigan_browse_selected = Some(p.clone());
                    },
                    None => {}
                }
            }
            Message::HubertBrowsePressed => { 
                let path = FileDialog::new()
                    // Hubert shhhh
                    .add_filter("Base Model 2", &["pt"])
                    .set_location("./models")
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        let models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_hubert(path)}
                        );
                        self.view_state.hubert_browse_selected = Some(p.clone());
                    },
                    None => {}
                }
            }
            Message::InputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_open_single_file()
                    .unwrap();
                self.view_state.input_browse_selected = path;
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_save_single_file()
                    .unwrap();
                self.view_state.output_browse_selected = path;
            }
            Message::RecordPressed => { 
                if self.recording {
                    if self.record_sender.is_some()
                    {
                        self.record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone())).unwrap();
                    }
                } else {
                    let (record_sender, record_receiver) = std::sync::mpsc::channel();
                    self.record_sender = Some(record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    std::thread::spawn(move || { start_recording(record_receiver, input_device_name)} ); 
                    self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone())).unwrap();
                }

                self.recording = !self.recording;

            }
            Message::PlayTargetPressed => {
                let output_device_name = self.view_state.output_device_list_selected.clone();
                let models = self.models.clone();
                std::thread::spawn(move || {
                    play_target(models, output_device_name)
                });
            }
            Message::PlaySourcePressed => { 
                let output_device_name = self.view_state.output_device_list_selected.clone();
                std::thread::spawn(move || {
                    play_source(output_device_name);
                });
            }
            Message::InputDeviceChanged(new_device) => {
                self.view_state.input_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                if self.realtime_recording {
                    self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            Message::OutputDeviceChanged(new_device) => {
                self.view_state.output_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                if self.realtime_recording {
                    self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.view_state.input_device_list_selected.clone();
                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }

            }
        }
        Command::none()
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

/// Realtime record => record and playback at the same time
fn start_realtime_recording(realtime_record_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device_name: Option<String>, output_device_name: Option<String>)  {
    let input_device = audio::get_input_device(input_device_name).0.unwrap();
    let output_device = audio::get_output_device(output_device_name).0.unwrap();

    // Windows is bad and won't let me request 16KHz input so we'll have to do 48 and drop 2/3 when
    // we pass to the 16khz model
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
    let ring = RingBuffer::new(100_000);
    let (mut producer, mut consumer) = ring.split();
    

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        for &sample in data {
            // Don't forget to drop 2/3 of the samples when you go to run the algo on this
            //FIXME unsafe
            producer.push(audio::apply_volume(sample, unsafe { INPUT_VOLUME })).unwrap();
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        // Expand one sample to 2 to convert 48khz mono to 48khz stereo
        let mut s: f32 = 0.0;
        for (j, sample) in data.iter_mut().enumerate() {
            if j%2==0
            {
                match consumer.pop() {
                    Some(sa) => s = sa,
                    None => s = 0.0,
                }
            }
            *sample = audio::apply_volume(s, unsafe { OUTPUT_VOLUME});
        }
    };

    let input_stream = input_device.build_input_stream(&input_config, input_data_fn, err_fn).unwrap();
    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, err_fn).unwrap();

    input_stream.play().unwrap();
    output_stream.play().unwrap();
    loop {
        let (realtime_recording, ivol, ovol) = realtime_record_receiver.recv().unwrap();
        if realtime_recording {
            //FIXME unsafe
            unsafe { INPUT_VOLUME = ivol };
            unsafe { OUTPUT_VOLUME = ovol };
            std::thread::sleep(std::time::Duration::from_millis(50));
        } else {
            break
        }
    }
}

/// Record to a buffer for later conversion (non-realtime)
fn start_recording(record_receiver: std::sync::mpsc::Receiver<(bool, f32)>, input_device_name: Option<String>)  {
    let input_device = audio::get_input_device(input_device_name).0.unwrap();
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
    unsafe { RECORD_BUF = [0f32; 319507] };
    unsafe { RECORD_SAMPLE_COUNT = 0 };
    let mut i = 0;
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        for (j, &sample) in data.iter().enumerate() {
            // Drop 2/3 of the samples to make 16KHz
            if j % 3== 0 {
                //FIXME unsafe
               unsafe{ RECORD_BUF[i] = audio::apply_volume(sample, INPUT_VOLUME ) };
               unsafe { RECORD_SAMPLE_COUNT = i};
               i += 1;
               //FIXME overflow
           }
        }
    };

    let input_stream = input_device.build_input_stream(&input_config, input_data_fn, err_fn).unwrap();

    input_stream.play().unwrap();
    loop {
        let (recording, ivol) = record_receiver.recv().unwrap();
        if recording {
            //FIXME unsafe
            unsafe { INPUT_VOLUME = ivol };
            std::thread::sleep(std::time::Duration::from_millis(50));
        } else {
            break;
        }
    }
}

/// Play back the input buffer
fn play_source(output_device_name: Option<String>) {
    let output_device = audio::get_output_device(output_device_name).0.unwrap();
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };

    let mut i = 0;
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        for (j, sample) in data.iter_mut().enumerate() {
            let s = unsafe { RECORD_BUF[i] };
            *sample = audio::apply_volume(s, unsafe { OUTPUT_VOLUME});
            // Expand one sample to 6 to convert 16khz mono to 48khz stereo
            if j%6 == 5 && i < 319507
            {
                i += 1;
            }
        }
    };

    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, err_fn).unwrap();

    output_stream.play().unwrap();
    // FIXME ¯\_(ツ)_/¯
    // the goal of this is to stop playback before we run out of samples but it's super kludgey
    std::thread::sleep(core::time::Duration::from_secs(19));
}

/// Convert and play back the input buffer with the target voice
/// Still undecided if we should separate the conversion to another button
/// If the conversion is fast enough, we don't need to worry too much about
/// redoing it when the user wants to hear the same input twice.
/// If we are too slow, it could be a nice time saver for that case though.
fn play_target(models: Models, output_device_name: Option<String>) {
    //TODO some kind of loading spinner or something while this is happening
    let mut wav_data = unsafe { RECORD_BUF.to_vec() };
    wav_data.resize(319507, 0.0);
    let tensor = Tensor::of_slice(wav_data.as_slice());
    tch::maybe_init_cuda();
    let wav_tensor = tensor.unsqueeze(0).unsqueeze(0).to(tch::Device::Cuda(0));
    if cfg!(debug_assertions) {
        println!("wav: {:?}", wav_tensor.size());
    }
    tch::jit::set_graph_executor_optimize(false);
    let hubert_start_time = std::time::Instant::now();
    let hubert_output = {
        let hubert_model = models.hubert_model.lock().unwrap();
        hubert_model.as_ref().unwrap().method_ts("units", &[wav_tensor]).unwrap()
    };
    let hubert_end_time = std::time::Instant::now();
    if cfg!(debug_assertions) {
        println!("hubert: {:?}", hubert_output.size());
    }
    let generate_start_time = std::time::Instant::now();
    let generate_output = {
        let acoustic_model = models.acoustic_model.lock().unwrap();
        acoustic_model.as_ref().unwrap().method_ts("generate", &[hubert_output]).unwrap()
    };
    let generate_end_time = std::time::Instant::now();
    if cfg!(debug_assertions) {
        println!("generate: {:?}", generate_output.size());
    }
    let transpose_output = generate_output.transpose(1, 2);
    if cfg!(debug_assertions) {
        println!("transpose: {:?}", transpose_output.size());
    }

    let hifigan_start_time = std::time::Instant::now();
    let hifigan_output = tch::no_grad(move|| {
        let hifigan_model = models.hifigan_model.lock().unwrap();
        hifigan_model.as_ref().unwrap().forward_ts(&[transpose_output]).unwrap()
    });
    let hifigan_end_time = std::time::Instant::now();

    if cfg!(debug_assertions) {
        println!("hifigan: {:?}", hifigan_output.size());
    }

    //if cfg!(debug_assertions) {
        println!("hubert: {:?}", hubert_end_time - hubert_start_time);
        println!("generator: {:?}", generate_end_time - generate_start_time);
        println!("hifigan: {:?}", hifigan_end_time - hifigan_start_time);
    //}

    let sample_count = hifigan_output.size()[2];
    let samples = hifigan_output.reshape(&[sample_count]).to(tch::Device::Cpu);
    // RECORD_SAMPLE_COUNT is used instead of sample_count because the input
    // tensor is padded to 20 seconds and the model has a stroke if the user
    // stops recording before then and sample_count is used.
    let mut sample_buf = vec![0.0; unsafe { RECORD_SAMPLE_COUNT }];
    samples.copy_data(&mut sample_buf, unsafe { RECORD_SAMPLE_COUNT });

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create("test.wav", spec).unwrap();
    for sample in sample_buf.iter() {
        writer.write_sample(*sample).unwrap();
    }

    let mut i = 0;
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        for (j, sample) in data.iter_mut().enumerate() {
            let s = sample_buf[i];
            *sample = audio::apply_volume(s, unsafe { OUTPUT_VOLUME});
            // expand one sample to 6 to convert 16khz mono to 48khz stereo
            if j%6 == 5 && i < unsafe { RECORD_SAMPLE_COUNT } as usize
            {
                i += 1;
            }
        }
    };

    let output_device = audio::get_output_device(output_device_name).0.unwrap();
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };

    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, err_fn).unwrap();

    output_stream.play().unwrap();
    // FIXME ¯\_(ツ)_/¯
    // the goal of this is to stop playback before we run out of samples but it's super kludgey
    std::thread::sleep(core::time::Duration::from_secs_f32(unsafe { RECORD_SAMPLE_COUNT } as f32 / 16000.0 - 0.3));
}


fn err_fn(err: cpal::StreamError) {
    eprintln!("Error: {:?}", err);
}
