# Dependencies
- The Rust programming language https://rustup.rs/
- libtorch 1.13.0 cxx-abi https://pytorch.org/get-started/locally/
- cuda-toolkit https://developer.nvidia.com/cuda-downloads
- maybe cudnn too idk?
- vulkan/directx/metal (should be provided by your graphics driver)

# Setup
- Install the dependencies listed above
- Extract the [base models](https://storyteller-nextcloud.paulsajna.com/s/K7z8GHbKYpB3amH) to the models directory of the repo
- Download JIT acoustic(.recast) and hifigan(hifigan-jit.pt) models and copy to the models directory https://drive.google.com/drive/folders/1GY-K9cb-naBJdANGQ9MGc8Vzkzyh9Il3?usp=sharing
- run the app with `cargo run --release --bin fakeyou-recast --features=devmode`

#  Usage
- Load the acoustic model using the "Recast Model File" browse button
- (optionally) load the finetuned-hifigan model using the "Vocoder" browse button
- Select audio devices from the dropdown menus
- Use the record and play target buttons to perform non-realtime inference
- Use the realtime button to perform realtime inference
- Use the play source button to playback your recorded audio

# Training
https://drive.google.com/drive/folders/1KhVZHlUbVVfa8p-2GmvGt0Jc6P5TXAj5?usp=sharing
- use softvc_pynb for training acoustic
- use softvc-hifigan-fine-tuning for hifigan fine tuning
- use softvc-TraceAndOnnx for JIT'ing the models to the form the app needs (torchscript)
