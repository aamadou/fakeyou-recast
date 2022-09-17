use crate::Message;
use std::path::PathBuf;
use iced::{Text, Row, Column, Element, slider::{self, Slider}, pick_list::{self, PickList}, button::{self, Button}};

pub struct ViewState {
    pub model_browse_state: button::State,
    pub record_state: button::State,
    pub play_target_state: button::State,
    pub play_source_state: button::State,
    pub realtime_record_state: button::State,
    pub hifigan_browse_state: button::State,
    pub hubert_browse_state: button::State,
    pub input_browse_state: button::State,
    pub output_browse_state: button::State,
    pub slider_1_state: slider::State,
    pub slider_2_state: slider::State,
    pub slider_3_state: slider::State,
    pub slider_4_state: slider::State,
    pub input_device_list_state: pick_list::State<String>,
    pub output_device_list_state: pick_list::State<String>,
    pub model_browse_selected: Option<PathBuf>,
    pub hifigan_browse_selected: Option<PathBuf>,
    pub hubert_browse_selected: Option<PathBuf>,
    pub input_browse_selected: Option<PathBuf>,
    pub input_device_list_selected: Option<String>,
    pub output_device_list_selected: Option<String>,
    pub output_browse_selected: Option<PathBuf>,
    pub slider_1_value: f32,
    pub slider_2_value: f32,
    pub slider_3_value: f32,
    pub slider_4_value: f32,
    pub show_debug: bool,
}

