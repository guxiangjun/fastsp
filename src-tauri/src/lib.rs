mod asr;
mod audio;
mod input_listener;
mod model_manager;
mod storage;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use storage::{AppConfig, HistoryItem, ModelVersion};
use serde::Serialize;

// Define State Types
type AudioState = Mutex<audio::AudioService>;
type AsrState = asr::AsrService;
type StorageState = storage::StorageService;
type InputListenerState = input_listener::InputListener;

fn save_debug_wav(_samples: &[f32], _sample_rate: u32, _filename: &str) {
    /*
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    // Config path or just a known temp location. 
    // Using current dir for dev or AppData would be better but let's stick to simple relative for debug
    // or better, print the path.
    let path = std::env::temp_dir().join(filename);
    println!("Saving debug audio to: {:?}", path);

    if let Ok(mut writer) = hound::WavWriter::create(&path, spec) {
        for &sample in samples {
            let amplitude = i16::MAX as f32;
            let val = (sample * amplitude) as i16;
            writer.write_sample(val).ok();
        }
        writer.finalize().ok();
    }
    */
}

use enigo::{Enigo, Keyboard, Settings, Direction, Key}; // Update Enigo imports



fn paste_text(text: &str) {
    // println!("Pasting text: {}", text);
    
    // 1. Set Clipboard
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.set_text(text) {
                eprintln!("Failed to set clipboard: {}", e);
                return;
            }
        },
        Err(e) => {
            eprintln!("Failed to init clipboard: {}", e);
            return;
        }
    }

    // 2. Simulate Ctrl+V
    // Wait a bit for clipboard update propagation
    std::thread::sleep(std::time::Duration::from_millis(100));

    match Enigo::new(&Settings::default()) {
        Ok(mut enigo) => {
            // Ctrl + V
            let _ = enigo.key(Key::Control, Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), Direction::Click); 
            let _ = enigo.key(Key::Control, Direction::Release);
        },
        Err(e) => eprintln!("Failed to init Enigo: {:?}", e),
    }
}

#[derive(Serialize)]
pub struct ModelVersionsStatus {
    quantized: bool,
    unquantized: bool,
    current: String,
}

#[derive(Serialize)]
pub struct ModelDetailedStatus {
    downloaded: bool,
    loaded: bool,
}

#[tauri::command]
fn get_config(state: tauri::State<StorageState>) -> AppConfig {
    state.load_config()
}

#[tauri::command]
fn save_config(
    state: tauri::State<StorageState>, 
    listener: tauri::State<InputListenerState>,
    config: AppConfig
) -> Result<(), String> {
    // Update listener flags immediately (hot-reload)
    listener.enable_mouse.store(config.trigger_mouse, std::sync::atomic::Ordering::Relaxed);
    listener.enable_hold.store(config.trigger_hold, std::sync::atomic::Ordering::Relaxed);
    listener.enable_toggle.store(config.trigger_toggle, std::sync::atomic::Ordering::Relaxed);
    
    state.save_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_history(state: tauri::State<StorageState>) -> Vec<HistoryItem> {
    state.load_history()
}

#[tauri::command]
fn clear_history(state: tauri::State<StorageState>) -> Result<(), String> {
    state.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_model_status(state: tauri::State<'_, StorageState>) -> Result<bool, String> {
    let config = state.load_config();
    // Check if the currently selected version exists
    Ok(model_manager::check_model_exists_for_version(&config.model_dir, &config.model_version))
}

#[tauri::command]
async fn get_model_versions_status(state: tauri::State<'_, StorageState>) -> Result<ModelVersionsStatus, String> {
    let config = state.load_config();
    let quantized = model_manager::check_model_exists_for_version(&config.model_dir, &ModelVersion::Quantized);
    let unquantized = model_manager::check_model_exists_for_version(&config.model_dir, &ModelVersion::Unquantized);
    let current = match config.model_version {
        ModelVersion::Quantized => "quantized".to_string(),
        ModelVersion::Unquantized => "unquantized".to_string(),
    };
    Ok(ModelVersionsStatus { quantized, unquantized, current })
}

#[tauri::command]
async fn get_model_detailed_status(
    state: tauri::State<'_, StorageState>,
    asr: tauri::State<'_, AsrState>
) -> Result<ModelDetailedStatus, String> {
    let config = state.load_config();
    let downloaded = model_manager::check_model_exists_for_version(&config.model_dir, &config.model_version);
    let loaded = asr.is_loaded();
    Ok(ModelDetailedStatus { downloaded, loaded })
}

#[tauri::command]
async fn download_model<R: Runtime>(app: AppHandle<R>, state: tauri::State<'_, StorageState>) -> Result<(), String> {
    let config = state.load_config();
    let model_dir = config.model_dir.clone();
    
    // Run download in background
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let res = model_manager::download_model(&model_dir, move |current, total| {
            progress_handle.emit("download_progress", serde_json::json!({ "current": current, "total": total })).ok();
        }).await;
        
        if let Err(e) = res {
             handle.emit("download_error", e.to_string()).ok();
        } else {
             handle.emit("download_complete", ()).ok();
        }
    });
    
    Ok(())
}

