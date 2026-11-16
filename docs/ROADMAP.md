# Roadmap

## M0 — platform proof (current)

- [x] Repository + licence + CI skeleton
- [ ] Toolchain: SDK 36, NDK 27.2, cargo-ndk, arm64 target
- [ ] egui + eframe + GameActivity renders on device
- [ ] IME: Cyrillic, emoji, cursor mid-string, selection, paste, insets, back
- [ ] 10k-message list at 60 fps with fling
- [ ] SSE stream at 100+ tok/s with incremental markdown
- [ ] SQLCipher + FTS5 open/query after biometric unlock
- [ ] SAF picker, microphone → WAV, STT round-trip
- [ ] CPython cross-compile spike (`libpyrunner.so` + stdlib zip + pip wheel)
- [ ] Measure APK size and cold start

## M1 — chat MVP

Onboarding (key, base URL, balance, free-model default), chats/tree, streaming,
markdown/code/math, reasoning-on-tap, edit/branch/regenerate, per-chat cost,
personas + prompt layers, model picker with categories, provider preferences,
themes, RU/EN, SQLCipher, app-lock, trash/undo, export.

## M2 — multimodal + search

Images/camera/HEIC, PDF (`cloudflare-ai`), text files, STT, web plugin and
citations, FTS + embeddings search, folders/tags, auto-title, compaction,
memory, mermaid (WebView snapshot), full markdown, command palette, stats,
incognito/retention, encrypted backup.

## M3 — agent

Tool loop with approvals, diff/undo/audit, filesystem + SAF roots, embedded
CPython and packages UI, system shell, cloud shell, device intents, foreground
runs, subagent/advisor.

## M4 — beta

Performance, memory, offline edges, F-Droid metadata, screenshots, v1.0.
