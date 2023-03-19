use super::onnx;
use std::path::Path;


//use tract_onnx::{prelude::{TractError, SimplePlan, InferenceFact, TValue, tvec, IntoTensor}, tract_hir::{internal::{InferenceOp, tract_smallvec::SmallVec}, tract_ndarray}};
//use wonnx::SessionError;

//use onnx::OnnxOptimizedRunnable;

use onnxruntime::{OrtError, session::Session, tensor::OrtOwnedTensor, environment::Environment, GraphOptimizationLevel};

pub struct SoVitsSvcModel<'a> {
    hubert: Session<'a>,
    //acoustic: OnnxOptimizedRunnable
}

impl<'a> SoVitsSvcModel<'a>
{
    //pub fn try_new<P>(hubert_path: P, acoustic_path: P, environment: &Environment) -> Result<Self, OrtError>
    //where P: AsRef<Path> + 'a {
        //let hubert =  environment.clone()
    //.new_session_builder()?
    //.with_optimization_level(GraphOptimizationLevel::Basic)?
    //.with_number_threads(1)?
    //.with_model_from_file(hubert_path)?;

        ////println!("{}", onnx::describe_inputs(&hubert));
     ////   let acoustic = onnx::load_model(acoustic_path.as_ref())?;
        //Ok(SoVitsSvcModel{
            //hubert,
      ////      acoustic,
        //})
    //}

    //FIXME change this back to unpub when done testing
    pub fn hubert_inference<'s, 't, 'm>(&'a mut self, wav: &[f32]) -> Result<Vec<OrtOwnedTensor<'t, 'm, f32, ndarray::IxDyn>>, OrtError>
    where
        'm: 't, // 'm outlives 't (memory info outlives tensor)
        's: 'm, // 's outlives 'm (session outlives memory info)
        'a: 't,
        'a: 'm
    {
        //let tensor = tract_ndarray::arr1(wav).into_tensor().into_shape(&[1, 1, 142478])?;
        //onnx::inference(&self.hubert, tvec![tensor.into()])
        let tensor = vec![ndarray::arr1(wav)];
        onnx::inference(&mut self.hubert, tensor.into())
    }

    //fn acoustic_inference(&self, c: TValue, f0: TValue, mel2ph: TValue, uv: TValue, noise: TValue, g: TValue) -> Result<SmallVec<[TValue;4]>, TractError> {
        //onnx::inference(&self.acoustic, tvec![c, f0, mel2ph, uv, noise, g])
    //}

    pub fn change_acoustic<P>(&mut self, acoustic_path: P) -> Result<(), OrtError> 
    where P: AsRef<Path> {
        //self.acoustic = onnx::load_model(acoustic_path.as_ref())?;
        Ok(())
    }
}
