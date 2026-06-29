//! Launches whatever the user mapped to a Small Deck key — an application, a
//! file, a folder, or a URL. Mirrors Stream Deck's "Open" action: we do not
//! maintain any allow-list, we just hand the target to the OS shell, and the OS
//! decides whether it can open it.

use std::process::Command;

/// Open `target` using the platform's default mechanism. Returns once the
/// launch has been *requested* (it does not wait for the program to exit).
pub fn launch(target: &str) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("No program/target set for this key.".into());
    }

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(target).spawn();

    // `cmd /C start "" <target>` lets Windows resolve .exe, .lnk shortcuts,
    // documents and URLs the same way Explorer would. The empty "" is the
    // (ignored) window title that `start` requires as its first quoted arg.
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args(["/C", "start", "", target])
        .spawn();

    #[cfg(target_os = "linux")]
    let result = Command::new("xdg-open").arg(target).spawn();

    result
        .map(|_| ())
        .map_err(|e| format!("Failed to launch '{target}': {e}"))
}

/// Run a shell command line. The user types it in a text box; complex logic is
/// supported by having them call a script (e.g. `bash ~/foo.sh`). Runs with the
/// user's own privileges via the platform shell.
pub fn run_command(command: &str) -> Result<(), String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("No command set for this key.".into());
    }

    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", command]).spawn();

    #[cfg(not(target_os = "windows"))]
    let result = Command::new("sh").arg("-c").arg(command).spawn();

    result
        .map(|_| ())
        .map_err(|e| format!("Failed to run command: {e}"))
}

/// Dispatch a binding by its kind: `command` runs in the shell; everything else
/// (app / folder / url) is handed to the OS open mechanism.
pub fn run_entry(kind: &str, target: &str) -> Result<(), String> {
    if kind == "command" {
        run_command(target)
    } else {
        launch(target)
    }
}
