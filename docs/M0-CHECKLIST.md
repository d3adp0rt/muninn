# M0 device checklist

Run after `./scripts/build-apk.sh` and `adb install -r android/app/build/outputs/apk/debug/app-debug.apk`.

## Shell

- [ ] Cold start under 1.5 s (mid-range device)
- [ ] No crash on rotation / Activity recreation
- [ ] Back gesture does not kill the app unexpectedly
- [ ] Content is not hidden behind status/navigation bars (M0: fitsSystemWindows)
- [ ] 60 fps while scrolling a 10k-message list
- [ ] Fling feels native (no stutter at the end)

## IME (blocking gate)

- [ ] Keyboard opens when the composer is tapped
- [ ] Cyrillic input: `Привет, мир`
- [ ] Emoji input: `🐦‍⬛`
- [ ] Cursor in the middle: insert and delete around it
- [ ] Selection + copy/paste from another app
- [ ] Shift+Enter inserts a newline; Enter sends
- [ ] Keyboard does not cover the composer
- [ ] Gboard, Samsung Keyboard, and one third-party IME
- [ ] Composing text (predictive suggestions) does not duplicate characters

## Rendering

- [ ] Text is crisp at 1x/2x/3x density
- [ ] Dark theme background is uniform (no banding)
- [ ] Long words and URLs wrap instead of overflowing
- [ ] Emoji render in colour
- [ ] No Vulkan validation errors in `adb logcat`

## Fallbacks

- [ ] Force GLES (`WGPU_BACKEND=gl`) and repeat the rendering checks
- [ ] Low-end device / Android 8: app still starts

## Size

- [ ] `unzip -l app-debug.apk` total under the M0 budget
- [ ] arm64 `libmuninn.so` size recorded
