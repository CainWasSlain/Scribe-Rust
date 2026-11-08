# meeting-scribe

Records any Zoom / Google Meet / Teams call on Fedora (your mic + whatever the laptop plays),
transcribes it **offline** with Whisper (English, Malayalam, auto-detected per 30 s), and writes
`transcript.docx` with minutes: topics, summary, key points, decisions, action items, and the full
transcript with key lines highlighted. No cloud, no AI summarizer — the minutes are rule-based.

## One-time setup (Fedora)

```bash
sudo dnf install rust cargo cmake clang-devel gcc-c++ pipewire-utils vulkan-headers vulkan-loader-devel glslc
```

NVIDIA driver (RPM Fusion), needed for the GPU to be used via Vulkan:

```bash
sudo dnf install https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm
```

```bash
sudo dnf install akmod-nvidia vulkan-tools
```

Reboot, then `vulkaninfo --summary` should list the RTX 3060.

Whisper model (one download, ~1.6 GB; after this nothing touches the network):

```bash
mkdir -p ~/.local/share/meeting-scribe && curl -L -o ~/.local/share/meeting-scribe/ggml-large-v3-turbo.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin
```

For better Malayalam, also get `ggml-large-v3.bin` (~3 GB) the same way and pass `--model`.

Build and install:

```bash
cargo install --path .
```

(CPU only: `cargo install --path . --no-default-features --features stt`. CUDA instead of Vulkan: `--no-default-features --features cuda`, needs NVIDIA's CUDA toolkit.)

## Use

```bash
scribe record
```

Join your call, talk, press **Ctrl+C** when it ends. Output lands in
`~/Documents/Meetings/<date_time>/transcript.docx`. The audio files are deleted once the .docx is
written (`--keep-audio` to keep them), and the Whisper model is unloaded from GPU/RAM before the
document is built; the program then exits.

- `--lang en` / `--lang ml` forces a language (default `auto`).
- `scribe transcribe <folder>` re-runs transcription if you kept the audio (or it failed).
- `scribe summarize <folder>` rebuilds the minutes from `transcript.json`, e.g. after you edit the
  cue-word lists at the top of `src/minutes.rs` and reinstall.

## Good to know

- Malayalam speech is written in **Manglish** in the .docx (e.g. "ente report naale ayakkaam"),
  converted from Malayalam script by spelling rules in `src/manglish.rs`. `transcript.json` keeps
  the original script.

- Labels are **Me** (your mic) and **Others** (call audio); individual remote speakers aren't told apart.
- Speakers instead of headphones work: echo of the others picked up by your mic is removed by
  matching text. Talking over someone may still leak a little. For cleaner audio you can enable
  PipeWire's echo-cancel module.
- Minutes quote the meeting verbatim and use English cue words for decisions/action items — skim
  them before sending.
