//! Muninn — a private, local-first OpenRouter client for Android.
//!
//! The crate builds as a `cdylib` for Android and as a normal library/binary
//! for local development.

mod app;
mod theme;

pub use app::MuninnApp;

#[cfg(target_os = "android")]
mod android_entry {
    use winit::platform::android::activity::AndroidApp;

    /// Entry point used by `android-activity` (GameActivity backend).
    ///
    /// This may be called again if the Activity is recreated, so global
    /// initialization must be idempotent.
    #[unsafe(no_mangle)]
    pub fn android_main(app: AndroidApp) {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("muninn"),
        );

        log::info!("Muninn starting");

        let options = eframe::NativeOptions {
            android_app: Some(app),
            ..Default::default()
        };

        if let Err(err) = eframe::run_native(
            "Muninn",
            options,
            Box::new(|cc| Ok(Box::new(crate::MuninnApp::new(cc)))),
        ) {
            log::error!("eframe failed: {err}");
        }
    }
}
