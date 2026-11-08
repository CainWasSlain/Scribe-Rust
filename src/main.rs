mod docx_out;
mod manglish;
mod minutes;
mod record;
mod transcribe;

use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use transcribe::Segment;

/// Records Zoom / Meet / Teams calls (mic + speaker audio) and writes local Word minutes.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args)]
struct SttOpts {
    /// Spoken language: auto-detect per 30 s, or force English / Malayalam
    #[arg(long, default_value = "auto", value_parser = ["auto", "en", "ml"])]
    lang: String,
    /// Whisper ggml model file
    #[arg(long)]
    model: Option<PathBuf>,
    /// Keep mic.wav / system.wav after the .docx is written
    #[arg(long)]
    keep_audio: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Record until Ctrl+C, then transcribe and write transcript.docx
    Record {
        #[command(flatten)]
        stt: SttOpts,
        /// Parent folder for meeting sessions
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Re-transcribe a session folder that still has its audio
    Transcribe {
        dir: PathBuf,
        #[command(flatten)]
        stt: SttOpts,
    },
    /// Rebuild the minutes/.docx from a session's transcript.json
    Summarize { dir: PathBuf },
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
}

fn model_path(stt: &SttOpts) -> Result<PathBuf> {
    let p = stt
        .model
        .clone()
        .unwrap_or_else(|| home().join(".local/share/meeting-scribe/ggml-large-v3-turbo.bin"));
    ensure!(p.exists(), "Whisper model not found at {} (see README to download it)", p.display());
    Ok(p)
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Record { stt, out } => {
            let model = model_path(&stt)?; // fail before the meeting, not after
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M").to_string();
            let dir = out.unwrap_or_else(|| home().join("Documents/Meetings")).join(&stamp);
            std::fs::create_dir_all(&dir)?;
            record::record(&dir)?;
            transcribe_and_write(&dir, &model, &stt)
        }
        Cmd::Transcribe { dir, stt } => transcribe_and_write(&dir, &model_path(&stt)?, &stt),
        Cmd::Summarize { dir } => {
            let segs: Vec<Segment> = serde_json::from_slice(
                &std::fs::read(dir.join("transcript.json")).context("reading transcript.json")?,
            )?;
            write_docx(&dir, &segs)
        }
    }
}

fn transcribe_and_write(dir: &Path, model: &Path, stt: &SttOpts) -> Result<()> {
    println!("Transcribing (Ctrl+C again to abort; audio is kept)…");
    let segs = transcribe::transcribe_dir(dir, model, &stt.lang)?; // model freed on return
    let segs = minutes::remove_echo(segs);
    std::fs::write(dir.join("transcript.json"), serde_json::to_vec_pretty(&segs)?)?;
    write_docx(dir, &segs)?;
    if !stt.keep_audio {
        delete_audio(dir)?;
    }
    Ok(())
}

fn write_docx(dir: &Path, segs: &[Segment]) -> Result<()> {
    // transcript.json keeps the original script; the document is written in Manglish
    let segs: Vec<Segment> = segs
        .iter()
        .map(|s| Segment { text: manglish::to_manglish(&s.text), ..s.clone() })
        .collect();
    let segs = &segs[..];
    let name = dir.file_name().map(|n| n.to_string_lossy().replace('_', " ")).unwrap_or_default();
    let path = dir.join("transcript.docx");
    docx_out::write(&path, &name, segs, &minutes::build(segs))?;
    println!("Wrote {}", path.display());
    Ok(())
}

/// Deletes the WAVs only once a non-empty transcript.docx exists, so a failed run never loses audio.
fn delete_audio(dir: &Path) -> Result<()> {
    let size = std::fs::metadata(dir.join("transcript.docx")).map(|m| m.len()).unwrap_or(0);
    ensure!(size > 0, "transcript.docx missing or empty; keeping audio");
    for f in ["mic.wav", "system.wav"] {
        let p = dir.join(f);
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_only_deleted_when_docx_exists() {
        let dir = std::env::temp_dir().join(format!("scribe-clean-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mic.wav"), b"x").unwrap();
        assert!(delete_audio(&dir).is_err());
        assert!(dir.join("mic.wav").exists());
        std::fs::write(dir.join("transcript.docx"), b"PK").unwrap();
        delete_audio(&dir).unwrap();
        assert!(!dir.join("mic.wav").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
