# Muninn

A private, local-first OpenRouter client for Android, written in Rust.

> M0/M1 in progress: egui + GameActivity + wgpu, native Android text input,
> encrypted key and chat history, streaming markdown chat, personas with
> layered system prompts, branched conversations with edits and answer
> variants, thread drawer, and Dark/AMOLED/Light themes with a tokenized
> design system.

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

## Try it without an API key

A local SSE mock exercises the whole path (models → stream → reasoning → usage):

```bash
python3 scripts/mock-server.py 8765
adb reverse tcp:8765 tcp:8765
# in Muninn: base URL http://127.0.0.1:8765/v1, any key
```

The base URL `127.0.0.1` is allowed cleartext by
`res/xml/network_security_config.xml`; everything else requires TLS.

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

## Releases

Signed APKs are published on the
[releases page](https://github.com/d3adp0rt/muninn/releases). Only
`arm64-v8a` is built; Android 8.0 (API 26) or newer is required.

CI status: [![CI](https://github.com/d3adp0rt/muninn/actions/workflows/ci.yml/badge.svg)](https://github.com/d3adp0rt/muninn/actions/workflows/ci.yml)

Release signing, secrets and the tag procedure are documented in
[docs/RELEASE.md](docs/RELEASE.md).
