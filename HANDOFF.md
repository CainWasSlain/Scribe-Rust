# Handoff: meeting-scribe

## What it is
A Rust command-line app (`scribe`) for **Fedora**. It records any Zoom / Google Meet / Teams call and turns it into meeting minutes in a Word file. **Everything stays on the laptop.** After a one-time download of the Whisper model, nothing goes online.

Owner's machine: Fedora on a Ryzen 7 5800H laptop with an RTX 3060 Laptop GPU (6 GB) and 16 GB RAM.

## Requirements (from the owner)
- **Platform:** Rust, command line only, Fedora.
- **Calls:** works with any meeting app, and doesn't join the call as a bot.
- **Languages:** English, sometimes Malayalam. Malayalam must be written in **Manglish** (Malayalam in English letters).
- **Output:** a `.docx` laid out as minutes: summary, key points highlighted, decisions, action items.
- **No AI for the summary:** the minutes must be built from rules and word statistics. Whisper is still used for speech-to-text, running locally.
- **Cleanup:**
  - Delete the audio once the `.docx` is written.
  - Free GPU and RAM after transcription.
- **Headphones may not be used,** so echo from the laptop speakers has to be handled.

## How it works
1. **`scribe record`**
   - Starts two `pw-record` processes (from `pipewire-utils`), both writing 16 kHz mono 16-bit WAV:
     - `mic.wav`: the microphone, labelled **Me**.
     - `system.wav`: whatever the laptop plays (`-P '{ stream.capture.sink=true }'`), labelled **Others**.
   - Ctrl+C reaches the whole process group, so both `pw-record` processes stop and finish their files.
   - A second Ctrl+C aborts transcription and keeps the audio.
2. **Transcription** (`src/transcribe.rs`)
   - Uses `whisper-rs` 0.16 on 30-second windows, detecting the language for each window.
   - Skips silent windows (an RMS check) and drops non-speech tags like `[Music]` and `(silence)`.
   - The Whisper model is released when the function returns, which frees its GPU memory and RAM.
3. **Echo removal** (`minutes::remove_echo`): drops a Me segment if both are true:
   - It is within ±2 s of an Others segment.
   - At least 60% of its meaningful words (common words like "the" and "and" removed) also appear in that Others segment.
4. **`transcript.json`** is saved with the original script, so the minutes can be rebuilt later.
5. **Manglish** (`src/manglish.rs`) converts Malayalam script into English letters using fixed spelling rules. It's applied when the docx is written.
6. **Minutes** (`src/minutes.rs`):
   - **Summary and key points:** the highest-scoring sentences. A sentence scores higher when it contains words that come up repeatedly in the meeting; words said only once don't count. Sentences near the start or end of the meeting get a small boost.
   - **Decisions and action items:** found by English cue phrases. These lists are `const` arrays at the top of the file.
   - **Action item owner:** a name at the start of the sentence ("Rahul will …"), otherwise the speaker.
7. **Word file** (`src/docx_out.rs`, using `docx-rs`):
   - Sections in order: title, Topics, Summary, Key points, Decisions, an Action items table, then the Full transcript.
   - Key-point lines are highlighted yellow in the transcript.
8. **Audio cleanup** (`delete_audio` in `main.rs`): the WAVs are deleted only after a non-empty `transcript.docx` exists. `--keep-audio` skips the deletion.

### Commands
- `scribe record [--lang auto|en|ml] [--model PATH] [--out DIR] [--keep-audio]`
- `scribe transcribe <session-dir>` re-runs transcription on audio that was kept.
- `scribe summarize <session-dir>` rebuilds the docx from `transcript.json`.

### Default locations
- **Sessions:** `~/Documents/Meetings/<YYYY-MM-DD_HHMM>/`
- **Whisper model:** `~/.local/share/meeting-scribe/ggml-large-v3-turbo.bin`

## Build
- **Cargo features:**
  - `vulkan` (the default) uses the GPU through Vulkan.
  - `cuda` is optional.
  - `stt` alone gives a CPU-only build.
  - `--no-default-features` leaves out Whisper entirely. This builds quickly and is enough to run the unit tests.
- **Fedora packages:** `rust cargo cmake clang-devel gcc-c++ pipewire-utils vulkan-headers vulkan-loader-devel glslc`
- **whisper-rs must be 0.16 or newer.** Version 0.14's generated bindings don't compile with Fedora's clang 23.
- **Tests:** `cargo test --no-default-features` runs 12 tests: echo removal, minutes extraction, Manglish, Word output, mm:ss formatting, silence detection and audio cleanup.

## Status
- **Verified in a `fedora:latest` container:**
  - The Vulkan build compiles.
  - All 12 tests pass.
  - A full run on computer-generated English speech (tiny model, CPU) produced the expected minutes, and the audio was deleted.
- **Not yet tested on real hardware:**
  - Recording a live call with `pw-record`.
  - Whisper running on the GPU through Vulkan.
  - Real Malayalam audio and its Manglish output.
- **Repository:** https://github.com/CainWasSlain/Scribe-Rust (branch `main`).

## Known limits (accepted by the owner)
- Everyone on the other side of the call is labelled "Others"; individual remote speakers aren't told apart.
- Decisions and action items are only found from English cue phrases.
- Manglish spells words as they sound, with doubled long vowels ("njaan").
- The summary quotes sentences as they were said rather than rewriting them.
