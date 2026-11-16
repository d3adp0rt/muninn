# Roadmap

## M0 — platform proof (current)

- [x] Repository + licence + CI skeleton
- [x] Toolchain: SDK 36, NDK 27.2, cargo-ndk, arm64 target
- [x] egui + eframe + GameActivity renders on device
- [x] Native `EditText` overlay for all text input: caret, selection,
      long-press delete, emoji, clipboard, autocorrect
- [x] Insets, keyboard-aware layout, hardware back
- [ ] 10k-message list at 60 fps with fling
- [x] SSE streaming, reasoning, usage/cost, Stop
- [x] Markdown renderer (paragraphs, headings, lists, quotes, code, tables)
- [ ] Math (LaTeX), syntax highlighting
- [ ] SQLCipher + FTS5 open/query after biometric unlock
- [ ] SAF picker, microphone → WAV, STT round-trip
- [ ] CPython cross-compile spike (`libpyrunner.so` + stdlib zip + pip wheel)
- [x] APK size measured (~11 MB debug, arm64)

## M1 — chat MVP

Onboarding (key, base URL, balance, free-model default), chats/tree, streaming,
markdown/code/math, reasoning-on-tap, edit/branch/regenerate, per-chat cost,
personas + prompt layers, model picker with categories, provider preferences,
themes, RU/EN, SQLCipher, app-lock, trash/undo, export.

Done: onboarding, streaming + markdown + code copy, reasoning-on-tap,
per-chat cost, per-chat model, personas with three prompt layers, chat
rename/delete/regenerate/retry, themes, model picker. Remaining: branching,
edits as tree children, SQLCipher, app-lock, export, RU/EN toggle.

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
