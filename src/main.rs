use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use iced::{executor, Application, Command, Element, Settings, Column, Row, window, Subscription, keyboard::{self, KeyCode}};
use iced::widget::{Slider, slider, Button, button, Text, pick_list, PickList};
use iced_native::{event, subscription, Event};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, BufferSize};
use ringbuf::RingBuffer;
use native_dialog::FileDialog;
use tch::{CModule, Tensor};

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
    model_browse_state: button::State,
    model_browse_selected: Option<PathBuf>,
    hifigan_browse_state: button::State,
    hifigan_browse_selected: Option<PathBuf>,
    hubert_browse_state: button::State,
    hubert_browse_selected: Option<PathBuf>,
    show_debug: bool,
    slider_1_state: slider::State,
    slider_1_value: f32,
    slider_2_state: slider::State,
    slider_2_value: f32,
    record_state: button::State,
    recording: bool,
    record_sender: Option<std::sync::mpsc::Sender<(bool, f32)>>,
    play_target_state: button::State,
    play_source_state: button::State,
    realtime_record_state: button::State,
    realtime_recording: bool,
    realtime_record_sender: Option<std::sync::mpsc::Sender<(bool, f32, f32)>>,
    input_device_list_state: pick_list::State<String>,
    input_device_list_options: Vec<String>,
    input_device_list_selected: Option<String>,
    input_browse_state: button::State,
    input_browse_selected: Option<PathBuf>,
    output_device_list_state: pick_list::State<String>,
    output_device_list_options: Vec<String>,
    output_device_list_selected: Option<String>,
    output_browse_state: button::State,
    output_browse_selected: Option<PathBuf>
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
enum Message {
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
        let model_browse_state = button::State::new();
        let model_browse_selected = None;
        let hifigan_browse_state = button::State::new();
        let hifigan_browse_selected = None;
        let hubert_browse_state = button::State::new();
        let hubert_browse_selected = None;
        let show_debug = false;
        let slider_1_state = slider::State::new();
        let slider_1_value = 80.0;
        let slider_2_state = slider::State::new();
        let slider_2_value = 80.0;
        let record_state = button::State::new();
        let recording = false;
        let record_sender = None;
        let play_target_state = button::State::new();
        let play_source_state = button::State::new();
        let realtime_record_state = button::State::new();
        let realtime_recording = false;
        let realtime_record_sender = None;
        let input_device_list_state = pick_list::State::new();
        let mut input_device_list_options: Vec<String> = cpal::default_host().input_devices().unwrap().map(|d|d.name().unwrap()).collect();
        input_device_list_options.insert(0, String::from("From file"));
        let input_browse_state = button::State::new();
        let input_browse_selected = None;
        let input_device_list_selected = None;
        let output_device_list_state = pick_list::State::new();
        let mut output_device_list_options: Vec<String> = cpal::default_host().output_devices().unwrap().map(|d| d.name().unwrap()).collect();
        output_device_list_options.insert(0, String::from("To file"));
        let output_device_list_selected = None;
        let output_browse_state = button::State::new();
        let output_browse_selected = None;

