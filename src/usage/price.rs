//! What the tokens would cost at the providers' API list prices, in USD
//! per million tokens. An estimate: subscriptions are billed otherwise, and
//! a model missing here is shown as unpriced rather than guessed.

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
/// over 272K tokens costs more, which is not counted), with no charge for
/// writing its cache.
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
    ("gpt-6.1-sol", p(2.0, 10.0, 0.0, 0.10)),
    ("gpt-6-sol", p(2.0, 10.0, 0.0, 0.20)),
    // Its promotional price, to 2026-11-30.
    ("gpt-5.6-sol", p(4.0, 20.0, 0.0, 0.40)),
    ("gpt-5", p(1.25, 10.0, 0.0, 0.125)),
    ("gpt-5-codex", p(1.25, 10.0, 0.0, 0.125)),
    ("gpt-5-mini", p(0.25, 2.0, 0.0, 0.025)),
];

/// The price of `model`: its id without a date suffix (`-20250929`) or a
/// route prefix (`anthropic/`), matched whole, so a newer model never takes
/// an older one's price.
pub(crate) fn of(model: &str) -> Option<Price> {
    let id = model.rsplit('/').next().unwrap_or(model).to_ascii_lowercase();
    let id = match id.rsplit_once('-') {
        Some((head, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => head.to_owned(),
        _ => id,
    };
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
}
