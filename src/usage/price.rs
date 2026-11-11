//! What the tokens would cost at the providers' API list prices, in USD
//! per million tokens. An estimate: subscriptions are billed otherwise, and
//! a model with no known price is shown as unpriced rather than guessed.
//!
//! The prices come from models.dev's public list (`LIST`), fetched at most
//! once a day and kept in the data folder, so a new model is priced without
//! a new build. The table here is what is known with no network, and what
//! the list does not carry.

use super::Tokens;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Price {
    pub(crate) input: f64,
    pub(crate) output: f64,
    /// Writing the prompt cache (Anthropic's 5-minute rate; an hour's
    /// writes cost more, which the logs do not always tell apart).
    pub(crate) cache_write: f64,
    pub(crate) cache_read: f64,
}

const fn p(input: f64, output: f64, cache_write: f64, cache_read: f64) -> Price {
    Price { input, output, cache_write, cache_read }
}

/// Model ids as the logs write them (dates and provider prefixes cut off
/// first, see [`of`]). Anthropic's and OpenAI's pricing pages as of
/// 2026-11-11; OpenAI's at its standard tier and short context (a prompt
/// over 272K tokens costs more, which is not counted).
const TABLE: &[(&str, Price)] = &[
    ("claude-fable-5-2", p(8.0, 40.0, 10.0, 0.20)),
    ("claude-fable-5-1", p(10.0, 50.0, 12.5, 0.25)),
    ("claude-fable-5", p(10.0, 50.0, 12.5, 1.0)),
    ("claude-opus-5-5", p(4.0, 20.0, 5.0, 0.20)),
    ("claude-opus-5", p(5.0, 25.0, 6.25, 0.50)),
    ("claude-opus-4-8", p(5.0, 25.0, 6.25, 0.50)),
    ("claude-opus-4-7", p(5.0, 25.0, 6.25, 0.50)),
    ("claude-opus-4-6", p(5.0, 25.0, 6.25, 0.50)),
    ("claude-opus-4-5", p(5.0, 25.0, 6.25, 0.50)),
    ("claude-opus-4-1", p(15.0, 75.0, 18.75, 1.50)),
    ("claude-opus-4", p(15.0, 75.0, 18.75, 1.50)),
    ("claude-sonnet-5-5", p(2.0, 10.0, 2.5, 0.20)),
    ("claude-sonnet-5", p(2.0, 10.0, 2.5, 0.20)),
    ("claude-sonnet-4-6", p(3.0, 15.0, 3.75, 0.30)),
    ("claude-sonnet-4-5", p(3.0, 15.0, 3.75, 0.30)),
    ("claude-sonnet-4", p(3.0, 15.0, 3.75, 0.30)),
    ("claude-haiku-5-5", p(0.10, 0.50, 0.125, 0.01)),
    ("claude-haiku-4-5", p(1.0, 5.0, 1.25, 0.10)),
    ("gpt-6.1-sol", p(2.0, 10.0, 2.5, 0.10)),
    ("gpt-6-sol", p(2.0, 10.0, 2.5, 0.20)),
    // Its promotional price, to 2026-11-30.
    ("gpt-5.6-sol", p(4.0, 20.0, 5.0, 0.40)),
    ("gpt-5", p(1.25, 10.0, 0.0, 0.125)),
    ("gpt-5-codex", p(1.25, 10.0, 0.0, 0.125)),
    ("gpt-5-mini", p(0.25, 2.0, 0.0, 0.025)),
];

/// models.dev's list of every provider's models, with their costs.
const LIST: &str = "https://models.dev/api.json";
/// How long a fetched list is good for, in seconds.
const FRESH: i64 = 24 * 3600;

/// The fetched prices, by model id; empty until loaded.
static LIVE: std::sync::RwLock<Option<std::collections::HashMap<String, Price>>> = std::sync::RwLock::new(None);

/// The prices in models.dev's list, by model id. A model many providers
/// resell is priced as its maker sells it: the makers are read first.
pub(crate) fn parse_list(text: &str) -> std::collections::HashMap<String, Price> {
    const MAKERS: [&str; 8] = ["anthropic", "openai", "google", "xai", "deepseek", "moonshotai", "zhipuai", "alibaba"];
    let mut prices = std::collections::HashMap::new();
    let Ok(serde_json::Value::Object(providers)) = serde_json::from_str::<serde_json::Value>(text) else { return prices };
    let makers = MAKERS.iter().filter_map(|m| providers.get(*m));
    let others = providers.iter().filter(|(name, _)| !MAKERS.contains(&name.as_str())).map(|(_, p)| p);
    for provider in makers.chain(others) {
        let Some(models) = provider.get("models").and_then(serde_json::Value::as_object) else { continue };
        for (id, model) in models {
            let cost = |key: &str| model.pointer(&format!("/cost/{key}")).and_then(serde_json::Value::as_f64);
            let (Some(input), Some(output)) = (cost("input"), cost("output")) else { continue };
            // Free tiers and placeholders are not a price.
            if input <= 0.0 && output <= 0.0 {
                continue;
            }
            let price = Price { input, output, cache_write: cost("cache_write").unwrap_or(0.0), cache_read: cost("cache_read").unwrap_or(input) };
            prices.entry(key(id)).or_insert(price);
        }
    }
    prices
}

