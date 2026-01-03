use std::path::Path;
use std::fs::File;
use std::io::Write;
use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use tar::Archive;
use bzip2::read::BzDecoder;
use crate::storage::ModelVersion;

/// Get the download URL for a specific model version
pub fn get_model_url(version: &ModelVersion) -> &'static str {
    match version {
        ModelVersion::Quantized => "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2",
        ModelVersion::Unquantized => "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2",
    }
}

/// Get the extracted folder name for a specific model version
fn get_extracted_folder_name(version: &ModelVersion) -> &'static str {
    match version {
        ModelVersion::Quantized => "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
        ModelVersion::Unquantized => "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
    }
}

/// Get the subdirectory name for a specific model version
pub fn get_version_subdir(version: &ModelVersion) -> &'static str {
    match version {
        ModelVersion::Quantized => "quantized",
        ModelVersion::Unquantized => "unquantized",
    }
}

/// Get the full model directory path for a specific version
pub fn get_model_dir_for_version(base_dir: &str, version: &ModelVersion) -> String {
    let base = Path::new(base_dir);
    base.join(get_version_subdir(version)).to_string_lossy().to_string()
}

/// Check if model files exist for a specific version
pub fn check_model_exists_for_version(base_dir: &str, version: &ModelVersion) -> bool {
    let version_dir = get_model_dir_for_version(base_dir, version);
    let path = Path::new(&version_dir);
    path.join("model.onnx").exists() && path.join("tokens.txt").exists()
}

/// Check if model exists (legacy, uses default quantized)
pub fn check_model_exists(model_dir: &str) -> bool {
    let path = Path::new(model_dir);
    path.join("model.onnx").exists() && path.join("tokens.txt").exists()
}

/// Download model for a specific version
pub async fn download_model_version<F>(base_dir: &str, version: &ModelVersion, on_progress: F) -> Result<()> 
where F: Fn(u64, u64) + Send + 'static {
    let url = get_model_url(version);
    let version_dir = get_model_dir_for_version(base_dir, version);
    let target_path = Path::new(&version_dir);
    
    if !target_path.exists() {
        std::fs::create_dir_all(target_path)?;
    }

    let client = Client::new();
    let res = client.get(url).send().await?;
    let total_size = res.content_length().unwrap_or(0);
    
    let mut stream = res.bytes_stream();
    let temp_tar_path = target_path.join("model.tar.bz2");
    let mut file = File::create(&temp_tar_path)?;
    let mut downloaded: u64 = 0;

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    // Extract
    println!("Extracting model...");
    let tar_bz2 = File::open(&temp_tar_path)?;
    let tar = BzDecoder::new(tar_bz2);
    let mut archive = Archive::new(tar);
    archive.unpack(target_path)?;

    // Cleanup temp file
    std::fs::remove_file(temp_tar_path)?;

    // Handle nested folder structure
    let extracted_folder_name = get_extracted_folder_name(version);
    let nested_dir = target_path.join(extracted_folder_name);
    if nested_dir.exists() {
        for entry in std::fs::read_dir(&nested_dir)? {
            let entry = entry?;
            let path = entry.path();
            let mut file_name = entry.file_name();
            
            // Rename model.int8.onnx or similar to model.onnx
            let name_str = file_name.to_string_lossy();
            if name_str.ends_with(".onnx") && !name_str.eq("model.onnx") {
                 file_name = std::ffi::OsString::from("model.onnx");
            }

            // Move file to target_path
            std::fs::rename(&path, target_path.join(file_name))?;
        }
        std::fs::remove_dir(nested_dir)?;
    }

    Ok(())
}

/// Legacy download function (downloads quantized by default)
pub async fn download_model<F>(model_dir: &str, on_progress: F) -> Result<()> 
where F: Fn(u64, u64) + Send + 'static {
    // For backwards compatibility, download to model_dir directly
    let target_path = Path::new(model_dir);
    if !target_path.exists() {
        std::fs::create_dir_all(target_path)?;
    }

    let url = get_model_url(&ModelVersion::Quantized);
    let client = Client::new();
    let res = client.get(url).send().await?;
    let total_size = res.content_length().unwrap_or(0);
    
    let mut stream = res.bytes_stream();
    let temp_tar_path = target_path.join("model.tar.bz2");
    let mut file = File::create(&temp_tar_path)?;
    let mut downloaded: u64 = 0;

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    println!("Extracting model...");
    let tar_bz2 = File::open(&temp_tar_path)?;
    let tar = BzDecoder::new(tar_bz2);
    let mut archive = Archive::new(tar);
    archive.unpack(target_path)?;

    std::fs::remove_file(temp_tar_path)?;

    let extracted_folder_name = get_extracted_folder_name(&ModelVersion::Quantized);
    let nested_dir = target_path.join(extracted_folder_name);
    if nested_dir.exists() {
        for entry in std::fs::read_dir(&nested_dir)? {
            let entry = entry?;
            let path = entry.path();
            let mut file_name = entry.file_name();
            
            let name_str = file_name.to_string_lossy();
            if name_str.ends_with(".onnx") && !name_str.eq("model.onnx") {
                 file_name = std::ffi::OsString::from("model.onnx");
            }

            std::fs::rename(&path, target_path.join(file_name))?;
        }
        std::fs::remove_dir(nested_dir)?;
    }

    Ok(())
}

