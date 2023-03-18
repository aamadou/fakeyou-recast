use std::path::Path;
use tract_onnx::{onnx, prelude::{Framework, TractError, SimplePlan, InferenceFact, TValue}, tract_hir::internal::{InferenceOp, tract_smallvec::SmallVec}};

pub fn load_model<P>(path: P) -> Result<SimplePlan<InferenceFact, Box<dyn InferenceOp>, tract_onnx::prelude::Graph<InferenceFact, Box<dyn InferenceOp>>>, TractError>
where 
    P: AsRef<Path>,
{
    onnx().model_for_path(path)?.into_runnable()
}

pub fn inference(model: &SimplePlan<InferenceFact, Box<dyn InferenceOp>, tract_onnx::prelude::Graph<InferenceFact, Box<dyn InferenceOp>>>, inputs: SmallVec<[TValue;4]>) -> Result<SmallVec<[TValue;4]>, TractError> {
    model.run(inputs)
}
