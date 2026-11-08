use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: String,
    pub lang: String,
    pub text: String,
}

const RATE: usize = 16_000;
const WINDOW: usize = 30 * RATE; // Whisper's native window; language is detected per window
const SILENCE_RMS: f32 = 0.005;

pub fn is_silent(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let ms = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    ms.sqrt() < SILENCE_RMS
}

pub fn load_wav(path: &Path) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(path).with_context(|| format!("opening {}", path.display()))?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate as usize == RATE && spec.channels == 1 && spec.bits_per_sample == 16,
        "{} must be 16 kHz mono 16-bit (got {:?})",
        path.display(),
        spec
    );
    reader
        .into_samples::<i16>()
        .map(|s| Ok(s? as f32 / 32768.0))
        .collect()
}

/// Transcribes mic.wav (Me) and system.wav (Others). The Whisper model is dropped
/// when this returns, freeing its VRAM/RAM before the rest of the pipeline runs.
#[cfg(feature = "stt")]
pub fn transcribe_dir(dir: &Path, model: &Path, lang: &str) -> Result<Vec<Segment>> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let ctx = WhisperContext::new_with_params(
        model.to_str().context("model path is not UTF-8")?,
        WhisperContextParameters::default(),
    )
    .with_context(|| format!("loading Whisper model {}", model.display()))?;
    let mut state = ctx.create_state()?;
    let mut out = Vec::new();

    for (file, speaker) in [("mic.wav", "Me"), ("system.wav", "Others")] {
        let audio = load_wav(&dir.join(file))?;
        let windows = audio.len().div_ceil(WINDOW);
        for (w, chunk) in audio.chunks(WINDOW).enumerate() {
            eprint!("\r{speaker}: window {}/{windows}   ", w + 1);
            if is_silent(chunk) {
                continue;
            }
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_language(Some(lang));
            p.set_no_context(true); // windows are independent; stops one bad window poisoning the next
            p.set_suppress_blank(true);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_special(false);
            p.set_print_timestamps(false);
            state.full(p, chunk)?;
            let detected = whisper_rs::get_lang_str(state.full_lang_id_from_state())
                .unwrap_or(lang)
                .to_string();
            let offset = (w * WINDOW * 1000 / RATE) as i64;
            for seg in state.as_iter() {
                let text = seg.to_str_lossy()?.trim().to_string();
                if text.is_empty() || text.starts_with('[') || text.starts_with('(') {
                    continue; // [Music], (silence) etc.
                }
                out.push(Segment {
                    // Whisper timestamps are in 10 ms units
                    start_ms: offset + seg.start_timestamp() * 10,
                    end_ms: offset + seg.end_timestamp() * 10,
                    speaker: speaker.into(),
                    lang: detected.clone(),
                    text,
                });
            }
        }
        eprintln!();
    }
    out.sort_by_key(|s| s.start_ms);
    Ok(out)
}

#[cfg(not(feature = "stt"))]
pub fn transcribe_dir(_: &Path, _: &Path, _: &str) -> Result<Vec<Segment>> {
    anyhow::bail!("built without speech-to-text; rebuild with default features")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_detection() {
        assert!(is_silent(&[0.0; 1600]));
        assert!(is_silent(&[]));
        let tone: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.1).sin() * 0.2).collect();
        assert!(!is_silent(&tone));
    }
}