        (
            App {
                models,
                model_browse_state,
                model_browse_selected,
                hifigan_browse_state,
                hifigan_browse_selected,
                hubert_browse_state,
                hubert_browse_selected,
                show_debug,
                slider_1_state,
                slider_1_value,
                slider_2_state,
                slider_2_value,
                record_state,
                recording,
                record_sender,
                play_target_state,
                play_source_state,
                realtime_record_state,
                realtime_recording,
                realtime_record_sender,
                input_device_list_state,
                input_device_list_options,
                input_device_list_selected,
                input_browse_state,
                input_browse_selected,
                output_device_list_state,
                output_device_list_options,
                output_device_list_selected,
                output_browse_state,
                output_browse_selected,
            },
            Command::none()
        )
    }

    fn title(&self) -> String {
        String::from("FakeYou Recast")
    }

    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        match msg {
            Message::ShowDebugPressed => {
                self.show_debug = !self.show_debug;
            }
            Message::Slider1Changed(new_value) => {
                self.slider_1_value = new_value;
                if self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                }
            }
            Message::Slider2Changed(new_value) => {
                self.slider_2_value = new_value;
                if self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((self.realtime_recording.clone(), self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                }
            }
            // Realtime record button => record and playback at the same time
            Message::RealtimeRecordPressed => {
                if self.realtime_recording {
                    if self.realtime_record_sender.is_some()
                    {
                        self.realtime_record_sender.as_ref().unwrap().send((false, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                    }
                } else {
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.input_device_list_selected.clone();
                    let output_device_name = self.output_device_list_selected.clone(); std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                }

                self.realtime_recording = !self.realtime_recording;
            }
            Message::ModelBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("FakeYou Recast Model", &["recast"])
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        //TODO some kind of loading spinner or something while this is happening
                        tch::maybe_init_cuda();
                        let mut models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_acoustic(path)}
                        );
                        self.model_browse_selected = Some(p.clone());

                        let mut models = self.models.clone();
                        let path = PathBuf::from("./model1.recast_base");
                        std::thread::spawn(move ||{
                            models.load_hifigan(path)}
                        );
                        self.hifigan_browse_selected = Some(PathBuf::from("./model1.recast_base"));

                        let mut models = self.models.clone();
                        std::thread::spawn(move ||{
                            models.load_hubert(PathBuf::from("./model2.recast_base"))}
                        );
                        self.hubert_browse_selected = Some(PathBuf::from("./model2.recast_base"));
                    },
                    None => {}
                }
                //FIXME some kind of progress bar while all this crap loads
            }
            Message::HifiganBrowsePressed => { 
                let path = FileDialog::new()
                    // Hifigan shhhh
                    .add_filter("Base Model 1", &["pt"])
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        let mut models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_hifigan(path)}
                        );
                        self.hifigan_browse_selected = Some(p.clone());
                    },
                    None => {}
                }
            }
            Message::HubertBrowsePressed => { 
                let path = FileDialog::new()
                    // Hubert shhhh
                    .add_filter("Base Model 2", &["pt"])
                    .show_open_single_file()
                    .unwrap();
                match path {
                    Some(p) => {
                        let mut models = self.models.clone();
                        let path = p.clone();
                        std::thread::spawn(move ||{
                            models.load_hubert(path)}
                        );
                        self.hubert_browse_selected = Some(p.clone());
                    },
                    None => {}
                }
            }
            Message::InputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_open_single_file()
                    .unwrap();
                self.input_browse_selected = path;
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter("WAV audio file", &["wav"])
                    .show_save_single_file()
                    .unwrap();
                self.output_browse_selected = path;
            }
            Message::RecordPressed => { 
                if self.recording {
                    if self.record_sender.is_some()
                    {
                        self.record_sender.as_ref().unwrap().send((false, self.slider_1_value.clone())).unwrap();
                    }
                } else {
                    let (record_sender, record_receiver) = std::sync::mpsc::channel();
                    self.record_sender = Some(record_sender);
                    let input_device_name = self.input_device_list_selected.clone();
                    std::thread::spawn(move || { start_recording(record_receiver, input_device_name)} ); 
                    self.record_sender.as_ref().unwrap().send((true, self.slider_1_value.clone())).unwrap();
                }

                self.recording = !self.recording;

            }
            Message::PlayTargetPressed => {
                let output_device_name = self.output_device_list_selected.clone();
                let models = self.models.clone();
                std::thread::spawn(move || {
                    play_target(models, output_device_name)
                });
            }
            Message::PlaySourcePressed => { 
                let output_device_name = self.output_device_list_selected.clone();
                std::thread::spawn(move || {
                    play_source(output_device_name);
                });
            }
            Message::InputDeviceChanged(new_device) => {
                self.input_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                if self.realtime_recording {
                    self.realtime_record_sender.as_ref().unwrap().send((false, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.input_device_list_selected.clone();
                    let output_device_name = self.output_device_list_selected.clone();
                    std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                }
            }
            Message::OutputDeviceChanged(new_device) => {
                self.output_device_list_selected = Some(new_device);
                // Restart recording if currently recording and the device changes
                if self.realtime_recording {
                    self.realtime_record_sender.as_ref().unwrap().send((false, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                    let (realtime_record_sender, realtime_record_receiver) = std::sync::mpsc::channel();
                    self.realtime_record_sender = Some(realtime_record_sender);
                    let input_device_name = self.input_device_list_selected.clone();
                    let output_device_name = self.output_device_list_selected.clone();
                    std::thread::spawn(move || { start_realtime_recording(realtime_record_receiver, input_device_name, output_device_name) }); 
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.slider_1_value.clone(), self.slider_2_value.clone())).unwrap();
                }

            }
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        subscription::events_with(|event, status| {
            if let event::Status::Captured = status {
                return None;
            }

            match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    modifiers,
                    key_code,
                }) => match key_code {
                    KeyCode::F12 => Some(Message::ShowDebugPressed),
                    _ => None,
                }
                _ => None,
            }
        })
    }

    fn view(&mut self) -> Element<Self::Message> {
        let mut model_selection_row_elems: Vec<Element<Self::Message>> = vec![];
        let selected_model_str = if self.model_browse_selected.is_none() { "" } else {self.model_browse_selected.as_ref().unwrap().file_stem().unwrap().to_str().unwrap() };
        let selected_hifigan_str = if self.hifigan_browse_selected.is_none() { "" } else {self.hifigan_browse_selected.as_ref().unwrap().file_stem().unwrap().to_str().unwrap() };
        let selected_hubert_str = if self.hubert_browse_selected.is_none() { "" } else {self.hubert_browse_selected.as_ref().unwrap().file_stem().unwrap().to_str().unwrap() };
        model_selection_row_elems
            .push(Text::new("Select Recast Model File: ").into());
        model_selection_row_elems
            .push(Button::new(&mut self.model_browse_state, Text::new("Browse")).on_press(Message::ModelBrowsePressed).into());
        model_selection_row_elems
            .push(Text::new(selected_model_str).into());
        if self.show_debug {
            model_selection_row_elems
                .push(Text::new("Select Debug Model 1: ").into());
            model_selection_row_elems
                .push(Button::new(&mut self.hifigan_browse_state, Text::new("Browse")).on_press(Message::HifiganBrowsePressed).into());
            model_selection_row_elems
            .push(Text::new(selected_hifigan_str).into());
            model_selection_row_elems
                .push(Text::new("Select Debug Model 2: ").into());
            model_selection_row_elems
                .push(Button::new(&mut self.hubert_browse_state, Text::new("Browse")).on_press(Message::HubertBrowsePressed).into());
            model_selection_row_elems
            .push(Text::new(selected_hubert_str).into());
        }
        let mut model_selection_row = Row::with_children(model_selection_row_elems);

        let mut file_selection_row_elems: Vec<Element<Self::Message>> = vec![];
        if self.input_device_list_selected == Some(String::from("From file"))
        {
            file_selection_row_elems.push(
                Text::new("Input File: ").into()
            );
            file_selection_row_elems.push(
                Button::new(&mut self.input_browse_state, Text::new("Browse")).on_press(Message::InputBrowsePressed).into()
            );
        }
        if self.output_device_list_selected == Some(String::from("To file"))
        {
            file_selection_row_elems.push(
                Text::new("Output File: ").into()
            );
            file_selection_row_elems.push(
                Button::new(&mut self.output_browse_state, Text::new("Browse")).on_press(Message::OutputBrowsePressed).into()
            );
        }
        let mut file_selection_row = Row::with_children(file_selection_row_elems);

        let elements = Column::new()
            .push(model_selection_row)
            .push(
                Row::new()
                .push(Text::new("Input Device: "))
                .push(
                    PickList::new(
                        &mut self.input_device_list_state,
                        &self.input_device_list_options,
                        self.input_device_list_selected.clone(),
                        Message::InputDeviceChanged,
                    )
                )
                .push(Text::new("Output Device: "))
                .push(
                    PickList::new(
                        &mut self.output_device_list_state,
                        &self.output_device_list_options,
                        self.output_device_list_selected.clone(),
                        Message::OutputDeviceChanged,
                    )
                )
            )
            .push(
                file_selection_row
            )
            .push(
                Row::new()
                .push(Text::new("Input Volume: "))
                .push(
                    Slider::new(&mut self.slider_1_state, 0.0..=100.0, self.slider_1_value, Message::Slider1Changed)
                )
                .push(Text::new("Output Volume: "))
                .push(
                    Slider::new(&mut self.slider_2_state, 0.0..=100.0, self.slider_2_value, Message::Slider2Changed)
                )
            )
            .push(
                Row::new()
                .push(
                    Button::new(&mut self.record_state, if self.recording { Text::new("Stop")} else {Text::new("Record")}).on_press(Message::RecordPressed)
                )
                .push(
                    Button::new(&mut self.play_source_state, Text::new("Play Source")).on_press(Message::PlaySourcePressed)
                )
                .push(
                    Button::new(&mut self.play_target_state, Text::new("Play Target")).on_press(Message::PlayTargetPressed)
                )
                .push(
                    Button::new(&mut self.realtime_record_state, if self.realtime_recording { Text::new("Stop") } else {Text::new("Realtime\n(experimental)")}).on_press(Message::RealtimeRecordPressed)
                )

            );

        elements.into()
    }

}

