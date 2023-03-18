use super::onnx::{load_model, inference};
use std::path::Path;
use tract_onnx::{prelude::{TractError, SimplePlan, InferenceFact, TValue, tvec}, tract_hir::internal::{InferenceOp, tract_smallvec::SmallVec}};


pub struct SoVitsSvcModel(SimplePlan<InferenceFact, Box<dyn InferenceOp>, tract_onnx::prelude::Graph<InferenceFact, Box<dyn InferenceOp>>>);

impl SoVitsSvcModel
{
    pub fn try_new<P>(path: P) -> Result<Self, TractError>
    where P: AsRef<Path> {
        Ok(SoVitsSvcModel(load_model(path)?))
    }
    pub fn inference(&self, c: TValue, f0: TValue, mel2ph: TValue, uv: TValue, noise: TValue, g: TValue) -> Result<SmallVec<[TValue;4]>, TractError> {
        inference(&self.0, tvec![c, f0, mel2ph, uv, noise, g])
    }
}
