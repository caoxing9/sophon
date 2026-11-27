//! Joining class strings, shadcn's `cn`. NSH needs no `tailwind-merge`:
//! in one class string the later class wins, so the app's classes go last.

use nsh_core::prelude::*;

/// The classes of `parts` in order, empty parts left out.
pub(crate) fn cn<'a>(parts: impl IntoIterator<Item = &'a str>) -> SharedString {
    let mut out = String::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(part);
    }
    SharedString::from(out)
}

/// The classes of `classes` NSH does not know, parsed as an app with NSH's
/// default theme (shadcn's neutral tokens) parses them. Every component's
/// tests assert that this is empty for every class string it can draw with:
/// an unknown class is dropped at run time with only a log line.
#[cfg(test)]
pub(crate) fn unknown_classes(classes: &str) -> Vec<String> {
    let mut theme = nsh_core::tailwind::Theme::default();
    theme.set_tokens(&nsh_base::tokens_of(None).expect("the default theme"));
    nsh_core::tailwind::parse(&theme, classes)
        .unknown()
        .iter()
        .map(|class| class.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::cn;

    #[test]
    fn joins_in_order_and_skips_empty_parts() {
        assert_eq!(cn(["a b", "", "  c  ", "d"]).as_ref(), "a b c d");
    }
}
