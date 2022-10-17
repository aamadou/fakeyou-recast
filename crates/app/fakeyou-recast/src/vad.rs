use crate::{AppError, softvc::{Model, ModelArc}};
use crate::MODEL_SAMPLE_RATE;
use tch::IValue;

#[derive(Clone)]
pub struct Vad(ModelArc);

impl Model for Vad {
    fn get_inner(&self) -> &ModelArc { &self.0 }
}

impl Vad {
    pub fn forward(&self, samples: &[f32]) -> Result<tch::Tensor, AppError> {
        let tensor = tch::Tensor::of_slice(&samples).unsqueeze(0).to(tch::Device::Cuda(0));
        let model = self.0.try_lock().map_err(|_e| AppError::LockError)?;
        let output = tch::no_grad(|| {
            model.as_ref().ok_or(AppError::NoModel)?.forward_is(&[IValue::Tensor(tensor), IValue::Int(MODEL_SAMPLE_RATE as i64)]).map_err(|_e| AppError::ModelError(_e))
        })?;

        match output {
            IValue::Tensor(t) => {
                Ok(t)
            },
            _ => Err(AppError::ModelError(tch::TchError::UnknownKind(0)))
        }
    }
    pub fn new() -> Self {
        Self(ModelArc::default())
    }
}
