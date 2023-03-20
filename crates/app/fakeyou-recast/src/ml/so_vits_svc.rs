use super::onnx;
use std::path::Path;


//use tract_onnx::{prelude::{TractError, SimplePlan, InferenceFact, TValue, tvec, IntoTensor}, tract_hir::{internal::{InferenceOp, tract_smallvec::SmallVec}, tract_ndarray}};
//use wonnx::SessionError;

//use onnx::OnnxOptimizedRunnable;

use ndarray::{Dim, IxDynImpl};
use onnxruntime::{OrtError, session::Session, tensor::OrtOwnedTensor, environment::Environment, GraphOptimizationLevel, TypedArray};

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

pub fn inference<'a>(mut so_vits_svc: std::sync::MutexGuard<Session<'a>>, mut hubert: std::sync::MutexGuard<Session<'a>>, wav: &[f32]) -> Result<Vec<f32>, OrtError>{
    let wav_len = wav.len();
    let tensor = vec![ndarray::arr1(&wav).into_shape([1, 1, wav_len]).unwrap()];
    let hubert_run_start = std::time::Instant::now();
    let hubert_output: Vec<OrtOwnedTensor<f32, Dim<IxDynImpl>>> = hubert.run(tensor)?;
    let hubert_run_end = std::time::Instant::now();
    println!("Hubert time: {:?}", hubert_run_end-hubert_run_start);
    //println!("{:?}...", hubert_output.as_slice()[0].as_slice().unwrap()[0..20].to_vec());
    println!("{:?}", hubert_output[0].shape());
    
    let hubert_len = hubert_output[0].shape()[1];

    //println!("{:?}", so_vits_svc.inputs[1].dimensions);

    use rand::Fill;
    //let pitch = ndarray::Array::from_shape_vec(Dim([1,hubert_len]), vec![125.0f32; hubert_len]).unwrap().into_dyn();
    
    use rsworld::{dio, stonemask};
    use rsworld_sys::DioOption;

    let double: Vec<f64> = wav.iter().map(|float| (*float).into()).collect();
    let (t, f0) = dio(&double, 16000, &DioOption { f0_floor: 71.0, f0_ceil:800.0, channels_in_octave:2.0, frame_period: 1000.0*(wav_len as f64/hubert_len as f64)/16000.0, speed: 1, allowed_range: 0.1});
    let f0_vec = stonemask(&double, 16000, &t, &f0);
    let mut f0_vec: Vec<f32> = f0_vec.iter().map(|d| *d as f32 ).collect();
    f0_vec.resize(hubert_len, 0.0);

    let pitch = ndarray::Array::from_shape_vec(Dim([1, hubert_len]), f0_vec).unwrap().into_dyn();

    let mel2ph = (0..hubert_len as i64).into_iter().collect::<ndarray::Array1<i64>>().into_shape([1, hubert_len]).unwrap().into_dyn();
    let uv = ndarray::Array::ones([1, hubert_len]).into_dyn();
    let mut rng = rand::thread_rng();
    let mut random = vec![0.0; hubert_len * 192];
    random.try_fill(&mut rng).unwrap();
    let noise = ndarray::Array::from_shape_vec([1, 192, hubert_len], random.to_vec()).unwrap().into_dyn();
    //let noise = ndarray::Array::from_shape_vec([1, 192, hubert_len], vec![0.0; 85248]).unwrap().into_dyn();
    let speaker = ndarray::arr1(&[0]).into_dyn();


    let svc_inputs = vec![TypedArray::F32(hubert_output[0].to_owned()), TypedArray::F32(pitch), TypedArray::I64(mel2ph), TypedArray::F32(uv), TypedArray::F32(noise), TypedArray::I64(speaker)];

    let outputs = so_vits_svc.run_mixed::<f32, Dim<IxDynImpl>>(svc_inputs).unwrap();


    Ok(outputs[0].as_slice().unwrap().to_vec())

}
