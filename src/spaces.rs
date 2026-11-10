//! Spaces on this Mac: one Paseo service per person, each with its own
//! projects, conversations, agents and schedules (`~/.paseo` is the first;
//! the others live in `~/.paseo-spaces/<slug>`). A space is a saved host
//! whose address is `space:<its folder>`, so switching spaces is switching
//! hosts. The services all run here as this Mac's user: a space keeps
//! people's work apart, it does not wall it off.

use std::path::{Path, PathBuf};

/// A saved host's address for the space in `home`.
// Spaces are made on a Mac only.
#[cfg_attr(any(target_os = "ios", target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn url_of(home: &Path) -> String {
    format!("space:{}", home.display())
}

/// The folder of a `space:` address.
pub(crate) fn home_of(url: &str) -> Option<PathBuf> {
    url.strip_prefix("space:").map(PathBuf::from)
}

#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
mod imp {
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    fn home() -> PathBuf {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
    }

    fn root() -> PathBuf {
        home().join(".paseo-spaces")
    }

    /// `name` as a folder name: its ASCII letters and digits, else a fresh id.
    fn slug(name: &str) -> String {
        let s: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if s.is_empty() { format!("space-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]) } else { s }
    }

    /// The port a service in `home` is set to listen on.
    pub(crate) fn port_of(home: &Path) -> Option<u16> {
        let text = std::fs::read_to_string(home.join("config.json")).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        v.pointer("/daemon/listen")?.as_str()?.rsplit(':').next()?.parse().ok()
    }

    /// The first port from 6768 no space has and nothing listens on.
    fn free_port() -> Option<u16> {
        let taken: Vec<u16> = std::fs::read_dir(root())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| port_of(&e.path()))
            .collect();
        (6768..6868).find(|p| !taken.contains(p) && std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
    }

    fn paseo(args: &[&str]) -> Result<String, String> {
        let cli = crate::store::service_cli().ok_or("The service is not installed on this Mac")?;
        let out = std::process::Command::new(cli).args(args).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("failed").to_owned())
        }
    }

    /// Makes a space called `name` and starts its service (blocking): the
    /// first space's settings (agents, relay) with a port of its own. Its
    /// folder.
    pub(crate) fn create(name: &str) -> Result<PathBuf, String> {
        let mut home = root().join(slug(name));
        while home.exists() {
            home = root().join(format!("{}-{}", slug(name), &uuid::Uuid::new_v4().simple().to_string()[..4]));
        }
        let port = free_port().ok_or("No free port for another space")?;
        let main = std::fs::read_to_string(home_of_first().join("config.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let mut config = main.unwrap_or_else(|| serde_json::json!({}));
        config["daemon"]["listen"] = format!("127.0.0.1:{port}").into();
        std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        std::fs::write(home.join("config.json"), serde_json::to_string_pretty(&config).unwrap_or_default()).map_err(|e| e.to_string())?;
        start(&home)?;
        Ok(home)
    }

    /// Whether the service of the space in `home` runs (Paseo keeps its pid
    /// file while it does).
    pub(crate) fn running(home: &Path) -> bool {
        home.join("paseo.pid").exists()
    }

    /// The first space's folder, Paseo's own.
    pub(crate) fn home_of_first() -> PathBuf {
        std::env::var_os("PASEO_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".paseo"))
    }

    /// Starts the service of the space in `home` (blocking); one running
    /// already is left as it is.
    pub(crate) fn start(home: &Path) -> Result<(), String> {
        let home = home.to_string_lossy();
        paseo(&["daemon", "start", "--home", &home, "--timeout", "60"]).map(|_| ())
    }

    /// Stops the service of the space in `home` (blocking). Its folder,
    /// with its conversations, stays.
    pub(crate) fn stop(home: &Path) -> Result<(), String> {
        let home = home.to_string_lossy();
        paseo(&["daemon", "stop", "--home", &home]).map(|_| ())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A space made, started and found as Sophon finds it, in a home of
        /// its own (`SOPHON_TEST_HOME`, holding a copy of a Paseo config):
        /// never the user's.
        #[test]
        #[ignore]
        fn create_start_find_and_stop_live() {
            let Some(home) = std::env::var_os("SOPHON_TEST_HOME") else { return };
            // SAFETY: set before any thread of this test reads it.
            unsafe { std::env::set_var("HOME", &home) };
            let space = create("Test Space").expect("created");
            assert!(space.starts_with(PathBuf::from(&home).join(".paseo-spaces")));
            let port = port_of(&space).expect("a port");
            assert!(port >= 6768);
            let endpoint = crate::daemon::connection::discover_in(&space);
            eprintln!("space at {}: {} (credential: {})", space.display(), endpoint.url, endpoint.local_credential.is_some());
            assert_eq!(endpoint.url, format!("ws://127.0.0.1:{port}/ws"));
            assert!(endpoint.local_credential.is_some(), "the running service's credential");
            stop(&space).expect("stopped");
        }

        #[test]
        fn names_become_folder_names() {
            assert_eq!(slug("Lin's Space"), "lin-s-space");
            assert!(slug("林").starts_with("space-"));
        }
    }
}

#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) use imp::*;

// A phone or a page has no spaces of its own: the settings that list them
// are a Mac's.
#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
mod none {
    use std::path::{Path, PathBuf};

    pub(crate) fn home_of_first() -> PathBuf {
        PathBuf::new()
    }

    pub(crate) fn port_of(_home: &Path) -> Option<u16> {
        None
    }

    pub(crate) fn running(_home: &Path) -> bool {
        false
    }
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
pub(crate) use none::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_address_names_its_folder() {
        let home = Path::new("/Users/me/.paseo-spaces/lin");
        assert_eq!(home_of(&url_of(home)).as_deref(), Some(home));
        assert_eq!(home_of("ws://1.2.3.4:6767/ws"), None);
    }
}
