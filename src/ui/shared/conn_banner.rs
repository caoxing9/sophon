//! The connection's strip, under the phone's headers: what the attempt is
//! doing ("Connecting through the relay… · 4 s"), when the next one starts
//! after a failure (with Retry now), and "Connected · over the local
//! network" for a moment once it is back. An attempt that ends within
//! `store::CONN_QUIET_MS` shows nothing.

use nsh_core::prelude::*;

use crate::daemon::connection::{Stage, Via};
use crate::i18n::{t, tf};
use crate::store::{self, Conn, ConnProgress};
use crate::ui::kit::text;
use crate::ui::kit::*;

/// What the strip says.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Banner {
    /// An attempt under way: its step, and how long this outage has lasted
    /// (seconds) once that is worth saying.
    Working { stage: Option<Stage>, secs: Option<i64> },
    /// It failed: the seconds until the next attempt (`None`: now).
    Down { retry_in: Option<i64> },
    /// Back, and how.
    Back(Option<Via>),
}

/// The strip for `conn` and `p` at `now` (ms), if any.
pub(crate) fn banner_of(conn: &Conn, p: &ConnProgress, now: i64) -> Option<Banner> {
    match conn {
        Conn::Connected { .. } => p.connected_until.filter(|u| *u > now).map(|_| Banner::Back(p.via)),
        Conn::Down(_) if p.stage.is_none() => {
            let left = p.retry_at.map(|at| (at - now + 999) / 1000).filter(|s| *s > 0);
            Some(Banner::Down { retry_in: left })
        }
        _ => {
            let since = p.since.unwrap_or(now);
            if now - since < store::CONN_QUIET_MS {
                return None;
            }
            let secs = (now - since) / 1000;
            Some(Banner::Working { stage: p.stage, secs: (secs >= 3).then_some(secs) })
        }
    }
}

/// What it says, in words.
pub(crate) fn words(b: &Banner) -> String {
    match b {
        Banner::Working { stage, secs } => {
            let step = match stage {
                Some(Stage::Finding) => t("Looking for your computer on this network…"),
                Some(Stage::Reaching(Via::Lan)) => t("Connecting over the local network…"),
                Some(Stage::Reaching(Via::Relay)) => t("Connecting through the relay…"),
                Some(Stage::Securing(_)) => t("Securing the connection…"),
                Some(Stage::Greeting(_)) => t("Waiting for your computer to answer…"),
                _ => t("Connecting…"),
            };
            match secs {
                Some(s) => tf("{} · {} s", &[step, &s.to_string()]),
                None => step.to_owned(),
            }
        }
        Banner::Down { retry_in: Some(s) } => tf("Can't reach your computer · retrying in {} s", &[&s.to_string()]),
        Banner::Down { retry_in: None } => t("Can't reach your computer · retrying…").to_owned(),
        Banner::Back(Some(Via::Lan)) => t("Connected · over the local network").to_owned(),
        Banner::Back(Some(Via::Relay)) => t("Connected · through the relay").to_owned(),
        Banner::Back(_) => t("Connected").to_owned(),
    }
}

/// The strip itself: nothing while connected and quiet.
pub(crate) fn conn_banner() -> impl IntoElement {
    let banner = computed(|| banner_of(&store::conn(), &store::conn_progress(), store::conn_clock().max(store::now_ms())));
    // Its kind, not its words, decides what is built: the words change
    // every second while the strip stays.
    let kind = computed(move || {
        banner.with(|b| match b {
            None => 0u8,
            Some(Banner::Working { .. }) => 1,
            Some(Banner::Down { .. }) => 2,
            Some(Banner::Back(_)) => 3,
        })
    });
    let said = move || banner.with(|b| b.as_ref().map(words).unwrap_or_default());
    view().child(dynamic(move || {
        let k = kind.get();
        Some(untracked(move || {
            let strip = |tone: AdaptiveColor| {
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .min_h(px(32.0))
                    .px(sp(S3))
                    .mx(sp(S3))
                    .mt(sp(S1))
                    .mb(sp(S1))
                    .rounded(px(R_FULL))
                    .bg(tint(tone, 0.12))
            };
            let words = |tone: AdaptiveColor| text(said).text_size(tx(TEXT_SM)).text_color(tone).number_of_lines(1).flex_1();
            match k {
                1 => strip(FG_MUTED).child(eclipse(14.0)).child(words(FG_MUTED)).into_element(),
                2 => strip(WARNING)
                    .child(icon(Icon::CircleAlert, 14.0).text_color(WARNING))
                    .child(words(WARNING))
                    .child(
                        nsh_base::button()
                            .on_press(store::retry_now)
                            .accessibility_label(t("Retry now"))
                            .px(sp(S2))
                            .h(px(26.0))
                            .rounded(px(R_FULL))
                            .bg(tint(WARNING, 0.18))
                            .items_center()
                            .justify_center()
                            .active(|s| s.opacity(0.6))
                            .child(text(t("Retry now")).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).text_color(WARNING)),
                    )
                    .into_element(),
                3 => {
                    let glyph = match store::conn_progress().via {
                        Some(Via::Lan) => Icon::Wifi,
                        Some(Via::Relay) => Icon::Cloud,
                        _ => Icon::Check,
                    };
                    strip(SUCCESS).child(icon(glyph, 14.0).text_color(SUCCESS)).child(words(SUCCESS)).into_element()
                }
                _ => view().into_element(),
            }
        }))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(stage: Option<Stage>, since: Option<i64>) -> ConnProgress {
        ConnProgress { stage, via: None, since, retry_at: None, connected_until: None }
    }

    #[test]
    fn a_quick_attempt_shows_nothing_a_slow_one_its_step_and_time() {
        let reaching = Some(Stage::Reaching(Via::Relay));
        let conn = Conn::Connecting(String::new());
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + store::CONN_QUIET_MS - 1), None);
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + store::CONN_QUIET_MS), Some(Banner::Working { stage: reaching, secs: None }));
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + 4200), Some(Banner::Working { stage: reaching, secs: Some(4) }));
    }

    #[test]
    fn a_failure_counts_down_to_the_next_attempt() {
        let conn = Conn::Down("x".into());
        let p = ConnProgress { retry_at: Some(10_000), since: Some(0), ..Default::default() };
        assert_eq!(banner_of(&conn, &p, 7_100), Some(Banner::Down { retry_in: Some(3) }));
        assert_eq!(banner_of(&conn, &p, 10_000), Some(Banner::Down { retry_in: None }));
        // The next attempt under way: its step, not the failure.
        let trying = ConnProgress { stage: Some(Stage::Finding), ..p };
        assert!(matches!(banner_of(&conn, &trying, 11_000), Some(Banner::Working { .. })));
    }

    #[test]
    fn back_is_said_for_a_moment_with_its_route() {
        let conn = Conn::Connected { hostname: String::new(), version: String::new(), url: String::new() };
        let p = ConnProgress { via: Some(Via::Lan), connected_until: Some(5_000), ..Default::default() };
        assert_eq!(banner_of(&conn, &p, 4_000), Some(Banner::Back(Some(Via::Lan))));
        assert_eq!(banner_of(&conn, &p, 5_000), None);
        assert_eq!(banner_of(&conn, &ConnProgress::default(), 0), None);
    }
}
