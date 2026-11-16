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
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

    use android_activity::input::{TextInputState, TextSpan};
    use jni::objects::GlobalRef;

    static ACTIVITY: OnceLock<GlobalRef> = OnceLock::new();

    pub fn init_activity(app: &winit::platform::android::activity::AndroidApp) {
        use jni::{JavaVM, objects::JObject};

        let ctx = ndk_context::android_context();
        let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else {
            return;
        };
        let Ok(env) = vm.attach_current_thread() else {
            return;
        };
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        if let Ok(global) = env.new_global_ref(activity) {
            let _ = ACTIVITY.set(global);
        }
    }

    pub(super) fn activity() -> Option<&'static GlobalRef> {
        ACTIVITY.get()
    }

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
pub use android::{ime_text, init_activity, set_ime_text};

#[cfg(not(target_os = "android"))]
pub fn ime_text() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
pub fn set_ime_text(_text: &str, _cursor_bytes: usize) {}

#[cfg(target_os = "android")]
pub fn clipboard_get() -> Option<String> {
    use jni::JavaVM;

    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = android::activity()?.as_obj();

    let value = env
        .call_method(activity, "muninnClipboardGet", "()Ljava/lang/String;", &[])
        .ok()?;
    let obj = value.l().ok()?;
    if obj.is_null() {
        return None;
    }
    let js = jni::objects::JString::from(obj);
    let text = env.get_string(&js).ok()?;
    Some(text.into())
}

#[cfg(target_os = "android")]
pub fn clipboard_set(text: &str) {
    use jni::{JavaVM, objects::JObject};

    let ctx = ndk_context::android_context();
    let Ok(vm) = (unsafe { JavaVM::from_raw(ctx.vm().cast()) }) else {
        return;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return;
    };
    let Some(activity) = android::activity() else {
        return;
    };
    let Ok(string) = env.new_string(text) else {
        return;
    };
    let obj: JObject = string.into();
    let _ = env.call_method(
        activity.as_obj(),
        "muninnClipboardSet",
        "(Ljava/lang/String;)V",
        &[jni::objects::JValue::Object(&obj)],
    );
}

#[cfg(not(target_os = "android"))]
pub fn clipboard_get() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub fn clipboard_set(_text: &str) {}

#[cfg(target_os = "android")]
pub fn keystore_seal(plaintext: &[u8]) -> Option<Vec<u8>> {
    use jni::{
        JavaVM,
        objects::{JByteArray, JObject, JValue},
    };

    let ctx = ndk_context::android_context();
    let vm = match unsafe { JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(err) => {
            log::warn!("keystore: vm {err:?}");
            return None;
        }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(err) => {
            log::warn!("keystore: attach {err:?}");
            return None;
        }
    };
    let Some(activity) = android::activity() else {
        log::warn!("keystore: no activity");
        return None;
    };

    let array = match env.byte_array_from_slice(plaintext) {
        Ok(array) => array,
        Err(err) => {
            log::warn!("keystore: array {err:?}");
            return None;
        }
    };
    let obj: JObject = array.into();
    let value = match env.call_method(activity, "muninnSeal", "([B)[B", &[JValue::Object(&obj)]) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("keystore: call {err:?}");
            return None;
        }
    };
    let result = match value.l() {
        Ok(result) => result,
        Err(err) => {
            log::warn!("keystore: result {err:?}");
            return None;
        }
    };
    if result.is_null() {
        log::warn!("keystore: null ciphertext");
        return None;
    }
    match env.convert_byte_array(JByteArray::from(result)) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            log::warn!("keystore: bytes {err:?}");
            None
        }
    }
}

#[cfg(target_os = "android")]
pub fn keystore_open(blob: &[u8]) -> Option<Vec<u8>> {
    use jni::{
        JavaVM,
        objects::{JByteArray, JValue},
    };

    let ctx = ndk_context::android_context();
    let vm = match unsafe { JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(err) => {
            log::warn!("keystore: open vm {err:?}");
            return None;
        }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(err) => {
            log::warn!("keystore: open attach {err:?}");
            return None;
        }
    };
    let Some(activity) = android::activity() else {
        log::warn!("keystore: open no activity");
        return None;
    };

    let array = match env.byte_array_from_slice(blob) {
        Ok(array) => array,
        Err(err) => {
            log::warn!("keystore: open array {err:?}");
            return None;
        }
    };
    let obj: jni::objects::JObject = array.into();
    let value = match env.call_method(
        activity.as_obj(),
        "muninnOpen",
        "([B)[B",
        &[JValue::Object(&obj)],
    ) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("keystore: open call {err:?}");
            return None;
        }
    };
    let result = match value.l() {
        Ok(result) => result,
        Err(err) => {
            log::warn!("keystore: open result {err:?}");
            return None;
        }
    };
    if result.is_null() {
        log::warn!("keystore: open null plaintext");
        return None;
    }
    match env.convert_byte_array(JByteArray::from(result)) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            log::warn!("keystore: open bytes {err:?}");
            None
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn keystore_seal(_plaintext: &[u8]) -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_os = "android"))]
pub fn keystore_open(_blob: &[u8]) -> Option<Vec<u8>> {
    None
}