/// Realtime record => record and playback at the same time
fn start_realtime_recording(realtime_record_receiver: std::sync::mpsc::Receiver<(bool, f32, f32)>, input_device_name: Option<String>, output_device_name: Option<String>)  {
    let input_device = get_input_device(input_device_name).0.unwrap();
    let output_device = get_output_device(output_device_name).0.unwrap();

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
            producer.push(apply_volume(sample, unsafe { INPUT_VOLUME })).unwrap();
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        // Expand one sample to 2 to convert 48khz mono to 48khz stereo
        let mut iter = data.iter_mut(); 
        let mut s: f32 = 0.0;
        for (j, sample) in data.iter_mut().enumerate() {
            if (j%2==0)
            {
                match consumer.pop() {
                    Some(sa) => s = sa,
                    None => s = 0.0,
                }
            }
            *sample = apply_volume(s, unsafe { OUTPUT_VOLUME});
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
    let input_device = get_input_device(input_device_name).0.unwrap();
    let input_config = cpal::StreamConfig { channels: 1, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };
    unsafe { RECORD_BUF = [0f32; 319507] };
    unsafe { RECORD_SAMPLE_COUNT = 0 };
    let mut i = 0;
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        for (j, &sample) in data.iter().enumerate() {
            // Drop 2/3 of the samples to make 16KHz
            if j % 3== 0 {
                //FIXME unsafe
               unsafe{ RECORD_BUF[i] = apply_volume(sample, INPUT_VOLUME ) };
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

fn play_source(output_device_name: Option<String>) {
    let output_device = get_output_device(output_device_name).0.unwrap();
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };

    let mut i = 0;
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut iter = data.iter_mut(); 
        for (j, sample) in data.iter_mut().enumerate() {
            let s = unsafe { RECORD_BUF[i] };
            *sample = apply_volume(s, unsafe { OUTPUT_VOLUME});
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
    std::thread::sleep(core::time::Duration::from_secs(19));
}

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
    let mut transpose_output = generate_output.transpose(1, 2);
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
        let mut iter = data.iter_mut(); 
        for (j, sample) in data.iter_mut().enumerate() {
            let s = sample_buf[i];
            *sample = apply_volume(s, unsafe { OUTPUT_VOLUME});
            // expand one sample to 6 to convert 16khz mono to 48khz stereo
            if j%6 == 5 && i < unsafe { RECORD_SAMPLE_COUNT } as usize
            {
                i += 1;
            }
        }
    };

    let output_device = get_output_device(output_device_name).0.unwrap();
    let output_config = cpal::StreamConfig { channels: 2, sample_rate: SampleRate(48000), buffer_size: BufferSize::Default };

    let output_stream = output_device.build_output_stream(&output_config, output_data_fn, err_fn).unwrap();

    output_stream.play().unwrap();
    // FIXME ¯\_(ツ)_/¯
    std::thread::sleep(core::time::Duration::from_secs_f32(unsafe { RECORD_SAMPLE_COUNT } as f32 / 16000.0 - 0.3));
}

fn get_input_device(input_device_name: Option<String>) -> (Option<cpal::Device>, Option<File>) {
    let mut input_device: Option<cpal::Device> = None;
    let mut file: Option<std::fs::File> = None;
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

fn get_output_device(output_device_name: Option<String>) -> (Option<cpal::Device>, Option<File>) {
    let mut output_device: Option<cpal::Device> = None;
    let mut file: Option<std::fs::File> = None;
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

fn apply_volume(sample: f32, volume: f32) -> f32 {
    let db = (-90.0) + (30.0 - (-90.0)) * ( volume  / 100.0);
    let mut scale = (db/20.0 * (30.0f32).log10()).exp();
    if volume == 0.0 { scale = 0.0 };
    sample * scale
}


fn err_fn(err: cpal::StreamError) {
    eprintln!("Error: {:?}", err);
}


