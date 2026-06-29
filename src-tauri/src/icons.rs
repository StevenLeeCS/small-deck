//! Best-effort extraction of an application's icon so the UI can show it next to
//! the key it's bound to. Returns a PNG `data:` URL (works directly in <img>).
//!
//! macOS is implemented (read the .app's .icns and convert with `sips`). Other
//! platforms return None for now — the UI just shows no icon, which is fine.

#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use std::process::Command;

pub fn app_icon_data_url(path: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos_icon_data_url(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_icon_data_url(app_path: &str) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let icns = find_icns(app_path)?;

    // Cache the rendered PNG in the temp dir, keyed by the .icns path.
    let mut h = DefaultHasher::new();
    icns.hash(&mut h);
    let png = std::env::temp_dir().join(format!("xpad-icon-{:x}.png", h.finish()));

    if !png.exists() {
        let ok = Command::new("sips")
            .args(["-s", "format", "png", "-z", "64", "64", &icns, "--out"])
            .arg(&png)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return None;
        }
    }

    let bytes = std::fs::read(&png).ok()?;
    Some(format!("data:image/png;base64,{}", base64(&bytes)))
}

/// Resolve the .icns inside a .app bundle (or None if `app_path` isn't a bundle).
#[cfg(target_os = "macos")]
fn find_icns(app_path: &str) -> Option<String> {
    let p = Path::new(app_path);
    if p.extension().and_then(|e| e.to_str()) != Some("app") {
        return None; // only .app bundles carry an .icns we can read
    }
    let resources = p.join("Contents/Resources");

    // Preferred: the bundle's declared icon file (CFBundleIconFile).
    let info = p.join("Contents/Info.plist");
    if let Ok(out) = Command::new("plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&info)
        .output()
    {
        if out.status.success() {
            let mut name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !name.is_empty() {
                if !name.ends_with(".icns") {
                    name.push_str(".icns");
                }
                let cand = resources.join(&name);
                if cand.exists() {
                    return Some(cand.to_string_lossy().into_owned());
                }
            }
        }
    }

    // Fallbacks: a conventionally-named icon, then any .icns in Resources.
    let app_icon = resources.join("AppIcon.icns");
    if app_icon.exists() {
        return Some(app_icon.to_string_lossy().into_owned());
    }
    if let Ok(rd) = std::fs::read_dir(&resources) {
        for entry in rd.flatten() {
            let pth = entry.path();
            if pth.extension().and_then(|x| x.to_str()) == Some("icns") {
                return Some(pth.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Minimal standard base64 encoder (avoids pulling in an extra crate).
#[cfg(target_os = "macos")]
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
