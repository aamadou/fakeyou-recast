// launch as a gui app on windows without a command prompt
#![windows_subsystem = "windows"]

use std::{path::PathBuf, env, io::Write, sync::mpsc}; 

use iced::{executor, Application, Element, Command, Settings, window::{self, Icon}, Subscription, Color, time};
use log::{info, error};
use model_serde::serde::deserializer::ModelDeserializer;
use native_dialog::{FileDialog, MessageDialog};
use rust_i18n::t;
use rust_i18n::i18n;

i18n!("../../../assets/locales");

mod api;
mod audio;
mod consts;
mod discord;
mod softvc;
mod view;
mod spinner;
mod error;
mod app_state;
mod audio_pipelines;

use consts::*;
use crate::discord::spawn_discord_thread;
use softvc::Model;
use api::ApiError; // should this be in the error module?
use app_state::AppState;
use error::AppError;

use audio_pipelines::{play_source::play_source, play_target::{play_target, play_target_cpu}, realtime::start_realtime, record::start_recording};

const ICON: &[u8; 262144] = include_bytes!("../../../../assets/icon.data");

pub fn main() -> iced::Result {
    let _ = init_logging(); //nowhere to log an error if one happens here

    spawn_discord_thread();

    let settings = Settings {
        window: window::Settings {
            size: (WINDOW_WIDTH, WINDOW_HEIGHT),
            resizable: false,
            //decorations: false, // this removes the menubar
            icon: Some(Icon::from_rgba(ICON.to_vec(), 256, 256).unwrap()),
            ..Default::default()
        },
        default_font: Some(&view::style::BOLD_FONT),
        default_text_size: 14,
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
        .level_for("fakeyou_recast::audio_pipelines", log::LevelFilter::Info) // NB do we need to do this?
                                                                              
        .level_for("fakeyou_recast::audio", log::LevelFilter::Info)
        //.level_for("fakeyou_recast::softvc", log::LevelFilter::Info)
        //.level_for("fakeyou_recast::vad", log::LevelFilter::Info)   // commented out because I realized this leaks our
                                                                      //model architecture (as a string), hopefully we don't need
                                                                      //it for logging to work well
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

struct App {
    /// Directory the application is launched from / running in.
    current_directory: PathBuf,
    /// Utility that handles model file loading and decryption.
    model_deserializer: ModelDeserializer,
    hubert: softvc::Hubert,
    acoustic: softvc::Acoustic,
    hifigan: softvc::Hifigan,
    view_state: view::ViewState,
    input_device_list_options: Vec<String>,
    output_device_list_options: Vec<String>,
    //NB this will be used for the api when we download models probably
    #[allow(unused)]
    session_token: Option<String>, 
    userdata: Option<json::JsonValue>,
    record_sender: Option<mpsc::Sender<(bool, f32)>>,
    realtime_record_sender: Option<mpsc::Sender<(bool, f32, f32)>>,
    play_target_sender: Option<mpsc::Sender<(bool, f32)>>,
    play_source_sender: Option<mpsc::Sender<(bool, f32)>>,
    cpu_mode: bool,
    offline_mode: bool,
    state: AppState,
    record_vec: RecordVector,
    spinner_frames: Vec<Vec<u8>>
}

// FIXME we should probably get rid of this and just use a vec
// at one point I was trying to encapsulate some fancier stuff
// for shared mutability but it didn't work out
#[derive(Debug, Clone)]
struct RecordVector(Vec<f32>);
impl RecordVector {
    pub fn new() -> RecordVector {
        Self(Vec::new())
    }
    pub fn set_contents(&mut self, inner: Vec<f32>) {
        self.0 = inner;
    }
    pub fn get_contents(&self) -> Vec<f32> {
        self.0.clone()
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    FakeYouModelBrowsePressed,
    #[cfg(feature="devmode")]
    ModelBrowsePressed,
    #[cfg(feature="devmode")]
    HifiganBrowsePressed,
    #[cfg(feature="devmode")]
    HubertBrowsePressed,
    Slider1Changed(f32),
    Slider2Changed(f32),
    ChunkSizeSliderChanged(f32),
    CheckboxToggled(bool),
    RecordPressed,
    PlayTargetPressed,
    PlaySourcePressed,
    RealtimeRecordPressed,
    InputDeviceChanged(String),
    OutputDeviceChanged(String),
    InputBrowsePressed,
    OutputBrowsePressed,
    RecordingFinished(Vec<f32>),
    UsernameChanged(String),
    PasswordChanged(String),
    LoginPressed,
    LoginSuccess(String),
    LoginFailed(ApiError),
    SessionValid(json::JsonValue),
    SessionFailed(ApiError),
    SpinnerNextFrame(std::time::Instant),
    TabChanged(usize),
    Done
}

fn handle_errors(res: Result<(), AppError>) -> Message {
    if let Some(e) = res.err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error)
            .set_title((t!("app_name") + t!("misc.error").as_str()).as_str())
            .set_text(&format!("{}", e)).show_alert().unwrap();
    }
    Message::Done
}

fn handle_recording(res: Result<Vec<f32>, AppError>) -> Message {
    if let Some(e) = res.as_ref().err() {
        error!("{}", e);
        MessageDialog::new().set_type(native_dialog::MessageType::Error)
            .set_title((t!("app_name") + t!("misc.error").as_str()).as_str())
            .set_text(&format!("{}", e)).show_alert().unwrap();
    } else {
        let record_vec = res.unwrap();
        return Message::RecordingFinished(record_vec);
    }
    Message::Done
}

impl Application for App {
    type Executor = executor::Default;
    type Message = crate::Message;
    type Flags = ();

    /// App initialization
    fn new(_flags: ()) -> (App, Command<Self::Message>) {
        let hubert = softvc::Hubert::new();
        let acoustic = softvc::Acoustic::new();
        let hifigan = softvc::Hifigan::new();
        let view_state = view::ViewState::new();
        let record_sender = None;
        let realtime_record_sender = None;
        let play_target_sender = None;
        let play_source_sender = None;
        let mut input_device_list_options = audio::get_input_device_list();
        input_device_list_options.insert(0, t!("main_ui.list_item.from_file"));
        let mut output_device_list_options: Vec<String> = audio::get_output_device_list();
        output_device_list_options.insert(0, t!("main_ui.list_item.to_file"));
        let mut cpu_mode = false;
        let offline_mode = false;
        let state = AppState::Idle;

        let spinner_frames = vec![
            spinner::FRAME1.to_vec(),
            spinner::FRAME2.to_vec(),
            spinner::FRAME3.to_vec(),
            spinner::FRAME4.to_vec(),
            spinner::FRAME5.to_vec(),
            spinner::FRAME6.to_vec(),
            spinner::FRAME7.to_vec(),
            spinner::FRAME8.to_vec(),
            spinner::FRAME9.to_vec(),
            spinner::FRAME10.to_vec(),
            spinner::FRAME11.to_vec(),
            spinner::FRAME12.to_vec(),
            spinner::FRAME13.to_vec(),
            spinner::FRAME14.to_vec(),
            spinner::FRAME15.to_vec(),
            spinner::FRAME16.to_vec(),
            spinner::FRAME17.to_vec(),
            spinner::FRAME18.to_vec(),
            spinner::FRAME19.to_vec(),
            spinner::FRAME20.to_vec(),
            spinner::FRAME21.to_vec(),
            spinner::FRAME22.to_vec(),
            spinner::FRAME23.to_vec(),
            spinner::FRAME24.to_vec(),
            spinner::FRAME25.to_vec(),
            spinner::FRAME26.to_vec(),
            spinner::FRAME27.to_vec(),
            spinner::FRAME28.to_vec(),
            spinner::FRAME29.to_vec(),
            spinner::FRAME30.to_vec(),
        ];

        let mut session_token = None;
        let userdata = None;
        if let Ok(string) = std::fs::read_to_string(".sessiontoken") {
            session_token = Some(string);
        }

        let record_vec = RecordVector::new();

        #[cfg(not(feature="devmode"))]
        view_state.login_modal_state.show(true);

        tch::maybe_init_cuda();
        if !tch::Cuda::is_available() {
            handle_errors(Err(AppError::NoCuda));
            cpu_mode = true;
        }

        let command;
        if let Some(token) =  session_token.clone() {
           command = Command::perform(api::check_session(token), api::session_callback);
        }
        else {
            command = Command::none();
        }

        let launch_location = env::current_dir()
            .map_err(|err| error!("{}", t!("log.error.startup_dir", error = err.to_string().as_str())))
            .unwrap_or(PathBuf::from(""));

        (
            App {
                current_directory: launch_location,
                model_deserializer: ModelDeserializer::new("FAKEYOU_DEVELOPMENT"), // TODO: Real secrets.
                hubert,
                acoustic,
                hifigan,
                view_state,
                input_device_list_options,
                output_device_list_options,
                session_token,
                userdata,
                record_sender,
                realtime_record_sender,
                play_target_sender,
                play_source_sender,
                state,
                record_vec,
                cpu_mode,
                offline_mode,
                spinner_frames,
            },
            command
        )
    }

    /// Controls the title bar of the application window
    fn title(&self) -> String {
        t!("app_name")
    }

    fn background_color(&self) -> Color {
        Color::new(26.0, 26.0, 39.0, 1.0)
    }

    /// UI setup and state changes
    fn view(&mut self) -> Element<Message> { view::view(&mut self.view_state, &self.input_device_list_options, &self.output_device_list_options, self.state, self.cpu_mode.clone()) }

    /// Respond to messages from the UI with appropriate actions
    fn update(&mut self, msg: Self::Message) -> Command<Self::Message> {
        let mut command: Command<Self::Message> = Command::none();
        match msg {
            Message::Slider1Changed(new_value) => {
                self.view_state.slider_1_value = new_value;
                //FIXME unsafe and stupid
                unsafe { INPUT_VOLUME = new_value };
                if self.state == AppState::Realtime && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
                if self.state == AppState::Recording && self.record_sender.is_some() {
                    self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone())).unwrap();
                }
            }
            Message::Slider2Changed(new_value) => {
                self.view_state.slider_2_value = new_value;
                //FIXME unsafe and stupid
                unsafe { OUTPUT_VOLUME  = new_value };
                if self.state == AppState::Realtime && self.realtime_record_sender.is_some() {
                    self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone())).unwrap();
                }
                if self.state == AppState::PlayTarget && self.play_target_sender.is_some() {
                    self.play_target_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                }
                if self.state == AppState::PlaySource && self.play_source_sender.is_some() {
                    self.play_source_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                }
            }
            Message::ChunkSizeSliderChanged(new_value) => {
                self.view_state.chunk_size_slider_value = new_value;
            }
            Message::CheckboxToggled(new_value) => {
                self.view_state.duplicate_5x = new_value;
            }
            // Realtime record button => record and playback at the same time
            Message::RealtimeRecordPressed => {
                if self.state == AppState::Realtime {
                    if self.realtime_record_sender.is_some()
                    {
                        let res = self.realtime_record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                           error!("{}", e); 
                        }
                        self.state = AppState::Idle;
                    }
                } else {
                    self.view_state.show_spinner = true;
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
                    command = Command::perform(start_realtime(realtime_record_receiver, input_device, output_device, hubert, acoustic, hifigan, self.view_state.chunk_size_slider_value, self.view_state.duplicate_5x), handle_errors);
                    let res = self.realtime_record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone(), self.view_state.slider_2_value.clone()));
                    if let Some(e) = res.err() {
                        error!("{}", e);
                    }

                    self.state = AppState::Realtime;
                }
            }
            Message::FakeYouModelBrowsePressed => {
                match self.handle_fakeyou_model_browse_pressed() {
                    Err(_e) => {
                        handle_errors(Err(AppError::FileDialogError(self.current_directory.clone())));
                    }
                    Ok(_) => {}
                }
            }
            #[cfg(feature="devmode")]
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
                            info!("{}", t!("log.info.opening_model", file =  &p.clone().file_name().unwrap_or_default().to_string_lossy()));
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
                        },
                        Ok(None) => {},
                        Err(_e) => { handle_errors(Err(AppError::FileDialogError(location))); }
                    }
                    //FIXME some kind of progress bar while all this crap loads
                    // I need to change these functions to async instead of this
                    // random thread crap to be able to tell when they're finished
                }
            }
            #[cfg(feature="devmode")]
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
            #[cfg(feature="devmode")]
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
                    .add_filter(t!("file_dialog.wav").as_str(), &["wav"])
                    .show_open_single_file();
                match path {
                    Ok(i) => {
                        info!("{}", t!("log.info.in_sel_file", 
                                file = i.clone().unwrap_or_default().file_name().unwrap_or_default().to_str().unwrap_or_default()
                            )
                        );
                        self.view_state.input_browse_selected = i;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::OutputBrowsePressed => { 
                let path = FileDialog::new()
                    .add_filter(t!("file_dialog.wav").as_str(), &["wav"])
                    .set_filename("output.wav")
                    .show_save_single_file();
                match path {
                    Ok(o) => {
                        info!("{}", t!("log.info.out_sel_file", 
                                file = o.clone().unwrap_or_default().file_name().unwrap_or_default().to_str().unwrap_or_default()
                            )
                        );
                        self.view_state.output_browse_selected = o;
                    },
                    Err(_e) => { handle_errors(Err(AppError::FileDialogError(PathBuf::from("")))); }
                }
            }
            Message::RecordPressed => { 
                let input_device_name = self.view_state.input_device_list_selected.clone();
                if self.state == AppState::Recording || self.state == AppState::ReadingFile {
                    if self.record_sender.is_some()
                    {
                        let res = self.record_sender.as_ref().unwrap().send((false, self.view_state.slider_1_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                    }
                    self.state = AppState::Idle;
                } else {
                    self.view_state.show_spinner = true;
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
                    if self.record_sender.is_some() {
                        if input_device_name != Some(t!("main_ui.list_item.from_file")) {
                            let _ = self.record_sender.as_ref().unwrap().send((true, self.view_state.slider_1_value.clone()));
                            self.state = AppState::Recording;
                        } else {
                            self.state = AppState::ReadingFile;
                        }
                    }
                }


            }
            Message::PlayTargetPressed => {
                if self.state == AppState::PlayTarget || self.state == AppState::WritingFile {
                    if self.play_target_sender.is_some()
                    {
                        let res = self.play_target_sender.as_ref().unwrap().send((false, self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                    }
                    self.state = AppState::Idle;
                } else {
                    let (play_target_sender, play_target_receiver) = std::sync::mpsc::channel();
                    self.play_target_sender = Some(play_target_sender);
                    self.view_state.show_spinner = true;

                    let output_device_name = self.view_state.output_device_list_selected.clone();
                    let output_file = self.view_state.output_browse_selected.clone();
                    let hubert = self.hubert.clone();
                    let acoustic = self.acoustic.clone();
                    let hifigan = self.hifigan.clone();

                    let output_device_res = audio::get_output_device(output_device_name.clone(), output_file);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };
                    if self.cpu_mode {
                        command = Command::perform(play_target_cpu(self.record_vec.get_contents(), hubert, acoustic, hifigan, output_device, play_target_receiver), handle_errors);
                    } else {
                        command = Command::perform(play_target(self.record_vec.get_contents(), hubert, acoustic, hifigan, output_device, play_target_receiver), handle_errors);
                    }

                    if  self.play_target_sender.is_some() {
                        if output_device_name != Some(t!("main_ui.list_item.to_file")) {
                            self.play_target_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                            self.state = AppState::PlayTarget;
                        }
                        else {
                            self.state = AppState::WritingFile;
                        }
                    }
                }
            }
            Message::PlaySourcePressed => { 
                if self.state == AppState::PlaySource {
                    if self.play_source_sender.is_some()
                    {
                        let res = self.play_source_sender.as_ref().unwrap().send((false, self.view_state.slider_2_value.clone()));
                        if let Some(e) = res.err() {
                            error!("{}", e);
                        }
                    }
                    self.state = AppState::Idle;
                } else {
                    let (play_source_sender, play_source_receiver) = std::sync::mpsc::channel();
                    self.play_source_sender = Some(play_source_sender);
                    self.view_state.show_spinner = true;
                    let output_device_name = self.view_state.output_device_list_selected.clone();

                    // pass None for file because playing source to file is pointless and confusing
                    let output_device_res = audio::get_output_device(output_device_name, None);
                    let mut output_device = audio::DeviceType::None;
                    match output_device_res {
                        Err(e) => { handle_errors(Err(e)); },
                        Ok(out) => { output_device = out; }
                    };
                    command = Command::perform(play_source(self.record_vec.get_contents(), output_device, play_source_receiver), handle_errors);

                    if self.play_source_sender.is_some() {
                        self.play_source_sender.as_ref().unwrap().send((true, self.view_state.slider_2_value.clone())).unwrap();
                    }
                    self.state = AppState::PlaySource;
                }
            }
            Message::InputDeviceChanged(new_device) => {
                info!("{}", t!("log.info.in_sel_dev", dev = new_device.to_string().as_str()));
                self.view_state.input_device_list_selected = Some(new_device);
            }
            Message::OutputDeviceChanged(new_device) => {
                info!("{}", t!("log.info.out_sel_dev", dev = new_device.to_string().as_str()));
                self.view_state.output_device_list_selected = Some(new_device);
            }

            Message::RecordingFinished(vec) => {
                self.state = AppState::Idle;
                self.view_state.show_spinner = false;
                self.record_vec.set_contents(vec);
            }
            Message::UsernameChanged(username) => {
                self.view_state.login_modal_state.inner_mut().username_field_value = username;
            }
            Message::PasswordChanged(password) => {
                self.view_state.login_modal_state.inner_mut().password_field_value = password;
            }
            Message::LoginPressed => {
                let username = self.view_state.login_modal_state.inner().username_field_value.clone();
                let password = self.view_state.login_modal_state.inner().password_field_value.clone();

                command = Command::perform(api::login(username, password), api::login_callback);
            }
            Message::LoginSuccess(token) => {
                match std::fs::File::create(".sessiontoken") {
                    Ok(mut file) => { let _ = file.write_all(token.as_bytes()); },
                    Err(e) => { error!("{}", t!("log.error.write_sess_token", error = e.to_string().as_str())); }
                }
                self.offline_mode = false;
                self.view_state.login_modal_state.inner_mut().login_failed = false;
                self.view_state.login_modal_state.show(false);
            }
            Message::LoginFailed(e) => {
                self.view_state.login_modal_state.inner_mut().login_err_msg = format!("{}", e);
                self.view_state.login_modal_state.inner_mut().login_failed = true;
                if e == ApiError::ReqwestError {
                    self.offline_mode = true;
                    // fail open (for now)
                    self.view_state.login_modal_state.show(false);
                }
            }
            Message::SessionValid(user) => {
                self.offline_mode = false;
                self.userdata = Some(user);
                self.view_state.login_modal_state.show(false);
            }
            Message::SessionFailed(e) => {
                error!("{}", t!("log.error.invalid_session", error = e.to_string().as_str()));
                if e == ApiError::ReqwestError {
                    self.offline_mode = true;
                    // fail open (for now)
                    self.view_state.login_modal_state.show(false);
                }
            }
            Message::SpinnerNextFrame(_) => {
               self.view_state.spinner_frame_index = (self.view_state.spinner_frame_index + 1) % 30;
               self.view_state.spinner_frame = self.spinner_frames[self.view_state.spinner_frame_index as usize].clone();
            }
            Message::TabChanged(index) => {
                self.view_state.active_tab = index;
            }
            Message::Done => {
                // set the button text back to normal state
                self.state = AppState::Idle;

                // stop the spinner
                self.view_state.show_spinner = false;
            }
        }
        self.view_state.status_message = self.state.to_string();
        command
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.view_state.show_spinner {
            time::every(core::time::Duration::from_millis(33)).map(Message::SpinnerNextFrame)
        }
        else {
            Subscription::none()
        }
    }
}

impl App {
    fn handle_fakeyou_model_browse_pressed(&mut self) -> Result<(), AppError> {
        let mut model_directory = self.current_directory.clone();
        model_directory.push("models");

        let path = FileDialog::new()
            .add_filter("FakeYou Model", &["fakeyou"])
            .set_location(&model_directory)
            .show_open_single_file();

        match path {
            Ok(Some(p)) => {
                let model = self.model_deserializer.read_full_model_file(&p)
                    .map_err(|_e| AppError::FileDialogError(model_directory.clone()))?;

                let acoustic = self.acoustic.clone();
                let hifigan = self.hifigan.clone();

                // TODO use async instead of threads and handle errors
                std::thread::spawn(move ||{
                    acoustic.load_bytes(&model.acoustic_model_bytes)
                        .map_err(|e| {
                            e
                        })
                        .unwrap();
                    hifigan.load_bytes(&model.vocoder_model_bytes)
                        .map_err(|e| {
                            e
                        })
                        .unwrap();
                });

                self.view_state.fakeyou_model_browse_selected = Some(p.clone());

                let hubert = self.hubert.clone();
                std::thread::spawn(move ||{
                    hubert.load(PathBuf::from("./models/model2.recast_base"))}
                );

                self.view_state.hubert_browse_selected = Some(PathBuf::from("./models/model2.recast_base"));
            },
            Ok(None) => {},
            Err(_e) => { handle_errors(Err(AppError::FileDialogError(model_directory))); }
        }
        //FIXME some kind of progress bar while all this crap loads
        Ok(())
    }
}

