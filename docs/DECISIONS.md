# Decisions

Locked decisions for Muninn. Change only with a written note here.

## Platform

- Android only. `minSdk 26`, `targetSdk 36`, release ABI `arm64-v8a`.
- UI: egui 0.37 + eframe + wgpu (Vulkan, GLES fallback). No CPU renderer.
- Android glue: GameActivity (IME) + a small Java `MainActivity`. No other
  Java/Kotlin in the app.
- No desktop app; a `desktop` feature exists only for local UI preview.

## API

- Chat Completions is the primary API. Responses API is used only where a
  server tool requires it (hosted shell, subagent/advisor).
- Any OpenAI-compatible base URL is supported; OpenRouter is the default.
- Multiple keys, one account, key pasted manually. No OAuth.
- Default model is a free one; `openrouter/auto` and fallback chains supported.
- Reasoning is buffered and rendered when the user expands it.
- Web search uses the `web` plugin. PDFs use `file-parser` + `cloudflare-ai`,
  no OCR.
- Speech input uses `/audio/transcriptions`. No TTS.
- Inspector (raw request/SSE) is debug-only.

## Data

- Single user, local only, no sync, no telemetry, no logs.
- Everything encrypted: SQLCipher for the database, AES-256-GCM for files.
  Key wrapped by Android Keystore. System biometric/device-credential lock.
- No recovery if the Keystore key is lost; encrypted export is the backup path.
- Conversation tree with branches; edits and regenerations create children.
- Raw request/response JSON is stored for the inspector and reproducibility.
- Embeddings are used only for semantic chat search; FTS5 remains the base.
- `FLAG_SECURE` on by default; temporary opt-out in settings.
- `allowBackup=false`.

## Python

- Embedded CPython 3.13/3.14, static build, shipped as `libpyrunner.so` in
  `jniLibs` and executed from `nativeLibraryDir` (W^X-safe).
- stdlib as a zip; pip bootstrapped from a bundled wheel; wheels must be
  `py3-none-any` (no C extensions).
- C-extension packages and full Linux userland run in the OpenRouter cloud
  shell, not locally.

## Agent

- Approvals: reads auto, writes ask with diff, exec ask with command preview,
  destructive always ask. "Always allow" is scoped to a chat.
- Workspace: app-private by default; SAF folders only after an explicit grant
  and warning.
- Shell: `/system/bin/sh` + toybox locally; Termux is not used.
- Cloud shell: `openrouter:shell` on Responses API.
- No user-facing budget limits; hard technical caps (time, output, steps).
- Optional Accessibility-driven UI control is a separate, disabled-by-default
  module (v2). Device actions go through Intents with confirmation.
- Full audit log of tool calls, visible to the user.

## Product

- Zero monetisation, zero telemetry, zero censorship in prompts.
- User owns the system prompt end to end; the app only adds visible structure
  (date, tool schemas, memory) and shows it in a preview.
- Themes: dark, AMOLED, light, deep customisation; Material 3 language.
- Licence: GPL-3.0-or-later.
