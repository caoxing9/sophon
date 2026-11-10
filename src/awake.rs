//! Keeps this Mac awake while its agents work, as `caffeinate` does: idle
//! sleep held off, and the display too when asked. The hold is a
//! `caffeinate` child that waits on Sophon, so a crash lets it go.

use std::{process::Child, sync::Mutex};

/// The `caffeinate` holding the Mac, and whether it holds the display.
static HOLD: Mutex<Option<(bool, Child)>> = Mutex::new(None);

/// Holds the Mac awake (`Some(display)`) or lets it sleep (`None`).
pub(crate) fn hold(want: Option<bool>) {
    let Ok(mut hold) = HOLD.lock() else { return };
    if hold.as_ref().map(|(display, _)| *display) == want {
        return;
    }
    if let Some((_, mut child)) = hold.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let Some(display) = want else { return };
    let mut cmd = std::process::Command::new("/usr/bin/caffeinate");
    cmd.arg(if display { "-di" } else { "-i" }).args(["-w", &std::process::id().to_string()]);
    match cmd.spawn() {
        Ok(child) => *hold = Some((display, child)),
        Err(e) => log::warn!("cannot keep the Mac awake: {e}"),
    }
}

/// Takes the hold, sees macOS count it, and lets it go.
#[cfg(test)]
#[test]
#[ignore]
fn hold_live() {
    let mine = || {
        let out = std::process::Command::new("/bin/ps").args(["-axo", "ppid,command"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| l.trim_start().starts_with(&std::process::id().to_string()) && l.contains("caffeinate"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    hold(Some(false));
    std::thread::sleep(std::time::Duration::from_millis(300));
    eprintln!("on: {:?}", mine());
    assert_eq!(mine().len(), 1);
    hold(Some(true));
    eprintln!("display: {:?}", mine());
    assert!(mine()[0].contains("-di"));
    hold(None);
    assert!(mine().is_empty());
}
