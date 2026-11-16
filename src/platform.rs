//! Android-specific platform queries that eframe does not expose.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

pub fn system_insets(ui: &egui::Ui) -> Insets {
    #[cfg(target_os = "android")]
    {
        use std::sync::atomic::Ordering::Relaxed;
        let ppp = ui.ctx().pixels_per_point().max(0.5);
        let raw = Insets {
            top: android::TOP.load(Relaxed) as f32 / ppp,
            bottom: android::BOTTOM.load(Relaxed) as f32 / ppp,
            left: android::LEFT.load(Relaxed) as f32 / ppp,
            right: android::RIGHT.load(Relaxed) as f32 / ppp,
        };
        Insets {
            top: raw.top.clamp(0.0, 200.0),
            bottom: raw.bottom.clamp(0.0, 320.0),
            left: raw.left.clamp(0.0, 200.0),
            right: raw.right.clamp(0.0, 200.0),
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = ui;
        Insets::default()
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

    use android_activity::input::{TextInputState, TextSpan};

    pub(super) static TOP: AtomicI32 = AtomicI32::new(0);
    pub(super) static BOTTOM: AtomicI32 = AtomicI32::new(0);
    pub(super) static LEFT: AtomicI32 = AtomicI32::new(0);
    pub(super) static RIGHT: AtomicI32 = AtomicI32::new(0);

    /// Called from `MainActivity.onApplyWindowInsets`.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_d3adp0rt_muninn_MainActivity_nativeInsets(
        _env: *mut core::ffi::c_void,
        _this: *mut core::ffi::c_void,
        top: i32,
        bottom: i32,
        left: i32,
        right: i32,
    ) {
        TOP.store(top, Relaxed);
        BOTTOM.store(bottom, Relaxed);
        LEFT.store(left, Relaxed);
        RIGHT.store(right, Relaxed);
    }

    /// Current text state as seen by the Android IME.
    pub fn ime_text() -> Option<String> {
        crate::ANDROID_APP
            .get()
            .map(|app| app.text_input_state().text)
    }

    /// Push text/selection from the Rust UI into the IME.
    pub fn set_ime_text(text: &str, cursor_bytes: usize) {
        let Some(app) = crate::ANDROID_APP.get() else {
            return;
        };
        let cursor = cursor_bytes.min(text.len());
        let state = TextInputState {
            text: text.to_owned(),
            selection: TextSpan {
                start: cursor,
                end: cursor,
            },
            ..Default::default()
        };
        app.set_text_input_state(state);
    }
}

#[cfg(target_os = "android")]
pub use android::{ime_text, set_ime_text};

#[cfg(not(target_os = "android"))]
pub fn ime_text() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
pub fn set_ime_text(_text: &str, _cursor_bytes: usize) {}
