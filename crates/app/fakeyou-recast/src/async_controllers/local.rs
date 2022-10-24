use model_serialization::hydrated_model::HydratedModelLightMetadata;
use crate::{AppError, Message};
use futures::stream::{self, StreamExt};

pub async fn list_models_local(model_dir: std::path::PathBuf) -> Result<Vec<HydratedModelLightMetadata>, AppError> {
    let model_serdes = model_serialization::model_serializer::ModelSerializer::new(crate::consts::CRYPTO_SECRET);
    let dir_list = async_fs::read_dir(model_dir.clone()).await.map_err(|_e| AppError::ModelListError(model_dir.clone()))?;
    let vec: Vec<HydratedModelLightMetadata> = dir_list
        .filter_map(|res| async move { 
            match res {
                Ok(entry) => {
                    let file_type = entry.file_type().await.ok();
                    let path = entry.path();
                    let extension = path.extension();
                    let opt_tuple = match (file_type, extension) {
                        (Some(file_type), Some(ext)) => {
                            Some((file_type, ext))
                        }
                        _ => None
                    };
                    match opt_tuple {
                        Some((ftype, ext)) => {
                            if ftype.is_file() && ext == "fakeyou" {
                                Some(entry)
                            } else {
                                None
                            }
                        }
                        None => None
                    }
                },
                Err(_) => None
            }
    }).map(|dirent| model_serdes.read_fast_model_file_metadata(dirent.path()))
    .filter_map(|res| async move {
       res.ok() 
    })
    .collect().await;
    Ok(vec)
}

pub fn list_models_local_callback(res: Result<Vec<HydratedModelLightMetadata>, AppError>) -> Message {
    match res {
        Ok(vec) => {
            for model in vec.iter() {
               println!("{}", model.model_name);
            }
        }
        Err(_) => {}
    }
    Message::None
}
