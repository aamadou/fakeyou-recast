use std::path::Path;
use core::fmt::Debug;

use onnxruntime::{session::Session, error::OrtError, GraphOptimizationLevel, environment::Environment, tensor::OrtOwnedTensor, TypeToTensorElementDataType};
use ndarray::Array;

//use wonnx::{Session, SessionError};

//use tract_onnx::{onnx, prelude::{Framework, TractError, SimplePlan, InferenceFact, TValue, InferenceModelExt, DatumExt, TypedFact, TypedOp}, tract_hir::internal::{InferenceOp, tract_smallvec::SmallVec}};

//pub type OnnxOptimizedRunnable = SimplePlan<InferenceFact, Box<dyn InferenceOp>, tract_onnx::prelude::Graph<InferenceFact, Box<dyn InferenceOp>>>;

//pub type OnnxOptimizedRunnable = Session;

//pub fn load_model<'a, P>(path: P, environment: Environment) -> Result<Session<'a>, OrtError>
//where 
    //P: AsRef<Path> + 'a,
//{
    //let session = environment
    //.new_session_builder()?
    //.with_optimization_level(GraphOptimizationLevel::Basic)?
    //.with_number_threads(1)?
    //.with_model_from_file(path);
    //session
    ////onnx().model_for_path(path)?[>.into_optimized()?<].into_runnable()
//}


pub fn inference<'s, 't, 'm, TIn, TOut, D>(
        model: &'s mut Session,
        input_arrays: Vec<Array<TIn, D>>,
) -> Result<Vec<OrtOwnedTensor<'t, 'm, TOut, ndarray::IxDyn>>, OrtError>
where
        TIn: TypeToTensorElementDataType + Debug + Clone,
        TOut: TypeToTensorElementDataType + Debug + Clone,
        D: ndarray::Dimension,
        'm: 't, // 'm outlives 't (memory info outlives tensor)
        's: 'm, // 's outlives 'm (session outlives memory info)
{
    model.run(input_arrays)
}
// FIXME this could probably be a bit cleaner
//pub fn describe_inputs(model: &OnnxOptimizedRunnable) -> String {
    //let mut description = String::new();
    //let mut node_names = model.model.node_names();
    //for i in 0..model.model.inputs.len() {
        //let name = node_names.next().unwrap_or("");
        //let node_fact = model.model.node_facts(i).unwrap();
        //description += &format!("{}: {:?}\n", name, node_fact); 
    //}
    //description
//}
