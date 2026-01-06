mod asr;
mod audio;
mod http_client;
mod input_listener;
mod llm;
mod model_manager;
mod storage;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use storage::{AppConfig, HistoryItem, LlmConfig, ModelVersion, ProxyConfig};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

// Define State Types
type AudioState = Mutex<audio::AudioService>;
type AsrState = asr::AsrService;
type StorageState = storage::StorageService;
type InputListenerState = input_listener::InputListener;
type DownloadCancelState = Mutex<Option<CancellationToken>>;
type ProcessingState = Arc<std::sync::atomic::AtomicBool>; // 防止重复处理（跨线程/异步任务共享）

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use enigo::{Enigo, Keyboard, Settings};

// Monotonic id to correlate a single transcription pipeline across logs.
static TRANSCRIPTION_SEQ: AtomicU64 = AtomicU64::new(1);

fn preview_text(s: &str, max_chars: usize) -> String {
    // Keep logs readable: single-line preview with a hard cap.
    let mut out = String::with_capacity(max_chars.min(s.len()));
    for ch in s.chars() {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            out.push(' ');
        } else {
            out.push(ch);
        }
        if out.chars().count() >= max_chars {
            break;
        }
    }
    out
}

// Indicator window colors
const INDICATOR_COLOR_RECORDING: &str = "#4f9d9a"; // Indigo-cyan for normal recording
const INDICATOR_COLOR_LLM: &str = "#dc2626"; // Red for LLM processing

