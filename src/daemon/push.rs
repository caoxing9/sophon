//! Push notifications (`docs/push.md`): the shared part is `sophon-push`
//! (sealing, who wants what, the words, the gateway's socket); the service
//! sends them. What stays here is the app's own: whether an agent's change
//! is worth a word, for the notifications this Mac shows itself.

pub use sophon_push::*;

use super::model::Agent;

// ------------------------------------------------------------ what to say

/// Why an agent is worth a notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    /// It waits for an approval.
    Permission,
    /// It finished its turn.
    Finished,
    /// It needs its owner otherwise (an error, a question).
    Attention,
}

/// Why `next` is worth a notification after `before`, if it is. Only what
/// is new and unseen:
///
/// - an approval it starts to wait for (each one, as it comes);
/// - a result the service marks unread (`requires_attention` turning on):
///   a turn that ended while nobody was looking. A turn that ends in view,
///   or in an agent whose earlier result is still unread, says nothing
///   more: the mark is already there.
pub fn reason(before: Option<&Agent>, next: &Agent) -> Option<Reason> {
    let asking = |a: &Agent| a.pending.first().map(|p| p.id.clone());
    if next.pending.first().is_some() && before.map(asking) != Some(asking(next)) {
        return Some(Reason::Permission);
    }
    // An agent first heard of is not news: the app just connected.
    let unread_now = next.requires_attention && before.is_some_and(|b| !b.requires_attention);
    if !unread_now || !next.pending.is_empty() {
        return None;
    }
    match next.attention_reason.as_deref() {
        Some("error" | "needs_input") => Some(Reason::Attention),
        Some("finished") => Some(Reason::Finished),
        _ if next.is_running() => Some(Reason::Attention),
        _ => Some(Reason::Finished),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::model::Permission;

    fn agent(id: &str, running: bool, pending: &[(&str, &str)], attention: bool) -> Agent {
        Agent {
            id: id.into(),
            title: "sophon · main".into(),
            status: if running { "running" } else { "idle" }.into(),
            requires_attention: attention,
            pending: pending.iter().map(|(id, title)| Permission { id: (*id).into(), title: (*title).into(), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn only_what_is_new_and_unseen_is_worth_a_notification() {
        let unread = |reason: &str, running: bool| Agent { attention_reason: Some(reason.into()), ..agent("a", running, &[], true) };
        let running = agent("a", true, &[], false);
        // A turn that ended unseen: the service marks it unread.
        assert_eq!(reason(Some(&running), &unread("finished", false)), Some(Reason::Finished));
        assert_eq!(reason(Some(&running), &unread("error", false)), Some(Reason::Attention));
        // A turn that ended in view (no mark): nothing.
        assert_eq!(reason(Some(&running), &agent("a", false, &[], false)), None);
        // Still unread from before: another turn ending says nothing more.
        assert_eq!(reason(Some(&unread("finished", true)), &unread("finished", false)), None);
        assert_eq!(reason(Some(&unread("finished", false)), &unread("finished", false)), None);
        // An approval, each as it comes; the same one once.
        let asking = agent("a", true, &[("p1", "npm test")], false);
        assert_eq!(reason(Some(&running), &asking), Some(Reason::Permission));
        assert_eq!(reason(Some(&asking), &asking.clone()), None);
        assert_eq!(reason(Some(&asking), &agent("a", true, &[("p2", "git push")], false)), Some(Reason::Permission));
        // The mark that comes with an approval is the approval's.
        assert_eq!(reason(Some(&asking), &Agent { attention_reason: Some("permission".into()), ..agent("a", true, &[("p1", "npm test")], true) }), None);
        // First heard of (the app just connected): old news, but for an approval waiting.
        assert_eq!(reason(None, &unread("finished", false)), None);
        assert_eq!(reason(None, &asking), Some(Reason::Permission));
        assert_eq!(reason(Some(&running), &running.clone()), None);
    }

}
