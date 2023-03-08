use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tch::{CModule, Tensor};
use crate::error::AppError;

pub type ModelArc = Arc<Mutex<Option<CModule>>>;

pub trait Model {
    fn load(&self, path: PathBuf) -> Result <(), AppError> {
        let mut lock = self.get_inner().try_lock().map_err(|_e| AppError::LockError)?;
        let mut inner_model = CModule::load_on_device(path, tch::Device::cuda_if_available()).map_err(|_e| AppError::ModelLoadError)?;
        inner_model.set_eval();
        *lock = Some(inner_model);
        Ok(())
    }

    fn load_bytes(&self, model_bytes: &[u8]) -> Result<(), AppError> {
        let mut readable = BufReader::new(model_bytes);
        self.load_readable(&mut readable)
    }

    fn load_readable<T: std::io::Read>(&self, model_bytes: &mut T) -> Result<(), AppError> {
        let mut lock = self.get_inner().try_lock().map_err(|_e| AppError::LockError)?;
        let mut inner_model = CModule::load_data_on_device(model_bytes, tch::Device::cuda_if_available()).map_err(|_e| AppError::ModelLoadError)?;
        inner_model.set_eval();
        *lock = Some(inner_model);
        Ok(())
    }
    
    fn get_inner(&self) -> &ModelArc;
}

#[derive(Clone)]
pub struct Hubert(ModelArc);

impl Model for Hubert {
    fn get_inner(&self) -> &ModelArc { &self.0 }
}

impl Hubert {
    pub fn forward(&self, samples: &[f32]) -> Result<tch::Tensor, AppError> {
        let tensor = Tensor::of_slice(&samples).unsqueeze(0).unsqueeze(0).to(tch::Device::cuda_if_available());
        let model = self.0.try_lock().map_err(|_e| AppError::LockError)?;
        model.as_ref().ok_or(AppError::NoModel)?.method_ts("units", &[tensor]).map_err(|_e| AppError::ModelError(_e))
    }
    pub fn new() -> Self {
        Self(ModelArc::default())
    }
}

#[derive(Clone)]
pub struct Acoustic(ModelArc);

impl Model for Acoustic {
    fn get_inner(&self) -> &ModelArc { &self.0 }
}

impl Acoustic {
    pub fn forward(&self, input: tch::Tensor) -> Result<tch::Tensor, AppError> {
        let model = self.0.try_lock().map_err(|_e| AppError::LockError)?;
        model.as_ref().ok_or(AppError::NoModel)?.method_ts("generate", &[input]).map_err(|_e| AppError::ModelError(_e))
    }
    pub fn new() -> Self {
        Self(ModelArc::default())
    }
}



#[derive(Clone)]
pub struct Hifigan(ModelArc);

impl Model for Hifigan {
    fn get_inner(&self) -> &ModelArc { &self.0 }
}

impl Hifigan {
    pub fn forward(&self, input: tch::Tensor) -> Result<Vec<f32>, AppError> {
        let model = self.0.try_lock().map_err(|_e| AppError::LockError)?;
        let hifigan_output = tch::no_grad(|| {
             model.as_ref().ok_or(AppError::NoModel)?.forward_ts(&[input]).map_err(|_e| AppError::ModelError(_e))
        })?;
        let sample_count = hifigan_output.size()[2];
        let samples = hifigan_output.reshape(&[sample_count]).to(tch::Device::Cpu);
        let mut sample_buf: Vec<f32> = Vec::new();
        sample_buf.resize(sample_count as usize, 0.0);
        samples.copy_data(&mut sample_buf, sample_count as usize);
        Ok(sample_buf)
    }
    pub fn new() -> Self {
        Self(ModelArc::default())
    }
}

pub fn inference(in_samples: &[f32], hubert: Hubert, acoustic: Acoustic, hifigan: Hifigan) -> Result<Vec<f32>, AppError> {
    let hubert_start_time = std::time::Instant::now();
    let hubert_out = hubert.forward(in_samples)?;
    let hubert_end_time = std::time::Instant::now();
    let acoustic_start_time = std::time::Instant::now();
    let acoustic_out = acoustic.forward(hubert_out)?;
    let acoustic_end_time = std::time::Instant::now();
    let transpose = acoustic_out.transpose(2, 1);
    let hifigan_start_time = std::time::Instant::now();
    let out_samples = hifigan.forward(transpose)?; 
    let hifigan_end_time = std::time::Instant::now();

    // FIXME: don't forget to uncomment this if statement to hide our secrets before release
    if cfg!(debug_assertions) {
        println!("hubert: {:?}", hubert_end_time - hubert_start_time);
        println!("acoustic: {:?}", acoustic_end_time - acoustic_start_time);
        println!("hifigan: {:?}", hifigan_end_time - hifigan_start_time);
    }

    Ok(out_samples)
}