#[tauri::command]
async fn download_model_for_version<R: Runtime>(
    app: AppHandle<R>, 
    state: tauri::State<'_, StorageState>,
    asr: tauri::State<'_, AsrState>,
    version: String
) -> Result<(), String> {
    let config = state.load_config();
    let model_dir = config.model_dir.clone();
    let language = config.language.clone();
    let model_version = match version.as_str() {
        "quantized" => ModelVersion::Quantized,
        "unquantized" => ModelVersion::Unquantized,
        _ => return Err("Invalid version".to_string()),
    };
    
    let handle = app.clone();
    let version_for_download = model_version.clone();
    let asr_clone = asr.inner().clone();
    
    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let res = model_manager::download_model_version(&model_dir, &version_for_download, move |current, total| {
            progress_handle.emit("download_progress", serde_json::json!({ "current": current, "total": total })).ok();
        }).await;
        
        if let Err(e) = res {
             handle.emit("download_error", e.to_string()).ok();
        } else {
             // Download complete - now auto-load the model
             handle.emit("download_complete", ()).ok();
             
             // Update config to use this version (get state from handle)
             let storage = handle.state::<StorageState>();
             let mut new_config = storage.load_config();
             new_config.model_version = version_for_download.clone();
             let _ = storage.save_config(&new_config);
             
             // Load the model
             let model_path = model_manager::get_model_dir_for_version(&model_dir, &version_for_download);
             match asr_clone.load_model(model_path, language) {
                 Ok(_) => {
                     handle.emit("model_loaded", ()).ok();
                 },
                 Err(e) => eprintln!("Failed to auto-load model after download: {}", e),
             }
        }
    });
    
    Ok(())
}

