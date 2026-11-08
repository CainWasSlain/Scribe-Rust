use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Child, Command};

fn pw_record(extra: &[&str], out: &Path) -> Result<Child> {
    Command::new("pw-record")
        .args(extra)
        .args(["--rate", "16000", "--channels", "1", "--format", "s16"])
        .arg(out)
        .spawn()
        .context("could not start pw-record (install with: sudo dnf install pipewire-utils)")
}

/// Records mic + whatever the laptop plays until Ctrl+C.
/// Ctrl+C in a terminal signals the whole process group, so both pw-record
/// children get SIGINT themselves and finalise their WAV headers; we just wait.
pub fn record(dir: &Path) -> Result<()> {
    // First Ctrl+C stops recording (children exit); a second one aborts transcription too.
    let pressed = std::sync::atomic::AtomicBool::new(false);
    ctrlc::set_handler(move || {
        if pressed.swap(true, std::sync::atomic::Ordering::SeqCst) {
            eprintln!("\nAborted. Audio kept; re-run: scribe transcribe <session-dir>");
            std::process::exit(130);
        }
    })
    .context("installing Ctrl+C handler")?;
    let mut mic = pw_record(&[], &dir.join("mic.wav"))?;
    let mut sys = pw_record(&["-P", "{ stream.capture.sink=true }"], &dir.join("system.wav"))?;
    println!("Recording to {} — press Ctrl+C to stop.", dir.display());
    let (a, b) = (mic.wait()?, sys.wait()?);
    for (name, file) in [("mic", "mic.wav"), ("system", "system.wav")] {
        if !dir.join(file).exists() {
            bail!("{name} recording missing (pw-record exit: mic {a}, system {b})");
        }
    }
    Ok(())
}
