//! Text stays as it was typed, on macOS. AppKit's text views follow the
//! system's "smart quotes and dashes" (on unless a person turned it off):
//! `"x"` becomes `“x”`, `--` an em dash, `...` an ellipsis, and two spaces
//! a full stop. A message to a coding agent is often a command, a path or
//! a line of code, which those break. The app's own defaults say no to
//! each, as a terminal's and an editor's do; the system's setting, and
//! every other app's, stay as they are.

use objc2::{class, msg_send, runtime::AnyObject};
use objc2_foundation::NSString;

/// The substitutions an AppKit text view makes on its own
/// (`NSUserDefaults` keys it reads when it is made).
const SUBSTITUTIONS: [&str; 4] = [
    "NSAutomaticQuoteSubstitutionEnabled",
    "NSAutomaticDashSubstitutionEnabled",
    "NSAutomaticPeriodSubstitutionEnabled",
    "NSAutomaticCapitalizationEnabled",
];

/// Turns the substitutions off for this app, before any text input is
/// made (the app's root calls it first).
pub(crate) fn keep_as_typed() {
    // SAFETY: `standardUserDefaults` is the process's shared, thread-safe
    // object; `setBool:forKey:` takes a BOOL and an NSString.
    unsafe {
        let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
        if defaults.is_null() {
            return;
        }
        for key in SUBSTITUTIONS {
            let key = NSString::from_str(key);
            let _: () = msg_send![defaults, setBool: false, forKey: &*key];
        }
    }
}