#[tauri::command]
async fn switch_model_version(
    state: tauri::State<'_, StorageState>,
    asr: tauri::State<'_, AsrState>,
    version: String
) -> Result<(), String> {
    let model_version = match version.as_str() {
        "quantized" => ModelVersion::Quantized,
        "unquantized" => ModelVersion::Unquantized,
        _ => return Err("Invalid version".to_string()),
    };
    
    let mut config = state.load_config();
    
    // Check if version is downloaded
    if !model_manager::check_model_exists_for_version(&config.model_dir, &model_version) {
        return Err("Model version not downloaded".to_string());
    }
    
    // Update config
    config.model_version = model_version.clone();
    state.save_config(&config).map_err(|e| e.to_string())?;
    
    // Reload ASR with new model
    let model_path = model_manager::get_model_dir_for_version(&config.model_dir, &model_version);
    asr.load_model(model_path, config.language.clone()).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn open_model_folder(state: tauri::State<'_, StorageState>) -> Result<(), String> {
    let config = state.load_config();
    std::process::Command::new("explorer")
        .arg(&config.model_dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_input_devices() -> Vec<audio::AudioDevice> {
    audio::AudioService::get_input_devices()
}

#[tauri::command]
fn get_current_input_device(audio: tauri::State<AudioState>) -> String {
    if let Ok(audio) = audio.lock() {
        audio.get_current_device_name()
    } else {
        String::new()
    }
}

#[tauri::command]
fn switch_input_device<R: Runtime>(
    app: AppHandle<R>,
    audio: tauri::State<AudioState>,
    storage: tauri::State<StorageState>,
    device_name: String
) -> Result<(), String> {
    // Update audio service
    if let Ok(mut audio) = audio.lock() {
        audio.init_with_device(&device_name, app.clone()).map_err(|e| e.to_string())?;
    } else {
        return Err("Failed to lock audio service".to_string());
    }
    
    // Save to config
    let mut config = storage.load_config();
    config.input_device = device_name;
    storage.save_config(&config).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
fn start_audio_test(audio: tauri::State<AudioState>) -> Result<(), String> {
    if let Ok(audio) = audio.lock() {
        audio.start_test().map_err(|e| e.to_string())
    } else {
        Err("Failed to lock audio service".to_string())
    }
}

#[tauri::command]
fn stop_audio_test(audio: tauri::State<AudioState>) -> Result<(), String> {
    if let Ok(audio) = audio.lock() {
        audio.stop_test().map_err(|e| e.to_string())
    } else {
        Err("Failed to lock audio service".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // Initialize Storage
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("data"));
            let storage_service = storage::StorageService::new(app_dir.clone());
            let mut config = storage_service.load_config();
            
            // Initialize Services
            let asr_service = asr::AsrService::new();
            
            // Fix model path to be in AppData if it's the default relative path
            // This prevents "Rebuilding application" loops during download when running in dev mode
            if config.model_dir == "./models/sense-voice" {
                let model_path = app_dir.join("models").join("sense-voice");
                config.model_dir = model_path.to_string_lossy().to_string();
                let _ = storage_service.save_config(&config);
                println!("Updated model directory to: {}", config.model_dir);
            }

            // Load model for the selected version if it exists (in background)
            let asr_for_loading = asr_service.clone();
            let app_handle_for_loading = app_handle.clone();
            let config_for_loading = config.clone();
            
            tauri::async_runtime::spawn(async move {
                if model_manager::check_model_exists_for_version(&config_for_loading.model_dir, &config_for_loading.model_version) {
                    let model_path = model_manager::get_model_dir_for_version(&config_for_loading.model_dir, &config_for_loading.model_version);
                    match asr_for_loading.load_model(model_path, config_for_loading.language.clone()) {
                        Ok(_) => {
                            // Emit event that model is loaded
                            app_handle_for_loading.emit("model_loaded", ()).ok();
                        },
                        Err(e) => eprintln!("Failed to load model in background: {}", e),
                    }
                }
            });

            let mut audio_service = audio::AudioService::new();
            if let Err(e) = audio_service.init_with_device(&config.input_device, app_handle.clone()) {
                eprintln!("Failed to init audio: {}", e);
            }
            let audio_state = Mutex::new(audio_service);

            let input_listener = input_listener::InputListener::new();
            // Update listener flags based on config
            input_listener.enable_mouse.store(config.trigger_mouse, std::sync::atomic::Ordering::Relaxed);
            input_listener.enable_hold.store(config.trigger_hold, std::sync::atomic::Ordering::Relaxed);
            input_listener.enable_toggle.store(config.trigger_toggle, std::sync::atomic::Ordering::Relaxed);

            // Channel for Input Events
            let (tx, rx) = std::sync::mpsc::channel();
            input_listener.start(tx);

            // Background Thread to handle events
            std::thread::spawn(move || {
                let mut is_recording = false;
                
                for event in rx {
                    match event {
                        input_listener::InputEvent::Start => {
                            if !is_recording {
                                // Start Recording
                                let audio = app_handle.state::<AudioState>();
                                let started = {
                                    if let Ok(audio) = audio.lock() {
                                        audio.start_recording().is_ok()
                                    } else {
                                        false
                                    }
                                };
                                if started {
                                    is_recording = true;
                                    app_handle.emit("recording_status", true).ok();
                                }
                            }
                        },
                        input_listener::InputEvent::Stop => {
                            if is_recording {
                                // Stop & Transcribe
                                is_recording = false;
                                app_handle.emit("recording_status", false).ok();
                                
                                let audio = app_handle.state::<AudioState>();
                                let mut buffer = Vec::new();
                                let mut sample_rate = 48000u32;
                                if let Ok(ref audio) = audio.lock() {
                                    sample_rate = audio.get_sample_rate();
                                    if let Ok(b) = audio.stop_recording() {
                                        buffer = b;
                                    }
                                }
                                    // Save debug wav
                                    // save_debug_wav(&buffer, sample_rate, "fastsp_debug.wav");

                                    let asr = app_handle.state::<AsrState>();
                                    // Transcribe with actual sample rate
                                    match asr.transcribe(buffer, sample_rate) {
                                        Ok(text) => {
                                             if !text.trim().is_empty() {
                                                 let id = uuid::Uuid::new_v4().to_string();
                                                 let item = HistoryItem {
                                                     id: id.clone(),
                                                     timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                                     text: text.clone(),
                                                     duration_ms: 0, // Calculate if needed
                                                 };
                                                 
                                                 let storage = app_handle.state::<StorageState>();
                                                 storage.add_history_item(item.clone()).ok();
                                                 app_handle.emit("transcription_update", item).ok();
                                                 
                                                 // Paste text
                                                 paste_text(&text);
                                             }
                                        },
                                        Err(e) => eprintln!("Transcription error: {}", e),
                                    }
                            }
                        },
                        input_listener::InputEvent::Toggle => {
                            if is_recording {
                                // Simulate Stop
                                // Same logic as Stop... can refactor to function?
                                // For now duplicate logic or use loop with internal event dispatch
                                // BUT: We need to trigger the Stop logic.
                                // We can just send a Stop event to ourselves if channel allows? No, sender is moved.
                                // Just copy paste logic for now or extract function.
                                // COPY-PASTE STOP LOGIC:
                                is_recording = false;
                                app_handle.emit("recording_status", false).ok();
                                let audio = app_handle.state::<AudioState>();
                                let mut buffer = Vec::new();
                                let mut sample_rate = 48000u32;
                                if let Ok(ref audio) = audio.lock() {
                                    sample_rate = audio.get_sample_rate();
                                    if let Ok(b) = audio.stop_recording() {
                                        buffer = b;
                                    }
                                }
                                    // Save debug wav
                                    // save_debug_wav(&buffer, sample_rate, "fastsp_debug_toggle.wav");

                                    let asr = app_handle.state::<AsrState>();
                                    match asr.transcribe(buffer, sample_rate) {
                                        Ok(text) => {
                                             if !text.trim().is_empty() {
                                                 let item = HistoryItem {
                                                     id: uuid::Uuid::new_v4().to_string(),
                                                     timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                                                     text: text.clone(),
                                                     duration_ms: 0,
                                                 };
                                                 let storage = app_handle.state::<StorageState>();
                                                 storage.add_history_item(item.clone()).ok();
                                                 app_handle.emit("transcription_update", item).ok();

                                                 // Paste text
                                                 paste_text(&text);
                                             }
                                        },
                                        Err(e) => eprintln!("Transcribe error: {}", e),
                                    }
                            } else {
                                // Simulate Start
                                let audio = app_handle.state::<AudioState>();
                                let started = {
                                    if let Ok(audio) = audio.lock() {
                                        audio.start_recording().is_ok()
                                    } else {
                                        false
                                    }
                                };
                                if started {
                                    is_recording = true;
                                    app_handle.emit("recording_status", true).ok();
                                }
                            }
                        }
                    }
                }
            });

            // manage states
            app.manage(audio_state);
            app.manage(asr_service);
            app.manage(storage_service);
            app.manage(input_listener); // expose to commands if needed (to update config)
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config, save_config, get_history, clear_history, 
            check_model_status, download_model, open_model_folder,
            get_model_versions_status, get_model_detailed_status,
            download_model_for_version, switch_model_version,
            get_input_devices, get_current_input_device, switch_input_device,
            start_audio_test, stop_audio_test
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
