# Muninn

A private, local-first OpenRouter client for Android, written in Rust.

> M0: the Android shell is being wired up (egui + GameActivity + wgpu).
> Chat, keys, history and the agent land milestone by milestone.

## What it will be

- Chat with any OpenRouter model, or any OpenAI-compatible endpoint
- Conversation **tree** with branches, variants, edits and regeneration
- Personas as behaviour presets (not characters), layered system prompts
- Reasoning on tap, web search, images, PDFs, voice input
- Local-only history, encrypted with SQLCipher, system app-lock
- Agent workspace: files, shell, Python (CPython), approvals, diffs, undo
- No accounts, no telemetry, no logs, no monetisation

## Stack

| Layer | Choice |
|---|---|
| UI | `egui` / `eframe` 0.37, `wgpu` (Vulkan, GLES fallback) |
| Android glue | `android-activity` + GameActivity (IME) |
| Core | Rust 2024, `tokio`, `reqwest`, `rusqlite`/SQLCipher |
| Build | `cargo-ndk` + Gradle, arm64-v8a |

## Requirements

- Rust stable
- Android SDK (platform 36, build-tools 36)
- Android NDK 27.2.12479018
- JDK 17
- `cargo-ndk`

## Build

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"

rustup target add aarch64-linux-android
cargo install cargo-ndk

./scripts/build-apk.sh
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

Desktop preview (not shipped):

```bash
cargo run --features desktop --bin muninn-desktop
```

## Layout

```
src/            Rust: UI shell, theme, Android entrypoint
android/        Gradle project + the Java/Kotlin glue (MainActivity)
scripts/        build helpers
.github/        CI
```

## Licence

GPL-3.0-or-later.
