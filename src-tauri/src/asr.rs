use sherpa_onnx::sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer};
use anyhow::Result;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AsrService {
    recognizer: Arc<Mutex<Option<SenseVoiceRecognizer>>>,
}

impl AsrService {
    pub fn new() -> Self {
        Self {
            recognizer: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.recognizer.lock().unwrap().is_some()
    }

    pub fn load_model(&self, model_dir: String, language: String) -> Result<()> {
        let model_path = format!("{}/model.onnx", model_dir);
        let tokens_path = format!("{}/tokens.txt", model_dir);

        // println!("Loading model from: {}", model_path);

        let config = SenseVoiceConfig {
            model: model_path,
            tokens: tokens_path,
            language,
            use_itn: true,
            ..Default::default()
        };

        let recognizer = SenseVoiceRecognizer::new(config).map_err(|e| anyhow::anyhow!("{}", e))?;
        *self.recognizer.lock().unwrap() = Some(recognizer);
        // println!("Model loaded successfully");
        Ok(())
    }

    pub fn transcribe(&self, samples: Vec<f32>, sample_rate: u32) -> Result<String> {
        let mut guard = self.recognizer.lock().unwrap();
        if let Some(recognizer) = guard.as_mut() {
            // SenseVoice expects 16kHz. Resample if needed.
            let (resampled, target_rate) = if sample_rate != 16000 {
                // println!("Resampling from {}Hz to 16000Hz ({} samples)", sample_rate, samples.len());
                let resampled = resample_to_16k(&samples, sample_rate);
                // println!("Resampled to {} samples", resampled.len());
                (resampled, 16000)
            } else {
                (samples, 16000)
            };
            
            let result = recognizer.transcribe(target_rate, &resampled);
            Ok(result.text)
        } else {
            Err(anyhow::anyhow!("Model not loaded"))
        }
    }
}

/// Resample audio from source_rate to 16000Hz using linear interpolation
fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 16000 || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = source_rate as f64 / 16000.0;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos.floor() as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < samples.len() {
            // Linear interpolation between two samples
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}
