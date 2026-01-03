import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ModelVersion = "quantized" | "unquantized";

export interface ModelVersionsStatus {
    quantized: boolean;
    unquantized: boolean;
    current: ModelVersion;
}

export interface ModelDetailedStatus {
    downloaded: boolean;
    loaded: boolean;
}

export interface AudioDevice {
    name: string;
    is_default: boolean;
}

export interface AppConfig {
    trigger_mouse: boolean;
    trigger_hold: boolean;
    trigger_toggle: boolean;
    language: string;
    model_dir: string;
    model_version: ModelVersion;
    input_device: string;
}

export interface HistoryItem {
    id: string;
    timestamp: string;
    text: string;
    duration_ms: number;
}

export const api = {
    getConfig: () => invoke<AppConfig>("get_config"),
    saveConfig: (config: AppConfig) => invoke("save_config", { config }),
    getHistory: () => invoke<HistoryItem[]>("get_history"),
    clearHistory: () => invoke("clear_history"),
    checkModelStatus: () => invoke<boolean>("check_model_status"),
    getDetailedStatus: () => invoke<ModelDetailedStatus>("get_model_detailed_status"),
    downloadModel: () => invoke("download_model"),
    openModelFolder: () => invoke("open_model_folder"),
    getModelVersionsStatus: () => invoke<ModelVersionsStatus>("get_model_versions_status"),
    downloadModelForVersion: (version: ModelVersion) => invoke("download_model_for_version", { version }),
    switchModelVersion: (version: ModelVersion) => invoke("switch_model_version", { version }),
    // Audio device APIs
    getInputDevices: () => invoke<AudioDevice[]>("get_input_devices"),
    getCurrentInputDevice: () => invoke<string>("get_current_input_device"),
    switchInputDevice: (deviceName: string) => invoke("switch_input_device", { deviceName }),
};

export const events = {
    onTranscriptionUpdate: (callback: (payload: HistoryItem) => void) => listen<HistoryItem>("transcription_update", (e) => callback(e.payload)),
    onRecordingStatus: (callback: (isRecording: boolean) => void) => listen<boolean>("recording_status", (e) => callback(e.payload)),
    onDownloadProgress: (callback: (payload: { current: number, total: number }) => void) => listen("download_progress", (e) => callback(e.payload as any)),
    onDownloadComplete: (callback: () => void) => listen("download_complete", callback),
    onDownloadError: (callback: (error: string) => void) => listen("download_error", (e) => callback(e.payload as string)),
    onModelLoaded: (callback: () => void) => listen("model_loaded", callback),
};

