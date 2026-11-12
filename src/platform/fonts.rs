//! The fonts installed on this device, by family name, for the settings'
//! font pickers: AppKit's font manager on the Mac, UIKit's list on the
//! phone. Elsewhere there is no list, and a name is typed.

/// A family's name, and whether every letter of it is as wide as the next
/// (a font for code).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Family {
    pub(crate) name: String,
    pub(crate) fixed: bool,
}

/// What a name alone says, where the system does not: the usual names of
/// fonts for code.
fn looks_fixed(name: &str) -> bool {
    let n = name.to_lowercase();
    ["mono", "code", "courier", "menlo", "monaco", "consol", "terminal", "fixed"].iter().any(|w| n.contains(w))
}

/// The installed families, by name, without the system's hidden ones (a
/// leading dot). On the main thread.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) fn families() -> Vec<Family> {
    use objc2::{msg_send, rc::Retained, runtime::{AnyClass, AnyObject}};
    use objc2_foundation::NSString;
    let mut list: Vec<Family> = Vec::new();
    // SAFETY: documented class and instance methods returning an array of
    // strings, a font or nil, and a boolean.
    unsafe {
        #[cfg(target_os = "macos")]
        let names: Option<Retained<AnyObject>> = AnyClass::get(c"NSFontManager").map(|c| {
            let manager: Retained<AnyObject> = msg_send![c, sharedFontManager];
            msg_send![&*manager, availableFontFamilies]
        });
        #[cfg(target_os = "ios")]
        let names: Option<Retained<AnyObject>> = AnyClass::get(c"UIFont").map(|c| msg_send![c, familyNames]);
        let Some(names) = names else { return list };
        let count: usize = msg_send![&*names, count];
        for i in 0..count {
            let name: Retained<NSString> = msg_send![&*names, objectAtIndex: i];
            let text = name.to_string();
            if text.starts_with('.') || text.is_empty() {
                continue;
            }
            // The Mac says which are fixed pitch; the phone's are told by name.
            #[cfg(target_os = "macos")]
            let fixed = AnyClass::get(c"NSFont")
                .and_then(|c| {
                    let font: Option<Retained<AnyObject>> = msg_send![c, fontWithName: &*name, size: 12.0f64];
                    font
                })
                .map(|font| {
                    let fixed: bool = msg_send![&*font, isFixedPitch];
                    fixed
                })
                .unwrap_or_else(|| looks_fixed(&text));
            #[cfg(target_os = "ios")]
            let fixed = looks_fixed(&text);
            list.push(Family { name: text, fixed });
        }
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list.dedup_by(|a, b| a.name == b.name);
    list
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub(crate) fn families() -> Vec<Family> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_font_is_told_by_its_name_where_the_system_does_not_say() {
        for name in ["JetBrains Mono", "Fira Code", "Courier New", "Menlo", "SF Mono", "Monaco"] {
            assert!(looks_fixed(name), "{name}");
        }
        for name in ["Helvetica Neue", "Georgia", "PingFang SC"] {
            assert!(!looks_fixed(name), "{name}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn this_mac_lists_its_fonts_and_knows_menlo_is_for_code() {
        let list = families();
        assert!(list.len() > 20, "{} families", list.len());
        assert!(list.iter().any(|f| f.name == "Menlo" && f.fixed));
        assert!(list.iter().any(|f| f.name == "Helvetica" && !f.fixed));
        assert!(list.iter().all(|f| !f.name.starts_with('.')));
    }
}
