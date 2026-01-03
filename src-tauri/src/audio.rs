use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

pub struct AudioService {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    sample_rate: Arc<AtomicU32>,
    current_device_name: Arc<Mutex<String>>,
}

unsafe impl Send for AudioService {}
unsafe impl Sync for AudioService {}

impl AudioService {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            sample_rate: Arc::new(AtomicU32::new(16000)),
            current_device_name: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Get list of available input devices
    pub fn get_input_devices() -> Vec<AudioDevice> {
        let host = cpal::default_host();
        let default_device = host.default_input_device();
        let default_name = default_device.and_then(|d| d.name().ok()).unwrap_or_default();

        let mut devices = Vec::new();
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    devices.push(AudioDevice {
                        is_default: name == default_name,
                        name,
                    });
                }
            }
        }
        devices
    }

    /// Initialize with specific device name (empty for default)
    pub fn init_with_device<R: tauri::Runtime>(&mut self, device_name: &str, app_handle: tauri::AppHandle<R>) -> Result<()> {
        let host = cpal::default_host();
        
        let device = if device_name.is_empty() {
            host.default_input_device().ok_or(anyhow::anyhow!("No default input device"))?
        } else {
            host.input_devices()?
                .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
                .ok_or(anyhow::anyhow!("Device not found: {}", device_name))?
        };

        let actual_name = device.name()?;
        println!("Using input device: {}", actual_name);
        *self.current_device_name.lock().unwrap() = actual_name.clone();

        // Get default input config from device
        let default_config = device.default_input_config()?;
        println!("Default input config: {:?}", default_config);
        
        // Use the device's default sample rate and channels
        let sample_rate = default_config.sample_rate().0;
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        
        let config = cpal::StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        
        let channels = config.channels as usize;

        let buffer_clone = self.buffer.clone();
        let is_recording_clone = self.is_recording.clone();
        let app_handle_clone = app_handle.clone();

        // Counter for throttling events (emit approx every 50ms)
        // At 48kHz, buffer size is often ~480-1000 samples. 
        // We can just emit every chunk if it's not too frequent, or use a time check.
        // For visualizer responsiveness, ~30-60fps is good.
        let last_emit_time = Arc::new(Mutex::new(std::time::Instant::now()));

        // Build stream based on sample format
        let stream = match default_config.sample_format() {
            cpal::SampleFormat::F32 => {
                 device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &_| {
                        if is_recording_clone.load(Ordering::Relaxed) {
                            let mut buffer = buffer_clone.lock().unwrap();
                            
                            // Calculate RMS for visualization
                            let mut sum_squares = 0.0;
                            
                            // Convert stereo to mono if needed and calc RMS
                            if channels > 1 {
                                for chunk in data.chunks(channels) {
                                    let mono = chunk.iter().sum::<f32>() / channels as f32;
                                    buffer.push(mono);
                                    sum_squares += mono * mono;
                                }
                            } else {
                                buffer.extend_from_slice(data);
                                for &sample in data {
                                    sum_squares += sample * sample;
                                }
                            }
                            
                            // Emit level event
                            let sample_count = data.len() / channels;
                            if sample_count > 0 {
                                let rms = (sum_squares / sample_count as f32).sqrt();
                                
                                // Throttle emission to ~60fps (16ms)
                                let mut last_emit = last_emit_time.lock().unwrap();
                                if last_emit.elapsed().as_millis() >= 16 {
                                    use tauri::Emitter;
                                    let _ = app_handle_clone.emit("audio_level", rms);
                                    *last_emit = std::time::Instant::now();
                                }
                            }
                        }
                    },
                    move |err| {
                        eprintln!("Audio stream error: {}", err);
                    },
                    None
                )?
            },
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &_| {
                        if is_recording_clone.load(Ordering::Relaxed) {
                            let mut buffer = buffer_clone.lock().unwrap();
                            let mut sum_squares = 0.0;

                            // Convert i16 to f32 and stereo to mono if needed
                            if channels > 1 {
                                for chunk in data.chunks(channels) {
                                    let mono: f32 = chunk.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32;
                                    buffer.push(mono);
                                    sum_squares += mono * mono;
                                }
                            } else {
                                for &sample in data {
                                    let val = sample as f32 / 32768.0;
                                    buffer.push(val);
                                    sum_squares += val * val;
                                }
                            }
                            
                            // Emit level event
                             let sample_count = data.len() / channels;
                            if sample_count > 0 {
                                let rms = (sum_squares / sample_count as f32).sqrt();
                                
                                // Throttle emission
                                let mut last_emit = last_emit_time.lock().unwrap();
                                if last_emit.elapsed().as_millis() >= 16 {
                                    use tauri::Emitter;
                                    let _ = app_handle_clone.emit("audio_level", rms);
                                    *last_emit = std::time::Instant::now();
                                }
                            }

                        }
                    },
                    move |err| {
                        eprintln!("Audio stream error: {}", err);
                    },
                    None
                )?
            },
            format => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", format));
            }
        };

        stream.pause()?; // Start paused
        self.stream = Some(stream);
        println!("Audio initialized with sample rate: {}", sample_rate);
        Ok(())
    }



    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    pub fn get_current_device_name(&self) -> String {
        self.current_device_name.lock().unwrap().clone()
    }

    pub fn start_recording(&self) -> Result<()> {
        if let Some(ref stream) = self.stream {
            {
                let mut buffer = self.buffer.lock().unwrap();
                buffer.clear();
            }
            self.is_recording.store(true, Ordering::Relaxed);
            stream.play()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Audio stream not initialized"))
        }
    }

    pub fn stop_recording(&self) -> Result<Vec<f32>> {
        if let Some(ref stream) = self.stream {
            stream.pause()?;
            self.is_recording.store(false, Ordering::Relaxed);
            let buffer = self.buffer.lock().unwrap();
            Ok(buffer.clone())
        } else {
            Err(anyhow::anyhow!("Audio stream not initialized"))
        }
    }

    /// Start audio level test - just plays the stream without recording to buffer
    pub fn start_test(&self) -> Result<()> {
        if let Some(ref stream) = self.stream {
            self.is_recording.store(true, Ordering::Relaxed);
            stream.play()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Audio stream not initialized"))
        }
    }

    /// Stop audio level test
    pub fn stop_test(&self) -> Result<()> {
        if let Some(ref stream) = self.stream {
            stream.pause()?;
            self.is_recording.store(false, Ordering::Relaxed);
            // Clear any recorded buffer
            if let Ok(mut buffer) = self.buffer.lock() {
                buffer.clear();
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Audio stream not initialized"))
        }
    }
}

