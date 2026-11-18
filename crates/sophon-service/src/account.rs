use crate::provider;
use serde_json::{Value, json};
use std::{io, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

async fn codex(method: &str, params: &Value) -> Result<Value, String> {
    let binary = provider::binary("codex").ok_or("Codex is not installed")?;
    let mut child = Command::new(binary)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut input = child.stdin.take().ok_or("Missing CLI input")?;
    let mut lines = BufReader::new(child.stdout.take().ok_or("Missing CLI output")?).lines();
    let exchange = async {
        let initialize = json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"sophon","version":env!("CARGO_PKG_VERSION")},"capabilities":null}});
        input
            .write_all(format!("{initialize}\n").as_bytes())
            .await?;
        for expected in [1, 2] {
            loop {
                let line = lines
                    .next_line()
                    .await?
                    .ok_or_else(|| io::Error::other("Codex stopped before replying"))?;
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if message["id"] != expected {
                    continue;
                }
                if let Some(error) = message.pointer("/error/message").and_then(Value::as_str) {
                    return Err(io::Error::other(error.to_owned()));
                }
                if expected == 2 {
                    return Ok(message["result"].clone());
                }
                let query = json!({"id":2,"method":method,"params":params});
                input
                    .write_all(format!("{{\"method\":\"initialized\"}}\n{query}\n").as_bytes())
                    .await?;
                break;
            }
        }
        Err(io::Error::other("Codex did not answer"))
    };
    timeout(Duration::from_secs(20), exchange)
        .await
        .map_err(|_| "Codex did not answer in time".to_owned())?
        .map_err(|error| error.to_string())
}

fn iso(date: &Value) -> Option<String> {
    if let Some(number) = date.as_i64() {
        return if number < 1_000_000_000_000 {
            chrono::DateTime::from_timestamp(number, 0)
        } else {
            chrono::DateTime::from_timestamp_millis(number)
        }
        .map(|time| time.to_rfc3339());
    }
    chrono::DateTime::parse_from_rfc3339(date.as_str()?)
        .ok()
        .map(|time| time.to_utc().to_rfc3339())
}

fn credits(summary: &Value) -> Value {
    let mut usable: Vec<_> = summary["credits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["status"] == "available" && row["id"].is_string())
        .collect();
    usable.sort_by_key(|row| iso(&row["expiresAt"]).unwrap_or_else(|| "~".into()));
    let mut options = Vec::<Value>::new();
    for row in &usable {
        let kind = row["resetType"].as_str().unwrap_or("reset");
        if let Some(option) = options.iter_mut().find(|option| option["kind"] == kind) {
            option["count"] = json!(option["count"].as_u64().unwrap_or_default() + 1);
        } else {
            options.push(json!({"kind":kind,"count":1,"creditId":row["id"],"label":row["title"].as_str().unwrap_or("Rate limit reset"),
                "description":row["description"],"expiresAt":iso(&row["expiresAt"])}));
        }
    }
    let available = summary["availableCount"]
        .as_u64()
        .unwrap_or(usable.len() as u64);
    if available > 0 && options.is_empty() {
        options.push(json!({"kind":"reset","label":"Rate limit reset","count":available}));
    }
    json!({"provider":"codex","available":available,"expiresAt":options.first().map(|option|&option["expiresAt"]),"options":options})
}

pub async fn handle(request: &Value) -> Result<Value, String> {
    match request["type"].as_str() {
        Some("provider.usage.list.request") => {
            let limits = codex("account/rateLimits/read", &Value::Null).await?;
            let limits = &limits["rateLimits"];
            let mut windows = Vec::new();
            for (id, label) in [("primary", "Session"), ("secondary", "Weekly")] {
                if limits[id].is_object() {
                    windows.push(json!({"id":id,"label":label,"usedPct":limits[id]["usedPercent"],"resetsAt":iso(&limits[id]["resetsAt"])}));
                }
            }
            Ok(
                json!({"providers":[{"providerId":"codex","displayName":"Codex","planLabel":limits["planType"],"windows":windows}]}),
            )
        }
        Some("provider.credits.list.request") => {
            if provider::binary("codex").is_none() {
                return Ok(json!({"providers":[]}));
            }
            let limits = codex("account/rateLimits/read", &Value::Null).await?;
            Ok(json!({"providers":[credits(&limits["rateLimitResetCredits"])]}))
        }
        Some("provider.credits.use.request") => {
            let input = &request["input"];
            if input["provider"] != "codex" {
                return Err("Reset credits are unavailable for this provider".into());
            }
            let key = input["requestId"]
                .as_str()
                .filter(|key| {
                    !key.is_empty()
                        && key.len() <= 64
                        && key
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                })
                .ok_or("Invalid reset request ID")?;
            let mut params = json!({"idempotencyKey":key});
            if input["creditId"].is_string() {
                params["creditId"] = input["creditId"].clone()
            }
            let result = codex("account/rateLimitResetCredit/consume", &params).await?;
            let outcome = match result["outcome"].as_str() {
                Some("reset") => "reset",
                Some("nothingToReset") => "nothing_to_reset",
                Some("noCredit") => "no_credit",
                Some("alreadyRedeemed") => "already_redeemed",
                _ => return Err("Codex could not confirm the reset".into()),
            };
            Ok(json!({"outcome":outcome}))
        }
        _ => Err("Unknown account operation".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_credits_group_by_kind_and_spend_first_expiry() {
        let available = credits(&json!({"credits":[
            {"id":"later","status":"available","resetType":"session","expiresAt":"2027-02-01T00:00:00Z"},
            {"id":"first","status":"available","resetType":"session","expiresAt":"2027-01-01T00:00:00Z"},
            {"id":"spent","status":"redeemed"}]}));
        assert_eq!(available["available"], 2);
        assert_eq!(available["options"][0]["creditId"], "first");
        assert_eq!(available["options"][0]["count"], 2);
    }
}
