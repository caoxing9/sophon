//! The app's side of push notifications (`docs/push.md`): who wants them,
//! kept in `push.json` in the app's data folder as phones register over
//! the bridge, and each space's share of it written for that space's
//! service, which sends them whether the app is open or not
//! (`sophon_push::ServiceFile`). A phone delivers its actions from here
//! ([`deliver`]).

use std::{collections::HashSet, path::PathBuf, sync::Mutex};

pub use sophon_push::gateway::deliver;
use sophon_push::{Record, ServiceFile};

fn record_path() -> PathBuf {
    super::data_dir().join("push.json")
}

/// The record of who wants notifications (`push.json`), with this
/// computer's id at the gateway.
pub fn record() -> Option<Record> {
    Some(Record::load(&record_path()))
}

/// Changes the record and keeps it. Whether `change` said anything changed.
pub fn update_record(change: impl FnOnce(&mut Record) -> bool) -> bool {
    let path = record_path();
    let mut r = Record::load(&path);
    let changed = change(&mut r);
    if changed {
        r.save(&path);
    }
    changed
}

/// The gateway this computer uses: its relay's server, if it has one of
/// its own (`host:port` and whether over TLS).
pub fn target() -> Option<(String, bool)> {
    let endpoint = super::connection::relay_endpoint()?;
    let tls = super::connection::relay_use_tls().unwrap_or_else(|| endpoint.ends_with(":443"));
    sophon_push::gateway_url(&endpoint, tls, "").map(|_| (endpoint, tls))
}

/// The id at the gateway of the service of `space` (`None`: the first),
/// for a phone's actions to reach it.
pub fn space_id(space: Option<&str>) -> String {
    record().map(|r| r.space_id(space)).unwrap_or_default()
}

/// The homes a share was written to, so one left with nobody is cleared.
static WRITTEN: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

/// The home of the service of `space`.
fn home_of(space: Option<&str>) -> PathBuf {
    match space {
        Some(home) => PathBuf::from(home),
        None => crate::spaces::home_of_first(),
    }
}

/// Writes each space's share of the record for its service: its id, the
/// gateway, its devices and what checks an action (`secret`, `tokens`: the
/// bridge's key and the pairing tokens good now). First, the devices a
/// service saw forget this computer come off the record; they are
/// returned with their pairing tokens, for the bridge to revoke.
pub fn sync_services(secret: Option<[u8; 32]>, tokens: &[String]) -> Vec<(String, String)> {
    let mut gone = Vec::new();
    let mut homes: HashSet<PathBuf> = WRITTEN.lock().ok().and_then(|w| w.clone()).unwrap_or_default();
    homes.insert(home_of(None));
    let path = record_path();
    let mut r = Record::load(&path);
    for space in r.spaces() {
        homes.insert(home_of(space.as_deref()));
    }
    for home in &homes {
        let forgotten = sophon_push::forgotten(home);
        if forgotten.is_empty() {
            continue;
        }
        for (device, pairing) in forgotten {
            if r.forget(&device, &pairing) {
                log::info!("a paired device forgot this computer: no more push notifications to it");
            }
            gone.push((device, pairing));
        }
        sophon_push::clear_forgotten(home);
    }
    if !gone.is_empty() {
        r.save(&path);
    }
    let (gateway, tls) = target().unwrap_or_default();
    let mut written = HashSet::new();
    for home in homes {
        let first = home == home_of(None);
        let space = if first { None } else { r.spaces().into_iter().flatten().find(|s| home_of(Some(s)) == home) };
        let file = ServiceFile {
            id: r.space_id(space.as_deref()),
            gateway: gateway.clone(),
            tls,
            devices: r.wanting(space.as_deref()),
            secret,
            tokens: tokens.to_vec(),
        };
        if !file.devices.is_empty() {
            written.insert(home.clone());
        }
        file.store(&home);
    }
    if let Ok(mut w) = WRITTEN.lock() {
        *w = Some(written);
    }
    gone
}
