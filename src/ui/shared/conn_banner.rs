//! The connection's word, over the phone's screens as a notice is: a
//! small pill that takes no room of its own ("Connecting…"; "Can't reach
//! your computer" with Retry after a failure; "Connected" for a moment
//! once it is back). An attempt that ends within `store::CONN_QUIET_MS`
//! shows nothing.

use nsh_core::prelude::*;

use crate::daemon::connection::{Stage, Via};
use crate::i18n::t;
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
    /// The device itself has no network: nothing to retry until it has.
    NoNetwork,
}

/// The strip for `conn` and `p` at `now` (ms), if any; `online`: whether
/// the device has a network.
pub(crate) fn banner_of(conn: &Conn, p: &ConnProgress, now: i64, online: bool) -> Option<Banner> {
    match conn {
        _ if !online => Some(Banner::NoNetwork),
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

/// What it says, in a word or two: which step the attempt is at and how
/// long it has taken were a line across the screen that told nobody what
/// to do.
pub(crate) fn words(b: &Banner) -> String {
    match b {
        Banner::Working { .. } => t("Connecting…").to_owned(),
        Banner::Down { .. } => t("Can't reach your computer").to_owned(),
        Banner::Back(_) => t("Connected").to_owned(),
        Banner::NoNetwork => t("Offline").to_owned(),
    }
}

/// The pill itself, `top` points down the screen it is the last child of
/// (it lies over what is there and takes no room): nothing while
/// connected and quiet.
pub(crate) fn conn_banner(top: f32) -> impl IntoElement {
    let banner = computed(|| banner_of(&store::conn(), &store::conn_progress(), store::conn_clock().max(store::now_ms()), store::network_online()));
    // Its kind decides what is built: the clock turns every second while
    // the pill stays as it is.
    let kind = computed(move || {
        banner.with(|b| match b {
            None => 0u8,
            Some(Banner::Working { .. }) => 1,
            Some(Banner::Down { .. }) => 2,
            Some(Banner::Back(_)) => 3,
            Some(Banner::NoNetwork) => 4,
        })
    });
    view().child(dynamic(move || {
        let k = kind.get();
        if k == 0 {
            return None;
        }
        Some(untracked(move || {
            let said = banner.with_untracked(|b| b.as_ref().map(words).unwrap_or_default());
            let mut pill = view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .h(px(30.0))
                .px(sp(S3))
                .rounded(px(R_FULL))
                .bg(FG)
                .shadow_md()
                .child(text(said).text_size(tx(TEXT_SM)).text_color(SURFACE0).number_of_lines(1));
            // After a failure the next attempt comes by itself; the press
            // makes it now.
            if k == 2 {
                pill = pill.pr(sp(S1)).child(
                    nsh_base::button()
                        .on_press(store::retry_now)
                        .accessibility_label(t("Retry now"))
                        .px(sp(S2))
                        .h(px(22.0))
                        .rounded(px(R_FULL))
                        .bg(tint(SURFACE0, 0.18))
                        .items_center()
                        .justify_center()
                        .active(|s| s.opacity(0.6))
                        .child(text(t("Retry")).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).text_color(SURFACE0)),
                );
            }
            view()
                .absolute()
                .top(px(top))
                .left(px(0.0))
                .right(px(0.0))
                .items_center()
                // Only the pill takes a touch: what it lies over stays in reach.
                .pointer_events(PointerEvents::BoxNone)
                .child(pill)
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
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + store::CONN_QUIET_MS - 1, true), None);
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + store::CONN_QUIET_MS, true), Some(Banner::Working { stage: reaching, secs: None }));
        assert_eq!(banner_of(&conn, &progress(reaching, Some(1000)), 1000 + 4200, true), Some(Banner::Working { stage: reaching, secs: Some(4) }));
    }

    #[test]
    fn what_it_says_is_a_word_or_two_whatever_the_step() {
        let securing = Banner::Working { stage: Some(Stage::Securing(Via::Relay)), secs: Some(6) };
        assert_eq!(words(&securing), t("Connecting…"));
        assert_eq!(words(&Banner::Working { stage: None, secs: None }), t("Connecting…"));
        // No step, no count of seconds.
        assert!(!words(&securing).contains('6'));
        assert!(!words(&Banner::Down { retry_in: Some(9) }).contains('9'));
    }

    #[test]
    fn a_failure_counts_down_to_the_next_attempt() {
        let conn = Conn::Down("x".into());
        let p = ConnProgress { retry_at: Some(10_000), since: Some(0), ..Default::default() };
        assert_eq!(banner_of(&conn, &p, 7_100, true), Some(Banner::Down { retry_in: Some(3) }));
        assert_eq!(banner_of(&conn, &p, 10_000, true), Some(Banner::Down { retry_in: None }));
        // The next attempt under way: its step, not the failure.
        let trying = ConnProgress { stage: Some(Stage::Finding), ..p };
        assert!(matches!(banner_of(&conn, &trying, 11_000, true), Some(Banner::Working { .. })));
    }

    #[test]
    fn back_is_said_for_a_moment_with_its_route() {
        let conn = Conn::Connected { hostname: String::new(), version: String::new(), url: String::new() };
        let p = ConnProgress { via: Some(Via::Lan), connected_until: Some(5_000), ..Default::default() };
        assert_eq!(banner_of(&conn, &p, 4_000, true), Some(Banner::Back(Some(Via::Lan))));
        assert_eq!(banner_of(&conn, &p, 5_000, true), None);
        assert_eq!(banner_of(&conn, &ConnProgress::default(), 0, true), None);
    }

    #[test]
    fn an_offline_device_says_so_not_that_the_computer_is_gone() {
        let down = ConnProgress { retry_at: Some(10_000), since: Some(0), ..Default::default() };
        assert_eq!(banner_of(&Conn::Down("x".into()), &down, 1_000, false), Some(Banner::NoNetwork));
        assert_eq!(banner_of(&Conn::Connecting(String::new()), &ConnProgress::default(), 0, false), Some(Banner::NoNetwork));
    }
}
