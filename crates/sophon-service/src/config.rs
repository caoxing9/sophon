use serde_json::{Value, json};
use std::{fs, io, path::Path};

pub fn read(home: &Path) -> io::Result<Value> {
    match fs::read(home.join("config.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(error),
    }
}

fn merge(target: &mut Value, patch: &Value) {
    if let Some(fields) = patch.as_object() {
        if !target.is_object() {
            *target = json!({});
        }
        for (key, value) in fields {
            merge(&mut target[key], value)
        }
    } else {
        *target = patch.clone()
    }
}

pub fn update(home: &Path, patch: &Value) -> io::Result<Value> {
    let mut config = read(home)?;
    let mut changes = patch.clone();
    let object = changes
        .as_object_mut()
        .ok_or_else(|| io::Error::other("Expected a config object"))?;
    if let Some(ids) = object.remove("removeProviders") {
        for id in ids
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if let Some(providers) = config["providers"].as_object_mut() {
                providers.remove(id);
            }
        }
    }
    merge(&mut config, &changes);
    crate::state::write(home, "config.json", &serde_json::to_vec_pretty(&config)?)?;
    Ok(config)
}