/// The file the fetched prices are kept in: `{ "at": <secs>, "prices": { id: [input, output, write, read] } }`.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
fn kept() -> std::path::PathBuf {
    crate::daemon::data_dir().join("model-prices.json")
}

/// Brings the fetched prices in: from the file kept, and from models.dev
/// when that is a day old. Called where the logs are summed, off the UI's
/// thread. The first fetch is waited for (the sums need it); a later one
/// is not, and serves the next sums.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn refresh() {
    if cfg!(test) {
        return;
    }
    let read = || -> Option<(i64, std::collections::HashMap<String, Price>)> {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(kept()).ok()?).ok()?;
        let prices = v.get("prices")?.as_object()?.iter().filter_map(|(id, p)| {
            let n = |i: usize| p.get(i).and_then(serde_json::Value::as_f64);
            Some((id.clone(), Price { input: n(0)?, output: n(1)?, cache_write: n(2)?, cache_read: n(3)? }))
        });
        Some((v.get("at")?.as_i64()?, prices.collect()))
    };
    let fetch = || {
        let Ok(body) = crate::update::curl(LIST) else { return };
        let prices = parse_list(&String::from_utf8_lossy(&body));
        // A page that is not the list (an error page, a changed shape) keeps what there was.
        if prices.len() < 50 {
            return;
        }
        let rows: serde_json::Map<String, serde_json::Value> =
            prices.iter().map(|(id, p)| (id.clone(), serde_json::json!([p.input, p.output, p.cache_write, p.cache_read]))).collect();
        let _ = std::fs::write(kept(), serde_json::json!({ "at": super::now(), "prices": rows }).to_string());
        *LIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(prices);
    };
    match read() {
        Some((at, prices)) => {
            if LIVE.read().unwrap_or_else(|e| e.into_inner()).is_none() {
                *LIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(prices);
            }
            if super::now() - at > FRESH {
                std::thread::spawn(fetch);
            }
        }
        None => fetch(),
    }
}

/// A model's id as the table and the list are keyed: without a route
/// prefix (`anthropic/`) or a date suffix (`-20250929`), in lower case.
fn key(model: &str) -> String {
    let id = model.rsplit('/').next().unwrap_or(model).to_ascii_lowercase();
    match id.rsplit_once('-') {
        Some((head, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => head.to_owned(),
        _ => id,
    }
}

/// The price of `model`: its id without a date suffix (`-20250929`) or a
/// route prefix (`anthropic/`), matched whole, so a newer model never takes
/// an older one's price.
pub(crate) fn of(model: &str) -> Option<Price> {
    let id = key(model);
    // The fetched list first: it is the newer of the two.
    if let Some(price) = LIVE.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|live| live.get(&id)) {
        return Some(*price);
    }
    TABLE.iter().find(|(name, _)| *name == id).map(|(_, price)| *price)
}

/// The estimated cost of `tokens` on `model`; `None` when unpriced.
pub(crate) fn cost(model: &str, tokens: &Tokens) -> Option<f64> {
    let p = of(model)?;
    Some(
        (tokens.input as f64 * p.input
            + tokens.output as f64 * p.output
            + tokens.cache_write as f64 * p.cache_write
            + tokens.cache_read as f64 * p.cache_read)
            / 1e6,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_whole_after_trimming() {
        assert_eq!(of("claude-opus-5-5").map(|p| p.input), Some(4.0));
        assert_eq!(of("claude-sonnet-4-5-20250929").map(|p| p.input), Some(3.0));
        assert_eq!(of("anthropic/claude-haiku-4-5").map(|p| p.output), Some(5.0));
        // A newer model is unpriced, not priced as its family.
        assert_eq!(of("claude-fable-5-3"), None);
        assert_eq!(of("gpt-6.1-terra"), None);
        assert_eq!(of("claude-fable-5-2").map(|p| p.input), Some(8.0));
        assert_eq!(of("codex/gpt-6.1-sol").map(|p| p.cache_read), Some(0.10));
        assert_eq!(of("<synthetic>"), None);
    }

    #[test]
    fn cost_per_million() {
        let t = Tokens { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000, cache_write: 1_000_000 };
        let c = cost("claude-opus-5-5", &t).unwrap();
        assert!((c - (4.0 + 20.0 + 5.0 + 0.20)).abs() < 1e-9);
    }

    #[test]
    fn the_public_list_prices_a_model_as_its_maker_does() {
        let list = r#"{
            "reseller": {"models": {"claude-new-9": {"cost": {"input": 99, "output": 99}}, "only-here": {"cost": {"input": 1, "output": 2}}}},
            "anthropic": {"models": {
                "claude-new-9": {"cost": {"input": 8, "output": 40, "cache_read": 0.2, "cache_write": 10}},
                "claude-dated-20270101": {"cost": {"input": 3, "output": 15}},
                "free-one": {"cost": {"input": 0, "output": 0}},
                "no-cost": {}
            }}
        }"#;
        let prices = parse_list(list);
        assert_eq!(prices.get("claude-new-9"), Some(&p(8.0, 40.0, 10.0, 0.2)));
        // No cache prices given: a read costs as input, a write nothing more.
        assert_eq!(prices.get("claude-dated"), Some(&p(3.0, 15.0, 0.0, 3.0)));
        assert_eq!(prices.get("only-here").map(|p| p.output), Some(2.0));
        assert!(!prices.contains_key("free-one") && !prices.contains_key("no-cost"));
        assert!(parse_list("<html>").is_empty());
    }
}