impl ViewState{
    pub fn new() -> Self {
        let model_browse_state = button::State::new();
        let record_state = button::State::new();
        let play_target_state = button::State::new();
        let play_source_state = button::State::new();
        let realtime_record_state = button::State::new();
        let hifigan_browse_state = button::State::new();
        let hubert_browse_state = button::State::new();
        let input_browse_state = button::State::new();
        let output_browse_state = button::State::new();
        let slider_1_state = slider::State::new();
        let slider_2_state = slider::State::new();
        let slider_3_state = slider::State::new();
        let slider_4_state = slider::State::new();
        let input_device_list_state = pick_list::State::new();
        let output_device_list_state = pick_list::State::new();
        let model_browse_selected =  None;
        let hifigan_browse_selected = None;
        let hubert_browse_selected = None;
        let input_browse_selected = None;
        let input_device_list_selected = None;
        let output_device_list_selected = None;
        let output_browse_selected = None;
        let slider_1_value = 80.0;
        let slider_2_value = 80.0;
        let slider_3_value = 0.00003;
        let slider_4_value = 19200.0;
        let show_debug = false;

        Self {
            model_browse_state,
            record_state,
            play_target_state,
            play_source_state,
            realtime_record_state,
            hifigan_browse_state,
            hubert_browse_state,
            input_browse_state,
            output_browse_state,
            slider_1_state,
            slider_2_state,
            slider_3_state,
            slider_4_state,
            input_device_list_state,
            output_device_list_state,
            model_browse_selected,
            hifigan_browse_selected,
            hubert_browse_selected,
            input_browse_selected,
            input_device_list_selected,
            output_device_list_selected,
            output_browse_selected,
            slider_1_value,
            slider_2_value,
            slider_3_value,
            slider_4_value,
            show_debug,
        }
    }
}
pub fn view<'a> (view_state: &'a mut ViewState, input_device_list_options: &'a Vec<String>, output_device_list_options: &'a Vec<String>, recording: bool, realtime_recording: bool) -> Element<'a, Message> {
    let mut model_selection_row_elems: Vec<Element<Message>> = vec![];
    let empty_pathbuf = PathBuf::from("");
    let selected_model_str = if view_state.model_browse_selected.is_none() { "" } else {view_state.model_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    let selected_hifigan_str = if view_state.hifigan_browse_selected.is_none() { "" } else {view_state.hifigan_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    let selected_hubert_str = if view_state.hubert_browse_selected.is_none() { "" } else {view_state.hubert_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_stem().unwrap_or_default().to_str().unwrap_or_default() };
    model_selection_row_elems
        .push(Text::new("Select Recast Model File: ").into());
    model_selection_row_elems
        .push(Button::new(&mut view_state.model_browse_state, Text::new("Browse")).on_press(Message::ModelBrowsePressed).into());
    model_selection_row_elems
        .push(Text::new(selected_model_str).into());
        //model_selection_row_elems
            //.push(Text::new("Select Vocoder: ").into());
        //model_selection_row_elems
            //.push(Button::new(&mut view_state.hifigan_browse_state, Text::new("Browse")).on_press(Message::HifiganBrowsePressed).into());
        //model_selection_row_elems
        //.push(Text::new(selected_hifigan_str).into());
    if view_state.show_debug {
        model_selection_row_elems
            .push(Text::new("Select Debug Model: ").into());
        model_selection_row_elems
            .push(Button::new(&mut view_state.hubert_browse_state, Text::new("Browse")).on_press(Message::HubertBrowsePressed).into());
        model_selection_row_elems
        .push(Text::new(selected_hubert_str).into());
    }
    let model_selection_row = Row::with_children(model_selection_row_elems).spacing(10);

    let mut file_selection_row_elems: Vec<Element<Message>> = vec![];
    if view_state.input_device_list_selected == Some(String::from("From file"))
    {
        let selected_input_file_str = if view_state.input_browse_selected.is_none() { "" } else {view_state.input_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };
        file_selection_row_elems.push(
            Text::new("Input File: ").into()
        );
        file_selection_row_elems.push(
            Button::new(&mut view_state.input_browse_state, Text::new("Browse")).on_press(Message::InputBrowsePressed).into()
        );
        file_selection_row_elems.push(Text::new(selected_input_file_str).into());
    }
    if view_state.output_device_list_selected == Some(String::from("To file"))
    {
        let selected_output_file_str = if view_state.output_browse_selected.is_none() { "" } else {view_state.output_browse_selected.as_ref().unwrap_or(&empty_pathbuf).file_name().unwrap_or_default().to_str().unwrap_or_default() };
        file_selection_row_elems.push(
            Text::new("Output File: ").into()
        );
        file_selection_row_elems.push(
            Button::new(&mut view_state.output_browse_state, Text::new("Browse")).on_press(Message::OutputBrowsePressed).into()
        );
        file_selection_row_elems.push(Text::new(selected_output_file_str).into());
    }
    let file_selection_row = Row::with_children(file_selection_row_elems).spacing(10);

    let elements = Column::new().spacing(5)
        .push(model_selection_row)
        .push(
            Row::new().spacing(10)
            .push(Text::new("Input Device: "))
            .push(
                PickList::new(
                    &mut view_state.input_device_list_state,
                    input_device_list_options,
                    view_state.input_device_list_selected.clone(),
                    Message::InputDeviceChanged,
                )
            )
            .push(Text::new("Output Device: "))
            .push(
                PickList::new(
                    &mut view_state.output_device_list_state,
                    output_device_list_options,
                    view_state.output_device_list_selected.clone(),
                    Message::OutputDeviceChanged,
                )
            )
        )
        .push(
            file_selection_row
        )
        .push(
            Row::new().spacing(10)
            .push(Text::new("Input Volume: "))
            .push(
                Slider::new(&mut view_state.slider_1_state, 0.0..=100.0, view_state.slider_1_value, Message::Slider1Changed)
            )
            .push(Text::new("Output Volume: "))
            .push(
                Slider::new(&mut view_state.slider_2_state, 0.0..=100.0, view_state.slider_2_value, Message::Slider2Changed)
            )
        )
        //.push(
            //Row::new().spacing(10)
            //.push(Text::new("Speaker Threshold: "))
            //.push(
                //Slider::new(&mut view_state.slider_3_state, 0.000005..=0.000100, view_state.slider_3_value, Message::Slider3Changed).step(0.000001)
            //)
            ////FIXME
            //.push(Text::new("Buffer Size: "))
            //.push(
                //Slider::new(&mut view_state.slider_4_state, 4800.0..=67200.0, view_state.slider_4_value, Message::Slider4Changed).step(4800.0)
            //)
        //)
        .push(
            Row::new().spacing(10)
            .push(
                Button::new(&mut view_state.record_state, 
                    if view_state.input_device_list_selected == Some(String::from("From file")) { Text::new("Read File")  }
                    else if recording { Text::new("Stop")}
                    else {Text::new("Record")}).on_press(Message::RecordPressed)
            )
            .push(
                Button::new(&mut view_state.play_source_state, Text::new("Play Source")).on_press(Message::PlaySourcePressed)
            )
            .push(
                Button::new(&mut view_state.play_target_state, 
                    if view_state.output_device_list_selected == Some(String::from("To file")) { Text::new("Write File") }
                    else {
                        Text::new("Play Target")
                    })
                    .on_press(Message::PlayTargetPressed)
            )
            .push(
                Button::new(&mut view_state.realtime_record_state, if realtime_recording { Text::new("Stop") } else {Text::new("Realtime\n(experimental)")}).on_press(Message::RealtimeRecordPressed)
            )

        );

    elements.into()
}
