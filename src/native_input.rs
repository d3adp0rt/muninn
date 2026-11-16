//! Native Android text field used for the composer.
//!
//! Text editing is delegated to a real `EditText`: the IME owns the caret,
//! selection, backspace repeat and clipboard. Rust receives full text updates
//! through JNI and renders the surrounding chat UI.

use std::sync::Mutex;

#[cfg(target_os = "android")]
use crate::theme;
#[cfg(target_os = "android")]
use egui::Color32;
use egui::Rect;

pub const TARGET_COMPOSER: i32 = 0;
pub const TARGET_MODEL_SEARCH: i32 = 1;
pub const TARGET_CHAT_SEARCH: i32 = 2;
pub const TARGET_BASE_URL: i32 = 3;
pub const TARGET_API_KEY: i32 = 4;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Event {
    Text { target: i32, text: String },
    Selection { target: i32, start: i32, end: i32 },
    Focus { target: i32, focused: bool },
    Action { target: i32, action: i32 },
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());

#[cfg(target_os = "android")]
fn push(event: Event) {
    if let Ok(mut queue) = EVENTS.lock() {
        queue.push(event);
    }
}

pub fn poll() -> Vec<Event> {
    EVENTS
        .lock()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default()
}

#[cfg(target_os = "android")]
pub fn show(
    target: i32,
    rect: Rect,
    text: &str,
    hint: &str,
    multiline: bool,
    password: bool,
    ppp: f32,
) -> bool {
    use jni::objects::{JObject, JValue};

    let palette = theme::c();
    let x = (rect.min.x * ppp).round() as i32;
    let y = (rect.min.y * ppp).round() as i32;
    let w = (rect.width() * ppp).round().max(1.0) as i32;
    let h = (rect.height() * ppp).round().max(1.0) as i32;

    let geometry = format!(
        "{x}|{y}|{w}|{h}|-1|-1|{}|{}|0|15.0|{}|{}",
        argb(palette.text),
        argb(palette.text_tertiary),
        multiline as u8,
        password as u8,
    );

    let Some(result) = crate::platform::with_activity(|env, activity| -> bool {
        let Ok(geometry) = env.new_string(geometry) else {
            return false;
        };
        let Ok(text) = env.new_string(text) else {
            return false;
        };
        let Ok(hint) = env.new_string(hint) else {
            return false;
        };
        let geometry: JObject = geometry.into();
        let text: JObject = text.into();
        let hint: JObject = hint.into();

        env.call_method(
            activity,
            "muninnInputShow",
            "(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Int(target),
                JValue::Object(&geometry),
                JValue::Object(&text),
                JValue::Object(&hint),
            ],
        )
        .is_ok()
    }) else {
        return false;
    };

    result
}

#[cfg(not(target_os = "android"))]
pub fn show(
    _target: i32,
    _rect: Rect,
    _text: &str,
    _hint: &str,
    _multiline: bool,
    _password: bool,
    _ppp: f32,
) -> bool {
    false
}

#[cfg(target_os = "android")]
pub fn set_rect(rect: Rect, ppp: f32) {
    use jni::objects::JValue;

    let x = (rect.min.x * ppp).round() as i32;
    let y = (rect.min.y * ppp).round() as i32;
    let w = (rect.width() * ppp).round().max(1.0) as i32;
    let h = (rect.height() * ppp).round().max(1.0) as i32;

    let _ = crate::platform::with_activity(|env, activity| {
        let _ = env.call_method(
            activity,
            "muninnInputSetRect",
            "(IIII)V",
            &[
                JValue::Int(x),
                JValue::Int(y),
                JValue::Int(w),
                JValue::Int(h),
            ],
        );
    });
}

#[cfg(not(target_os = "android"))]
pub fn set_rect(_rect: Rect, _ppp: f32) {}

#[cfg(target_os = "android")]
#[allow(dead_code)]
pub fn set_text(text: &str) {
    use jni::objects::{JObject, JValue};

    let _ = crate::platform::with_activity(|env, activity| {
        let Ok(value) = env.new_string(text) else {
            return;
        };
        let value: JObject = value.into();
        let _ = env.call_method(
            activity,
            "muninnInputSetText",
            "(Ljava/lang/String;II)V",
            &[JValue::Object(&value), JValue::Int(-1), JValue::Int(-1)],
        );
    });
}

#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
pub fn set_text(_text: &str) {}

#[cfg(target_os = "android")]
pub fn hide() {
    let _ = crate::platform::with_activity(|env, activity| {
        let _ = env.call_method(activity, "muninnInputHide", "()V", &[]);
    });
}

#[cfg(not(target_os = "android"))]
pub fn hide() {}

#[cfg(target_os = "android")]
fn argb(color: Color32) -> i32 {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    ((a as i32) << 24) | ((r as i32) << 16) | ((g as i32) << 8) | (b as i32)
}

#[cfg(target_os = "android")]
mod callbacks {
    use jni::JNIEnv;
    use jni::objects::{JObject, JString};
    use jni::sys::{jboolean, jint};

    use super::{Event, push};

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_d3adp0rt_muninn_MainActivity_nativeInputText(
        mut env: JNIEnv<'_>,
        _this: JObject<'_>,
        target: jint,
        text: JString<'_>,
    ) {
        if let Ok(text) = env.get_string(&text) {
            push(Event::Text {
                target,
                text: text.into(),
            });
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_d3adp0rt_muninn_MainActivity_nativeInputSelection(
        _env: JNIEnv<'_>,
        _this: JObject<'_>,
        target: jint,
        start: jint,
        end: jint,
    ) {
        push(Event::Selection { target, start, end });
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_d3adp0rt_muninn_MainActivity_nativeInputFocus(
        _env: JNIEnv<'_>,
        _this: JObject<'_>,
        target: jint,
        focused: jboolean,
    ) {
        push(Event::Focus {
            target,
            focused: focused != 0,
        });
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_d3adp0rt_muninn_MainActivity_nativeInputAction(
        _env: JNIEnv<'_>,
        _this: JObject<'_>,
        target: jint,
        action: jint,
    ) {
        push(Event::Action { target, action });
    }
}
