//! Encrypted-at-rest storage for the API key and base URL.
//!
//! The plaintext is sealed by a non-exportable AES-GCM key in Android Keystore.
//! Losing the Keystore key (factory reset, uninstall) means the secret is gone;
//! there is deliberately no recovery path.

use std::path::PathBuf;

pub struct Secrets {
    pub base_url: String,
    pub api_key: String,
}

fn path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let Some(app) = crate::ANDROID_APP.get() else {
            log::warn!("secrets: app handle missing");
            return None;
        };
        let Some(dir) = app.internal_data_path() else {
            log::warn!("secrets: internal data path missing");
            return None;
        };
        Some(dir.join("secrets.bin"))
    }

    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

pub fn save(base_url: &str, api_key: &str) -> bool {
    let Some(path) = path() else {
        log::warn!("secrets: no path");
        return false;
    };

    let plaintext = format!("{base_url}\n{api_key}");
    let Some(blob) = crate::platform::keystore_seal(plaintext.as_bytes()) else {
        log::warn!("secrets: keystore seal failed");
        return false;
    };

    match std::fs::write(&path, &blob) {
        Ok(()) => {
            log::debug!("secrets: saved {} bytes to {}", blob.len(), path.display());
            true
        }
        Err(err) => {
            log::warn!("secrets: write failed: {err}");
            false
        }
    }
}

pub fn load() -> Option<Secrets> {
    let path = path()?;
    let blob = match std::fs::read(&path) {
        Ok(blob) => blob,
        Err(err) => {
            log::debug!("secrets: none at {} ({err})", path.display());
            return None;
        }
    };
    let plaintext = match crate::platform::keystore_open(&blob) {
        Some(plaintext) => plaintext,
        None => {
            log::warn!("secrets: keystore open failed ({} bytes)", blob.len());
            return None;
        }
    };
    let text = String::from_utf8(plaintext).ok()?;
    let (base_url, api_key) = text.split_once('\n')?;

    if api_key.is_empty() {
        return None;
    }

    log::debug!("secrets: loaded key for {base_url}");
    Some(Secrets {
        base_url: base_url.to_owned(),
        api_key: api_key.to_owned(),
    })
}

pub fn clear() {
    if let Some(path) = path() {
        let _ = std::fs::remove_file(path);
    }
}