/// Show the indicator window and set its color
fn show_indicator_window<R: Runtime>(app_handle: &AppHandle<R>, is_llm: bool) {
    if let Some(window) = app_handle.get_webview_window("indicator") {
        let color = if is_llm { INDICATOR_COLOR_LLM } else { INDICATOR_COLOR_RECORDING };
        
        // 获取最新的鼠标位置并立即移动窗口到该位置
        let listener = app_handle.state::<InputListenerState>();
        let (x, y) = listener.get_last_mouse_position();
        
        // 计算偏移后的位置
        // emoji 已改为左上角对齐（indicator.html），所以：
        //   offset_x = 想要的"鼠标 → emoji左边缘"距离
        //   offset_y = 想要的"鼠标 → emoji顶部"距离（负数=emoji中心对齐鼠标）
        // 当前 fontSize=16，要让 emoji 垂直居中对齐鼠标点，offset_y = -fontSize/2 = -8
        let offset_x: f64 = 12.0;   // emoji 左边缘在鼠标右侧 6px
        let offset_y: f64 = -12.0;  // emoji 中心与鼠标 y 对齐（fontSize=16 时）
        let pos = tauri::PhysicalPosition::new((x + offset_x) as i32, (y + offset_y) as i32);
        
        // 先移动到正确位置，再显示窗口
        window.set_position(pos).ok();
        window.emit("indicator_color", color).ok();
        window.show().ok();
        
        // 启用鼠标跟踪以便后续移动
        listener.track_mouse_position.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Hide the indicator window
fn hide_indicator_window<R: Runtime>(app_handle: &AppHandle<R>) {
    if let Some(window) = app_handle.get_webview_window("indicator") {
        window.hide().ok();
    }
}

/// Move the indicator window to follow the mouse
fn move_indicator_window<R: Runtime>(app_handle: &AppHandle<R>, x: f64, y: f64) {
    if let Some(window) = app_handle.get_webview_window("indicator") {
        // 与 show_indicator_window 保持一致
        // emoji 左上角对齐后：offset 直接就是"鼠标 → emoji"的距离
        let offset_x: f64 = 12.0;   // emoji 左边缘在鼠标右侧 6px
        let offset_y: f64 = -12.0;  // emoji 中心与鼠标 y 对齐
        let pos = tauri::PhysicalPosition::new((x + offset_x) as i32, (y + offset_y) as i32);
        window.set_position(pos).ok();
    }
}

/// Process transcribed text: apply LLM correction if enabled, save to history, emit event, paste
fn process_transcription<R: Runtime>(
    app_handle: &AppHandle<R>,
    text: String,
    processing: ProcessingState,
    seq_id: u64,
) {
    if text.trim().is_empty() {
        println!("[TRANSCRIPTION] #{} empty, skipping", seq_id);
        processing.store(false, std::sync::atomic::Ordering::SeqCst);
        return;
    }
    
    println!(
        "[TRANSCRIPTION] #{} Processing: {} chars, preview='{}'",
        seq_id,
        text.len(),
        preview_text(&text, 80)
    );

    let storage = app_handle.state::<StorageState>();
    let config = storage.load_config();
    let llm_config = config.llm_config.clone();
    let proxy_config = config.proxy.clone();

    let app_handle_clone = app_handle.clone();
    let processing_clone = processing.clone();

    // Use tokio runtime to handle async LLM correction
    tauri::async_runtime::spawn(async move {
        // Always clear the processing flag when this async pipeline is done
        struct ProcessingGuard(ProcessingState);
        impl Drop for ProcessingGuard {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _guard = ProcessingGuard(processing_clone);

        let final_text = if llm_config.enabled && !llm_config.api_key.is_empty() {
            app_handle_clone.emit("llm_processing", true).ok();
            {
                let listener = app_handle_clone.state::<InputListenerState>();
                listener.track_mouse_position.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            show_indicator_window(&app_handle_clone, true);

            let result = match llm::correct_text(&text, &llm_config, &proxy_config).await {
                Ok(corrected) => corrected,
                Err(e) => {
                    eprintln!("LLM correction failed, using original text: {}", e);
                    text
                }
            };

            app_handle_clone.emit("llm_processing", false).ok();
            {
                let listener = app_handle_clone.state::<InputListenerState>();
                listener.track_mouse_position.store(false, std::sync::atomic::Ordering::Relaxed);
            }
            hide_indicator_window(&app_handle_clone);
            result
        } else {
            text
        };

        if final_text.trim().is_empty() {
            println!("[TRANSCRIPTION] #{} final empty, skipping", seq_id);
            return;
        }

        // Save to history
        let item = HistoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            text: final_text.clone(),
            duration_ms: 0,
        };
        let storage = app_handle_clone.state::<StorageState>();
        storage.add_history_item(item.clone()).ok();
        app_handle_clone.emit("transcription_update", item).ok();

        // Output text (blocking, on a dedicated thread to not block tokio)
        let text_to_paste = final_text;
        let id = seq_id;
        std::thread::spawn(move || {
            output_text(&text_to_paste, id);
        }).join().ok();
    });
}

/// 将识别结果输出到当前焦点窗口
/// 使用 enigo.text() 直接输入文本
fn output_text(text: &str, seq_id: u64) {
    println!("[OUTPUT] #{} start: {} chars", seq_id, text.len());

    // 等待目标窗口完成鼠标/键盘事件处理
    // 这对于鼠标中键触发的场景尤其重要，某些 Windows 原生控件需要时间处理中键释放
    std::thread::sleep(std::time::Duration::from_millis(80));

    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[OUTPUT] #{} enigo init failed: {:?}", seq_id, e);
            return;
        }
    };

    // 直接输入文本
    if let Err(e) = enigo.text(text) {
        eprintln!("[OUTPUT] #{} text input failed: {:?}", seq_id, e);
        return;
    }

    println!("[OUTPUT] #{} done", seq_id);
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
    let proxy = config.proxy.clone();

    // Run download in background
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let res = model_manager::download_model(&model_dir, &proxy, move |current, total| {
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
    cancel_state: tauri::State<'_, DownloadCancelState>,
    version: String
) -> Result<(), String> {
    let config = state.load_config();
    let model_dir = config.model_dir.clone();
    let language = config.language.clone();
    let proxy = config.proxy.clone();
    let model_version = match version.as_str() {
        "quantized" => ModelVersion::Quantized,
        "unquantized" => ModelVersion::Unquantized,
        _ => return Err("Invalid version".to_string()),
    };

    // Create cancellation token
    let cancel_token = CancellationToken::new();
    {
        let mut guard = cancel_state.lock().map_err(|e| e.to_string())?;
        *guard = Some(cancel_token.clone());
    }

    let handle = app.clone();
    let version_for_download = model_version.clone();
    let asr_clone = asr.inner().clone();

    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let res = model_manager::download_model_version(
            &model_dir,
            &version_for_download,
            &proxy,
            cancel_token,
            move |current, total| {
                progress_handle.emit("download_progress", serde_json::json!({ "current": current, "total": total })).ok();
            }
        ).await;

        // Clear the cancel token
        let cancel_state = handle.state::<DownloadCancelState>();
        if let Ok(mut guard) = cancel_state.lock() {
            *guard = None;
        }

        match res {
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("cancelled") {
                    handle.emit("download_cancelled", ()).ok();
                } else {
                    handle.emit("download_error", error_msg).ok();
                }
            }
            Ok(_) => {
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
async fn cancel_download(
    cancel_state: tauri::State<'_, DownloadCancelState>
) -> Result<(), String> {
    let guard = cancel_state.lock().map_err(|e| e.to_string())?;
    if let Some(token) = guard.as_ref() {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
async fn import_model<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, StorageState>,
    asr: tauri::State<'_, AsrState>,
    file_path: String,
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

    // Run import in background
    let handle = app.clone();
    let version_for_import = model_version.clone();
    let asr_clone = asr.inner().clone();

    tauri::async_runtime::spawn(async move {
        // Emit importing status
        handle.emit("import_started", ()).ok();

        let res = model_manager::import_model_from_file(&file_path, &model_dir, &version_for_import);

        match res {
            Err(e) => {
                handle.emit("import_error", e.to_string()).ok();
            }
            Ok(_) => {
                // Import complete - now auto-load the model
                handle.emit("import_complete", ()).ok();

                // Update config to use this version - get state from app handle
                let storage = handle.state::<StorageState>();
                let mut new_config = storage.load_config();
                new_config.model_version = version_for_import.clone();
                let _ = storage.save_config(&new_config);

                // Load the model
                let model_path = model_manager::get_model_dir_for_version(&model_dir, &version_for_import);
                match asr_clone.load_model(model_path, language) {
                    Ok(_) => {
                        handle.emit("model_loaded", ()).ok();
                    },
                    Err(e) => eprintln!("Failed to auto-load model after import: {}", e),
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn open_model_folder(state: tauri::State<'_, StorageState>) -> Result<(), String> {
    let config = state.load_config();
    let model_dir = std::path::Path::new(&config.model_dir);

    // Create the directory if it doesn't exist
    if !model_dir.exists() {
        std::fs::create_dir_all(model_dir).map_err(|e| e.to_string())?;
    }

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

#[tauri::command]
async fn test_llm_connection(config: LlmConfig, proxy: ProxyConfig) -> Result<String, String> {
    llm::test_connection(&config, &proxy).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_default_llm_prompt() -> String {
    storage::DEFAULT_LLM_PROMPT.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Create indicator window
            println!("Creating indicator window...");
            let indicator_url = if cfg!(dev) {
                // Development mode: use dev server
                WebviewUrl::External("http://127.0.0.1:1420/indicator.html".parse().unwrap())
            } else {
                // Production mode: use bundled file
                WebviewUrl::App("indicator.html".into())
            };
            println!("Indicator URL: {:?}", indicator_url);

            match WebviewWindowBuilder::new(app, "indicator", indicator_url)
                .title("")
                .inner_size(18.0, 18.0) // 单个emoji，40x40足够
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .shadow(false)
                .focused(false)
                .build()
            {
                Ok(window) => {
                    println!("Indicator window created successfully: {:?}", window.label());
                },
                Err(e) => eprintln!("Failed to create indicator window: {:?}", e),
            }

            // Initialize Storage (config in AppData\Roaming)
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("data"));
            let storage_service = storage::StorageService::new(app_dir.clone());
            let mut config = storage_service.load_config();

            // Use AppData\Local for models (less likely to be deleted on uninstall)
            let local_data_dir = app.path().app_local_data_dir().unwrap_or_else(|_| app_dir.clone());

            // Initialize Services
            let asr_service = asr::AsrService::new();

            // Fix model path to be in AppData\Local if it's the default relative path
            // This prevents "Rebuilding application" loops during download when running in dev mode
            // and keeps models separate from app data that may be cleaned on uninstall
            if config.model_dir == "./models/sense-voice" || config.model_dir.contains("AppData\\Roaming") {
                let model_path = local_data_dir.join("models").join("sense-voice");
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

            // Try to initialize with configured device, fallback to default if it fails
            let device_init_result = audio_service.init_with_device(&config.input_device, app_handle.clone());

            if let Err(e) = device_init_result {
                eprintln!("Failed to init audio with configured device '{}': {}", config.input_device, e);
                eprintln!("Attempting to fallback to default audio device...");

                // Try to initialize with empty device name (default device)
                match audio_service.init_with_device("", app_handle.clone()) {
                    Ok(_) => {
                        println!("Successfully initialized with default audio device");
                        println!("Please select your preferred device in Settings");
                        // Do NOT update config - keep the original device name so user can see what was selected before
                    },
                    Err(fallback_err) => {
                        eprintln!("Failed to init audio with default device: {}", fallback_err);
                        eprintln!("Application will continue but audio recording will not work until a device is selected in settings.");
                    }
                }
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

            // Shared processing flag:
            // We must NOT allow a new transcription/paste to start while the previous async
            // pipeline (LLM + enigo typing) is still running; otherwise keystrokes interleave
            // and output becomes garbled/duplicated.
            let processing_state: ProcessingState = Arc::new(std::sync::atomic::AtomicBool::new(false));

            // Background Thread to handle events
            let processing_for_thread = processing_state.clone();
            std::thread::spawn(move || {
                let mut is_recording = false;

                for event in rx {
                    match event {
                        input_listener::InputEvent::Start => {
                            if !is_recording && !processing_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
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
                                    // Enable mouse tracking for indicator window
                                    let listener = app_handle.state::<InputListenerState>();
                                    listener.track_mouse_position.store(true, std::sync::atomic::Ordering::Relaxed);
                                    // Show indicator window (normal recording = indigo-cyan)
                                    show_indicator_window(&app_handle, false);
                                }
                            }
                        },
                        input_listener::InputEvent::Stop => {
                            if is_recording && !processing_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                                // Stop & Transcribe
                                is_recording = false;

                                // Mark as processing atomically; if another thread already did, bail.
                                if processing_for_thread
                                    .compare_exchange(
                                        false,
                                        true,
                                        std::sync::atomic::Ordering::SeqCst,
                                        std::sync::atomic::Ordering::SeqCst,
                                    )
                                    .is_err()
                                {
                                    continue;
                                }
                                
                                app_handle.emit("recording_status", false).ok();
                                // Disable mouse tracking (will re-enable if LLM processing starts)
                                let listener = app_handle.state::<InputListenerState>();
                                listener.track_mouse_position.store(false, std::sync::atomic::Ordering::Relaxed);
                                // Hide indicator window (will re-show if LLM processing)
                                hide_indicator_window(&app_handle);

                                let audio = app_handle.state::<AudioState>();
                                let mut buffer = Vec::new();
                                let mut sample_rate = 48000u32;
                                if let Ok(ref audio) = audio.lock() {
                                    sample_rate = audio.get_sample_rate();
                                    if let Ok(b) = audio.stop_recording() {
                                        buffer = b;
                                    }
                                }

                                    let asr = app_handle.state::<AsrState>();
                                    // Transcribe with actual sample rate
                                    match asr.transcribe(buffer, sample_rate) {
                                        Ok(text) => {
                                            let seq_id = TRANSCRIPTION_SEQ.fetch_add(1, AtomicOrdering::Relaxed);
                                            println!(
                                                "[STOP] #{} Transcribed {} chars, preview='{}'",
                                                seq_id,
                                                text.len(),
                                                preview_text(&text, 80)
                                            );
                                            process_transcription(&app_handle, text, processing_for_thread.clone(), seq_id);
                                        },
                                        Err(e) => {
                                            eprintln!("[STOP] Transcription error: {}", e);
                                            processing_for_thread.store(false, std::sync::atomic::Ordering::SeqCst);
                                        }
                                    }
                            }
                        },
                        input_listener::InputEvent::Toggle => {
                            if is_recording && !processing_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
                                // Stop & Transcribe (same as Stop)
                                is_recording = false;

                                if processing_for_thread
                                    .compare_exchange(
                                        false,
                                        true,
                                        std::sync::atomic::Ordering::SeqCst,
                                        std::sync::atomic::Ordering::SeqCst,
                                    )
                                    .is_err()
                                {
                                    continue;
                                }
                                app_handle.emit("recording_status", false).ok();
                                // Disable mouse tracking
                                let listener = app_handle.state::<InputListenerState>();
                                listener.track_mouse_position.store(false, std::sync::atomic::Ordering::Relaxed);
                                // Hide indicator window
                                hide_indicator_window(&app_handle);

                                let audio = app_handle.state::<AudioState>();
                                let mut buffer = Vec::new();
                                let mut sample_rate = 48000u32;
                                if let Ok(ref audio) = audio.lock() {
                                    sample_rate = audio.get_sample_rate();
                                    if let Ok(b) = audio.stop_recording() {
                                        buffer = b;
                                    }
                                }

                                    let asr = app_handle.state::<AsrState>();
                                    match asr.transcribe(buffer, sample_rate) {
                                        Ok(text) => {
                                            let seq_id = TRANSCRIPTION_SEQ.fetch_add(1, AtomicOrdering::Relaxed);
                                            println!(
                                                "[TOGGLE] #{} Transcribed {} chars, preview='{}'",
                                                seq_id,
                                                text.len(),
                                                preview_text(&text, 80)
                                            );
                                            process_transcription(&app_handle, text, processing_for_thread.clone(), seq_id);
                                        },
                                        Err(e) => {
                                            eprintln!("[TOGGLE] Transcription error: {}", e);
                                            processing_for_thread.store(false, std::sync::atomic::Ordering::SeqCst);
                                        }
                                    }
                            } else if !is_recording && !processing_for_thread.load(std::sync::atomic::Ordering::SeqCst) {
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
                                    // Enable mouse tracking
                                    let listener = app_handle.state::<InputListenerState>();
                                    listener.track_mouse_position.store(true, std::sync::atomic::Ordering::Relaxed);
                                    // Show indicator window
                                    show_indicator_window(&app_handle, false);
                                }
                            }
                        },
                        input_listener::InputEvent::MouseMove { x, y } => {
                            // Move indicator window to follow mouse
                            move_indicator_window(&app_handle, x, y);
                        }
                    }
                }
            });

            // manage states
            app.manage(audio_state);
            app.manage(asr_service);
            app.manage(storage_service);
            app.manage(input_listener); // expose to commands if needed (to update config)
            app.manage(processing_state);
            app.manage(Mutex::new(None::<CancellationToken>) as DownloadCancelState);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config, save_config, get_history, clear_history,
            check_model_status, download_model, open_model_folder,
            get_model_versions_status, get_model_detailed_status,
            download_model_for_version, switch_model_version, cancel_download, import_model,
            get_input_devices, get_current_input_device, switch_input_device,
            start_audio_test, stop_audio_test,
            test_llm_connection, get_default_llm_prompt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
