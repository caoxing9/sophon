//! Icons: the Lucide glyphs (ISC license, <https://lucide.dev>) shadcn/ui's
//! components and examples use, drawn by NSH's `svg` element.
//!
//! Each glyph strokes with `currentColor`, which `svg` resolves to the
//! element's text color, so an icon takes the color of the text around it,
//! as `lucide-react` icons do in shadcn. shadcn sizes icons in components
//! with `size-4` (16 points); [`icon`] does the same.

use nsh_core::element::{ImageSrc, Svg};
use nsh_core::prelude::*;

macro_rules! icons {
    ($( $variant:ident => $name:literal, $body:literal; )*) => {
        /// A Lucide icon of the built-in set; in TSX its Lucide name
        /// (`"chevron-down"`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PropEnum)]
        #[non_exhaustive]
        pub enum IconName {
            $(
                #[doc = concat!("`", $name, "`")]
                #[prop(rename = $name)]
                $variant,
            )*
        }

        impl IconName {
            /// Every icon.
            pub const ALL: &'static [IconName] = &[$( IconName::$variant, )*];

            /// The Lucide name (`chevron-down`).
            pub fn name(self) -> &'static str {
                match self { $( IconName::$variant => $name, )* }
            }

            fn body(self) -> &'static str {
                match self { $( IconName::$variant => $body, )* }
            }
        }
    };
}

icons! {
    AlignCenter => "align-center", r#"<path d="M21 5H3"/><path d="M17 12H7"/><path d="M19 19H5"/>"#;
    AlignLeft => "align-left", r#"<path d="M21 5H3"/><path d="M15 12H3"/><path d="M17 19H3"/>"#;
    AlignRight => "align-right", r#"<path d="M21 5H3"/><path d="M21 12H9"/><path d="M21 19H7"/>"#;
    ArrowDown => "arrow-down", r#"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"#;
    ArrowLeft => "arrow-left", r#"<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>"#;
    ArrowRight => "arrow-right", r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#;
    ArrowUp => "arrow-up", r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#;
    Bell => "bell", r#"<path d="M10.268 21a2 2 0 0 0 3.464 0"/><path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326"/>"#;
    Bold => "bold", r#"<path d="M6 12h9a4 4 0 010 8H7a1 1 0 01-1-1V5a1 1 0 011-1h7a4 4 0 010 8"/>"#;
    Bot => "bot", r#"<path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/><path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/>"#;
    Calendar => "calendar", r#"<path d="M8 2v3"/><path d="M16 2v3"/><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/>"#;
    Check => "check", r#"<path d="M20 6 9 17l-5-5"/>"#;
    ChevronDown => "chevron-down", r#"<path d="m6 9 6 6 6-6"/>"#;
    ChevronLeft => "chevron-left", r#"<path d="m15 18-6-6 6-6"/>"#;
    ChevronRight => "chevron-right", r#"<path d="m9 18 6-6-6-6"/>"#;
    ChevronUp => "chevron-up", r#"<path d="m18 15-6-6-6 6"/>"#;
    ChevronsUpDown => "chevrons-up-down", r#"<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>"#;
    Circle => "circle", r#"<circle cx="12" cy="12" r="10"/>"#;
    CircleAlert => "circle-alert", r#"<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>"#;
    CircleCheck => "circle-check", r#"<circle cx="12" cy="12" r="10"/><path d="m16 9-5.5 5.5L8 12"/>"#;
    CircleHelp => "circle-help", r#"<circle cx="12" cy="12" r="10"/><path d="M9 9.5a3 2.5 0 016 0c0 2-2 2-3 3.5"/><path d="M12 17h.01"/>"#;
    Code => "code", r#"<path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/>"#;
    Copy => "copy", r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#;
    CreditCard => "credit-card", r#"<rect width="20" height="14" x="2" y="5" rx="2"/><line x1="2" x2="22" y1="10" y2="10"/><path d="M6 14h2"/>"#;
    Download => "download", r#"<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>"#;
    Ellipsis => "ellipsis", r#"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"#;
    ExternalLink => "external-link", r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>"#;
    Eye => "eye", r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"#;
    EyeOff => "eye-off", r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>"#;
    File => "file", r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/>"#;
    Folder => "folder", r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#;
    GitBranch => "git-branch", r#"<path d="M15 6a9 9 0 0 0-9 9V3"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/>"#;
    GripVertical => "grip-vertical", r#"<circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/>"#;
    Heart => "heart", r#"<path d="M2 9.5a5.5 5.5 0 0 1 9.591-3.676.56.56 0 0 0 .818 0A5.49 5.49 0 0 1 22 9.5c0 2.29-1.5 4-3 5.5l-5.492 5.313a2 2 0 0 1-3 .019L5 15c-1.5-1.5-3-3.2-3-5.5"/>"#;
    House => "house", r#"<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#;
    Image => "image", r#"<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>"#;
    Inbox => "inbox", r#"<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>"#;
    Info => "info", r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"#;
    Italic => "italic", r#"<line x1="19" x2="10" y1="4" y2="4"/><line x1="14" x2="5" y1="20" y2="20"/><line x1="15" x2="9" y1="4" y2="20"/>"#;
    Keyboard => "keyboard", r#"<path d="M10 8h.01"/><path d="M12 12h.01"/><path d="M14 8h.01"/><path d="M16 12h.01"/><path d="M18 8h.01"/><path d="M6 8h.01"/><path d="M7 16h10"/><path d="M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/>"#;
    Loader => "loader", r#"<path d="M12 2v4"/><path d="m16.2 7.8 2.9-2.9"/><path d="M18 12h4"/><path d="m16.2 16.2 2.9 2.9"/><path d="M12 18v4"/><path d="m4.9 19.1 2.9-2.9"/><path d="M2 12h4"/><path d="m4.9 4.9 2.9 2.9"/>"#;
    LoaderCircle => "loader-circle", r#"<path d="M21 12a9 9 0 1 1-6.219-8.56"/>"#;
    Lock => "lock", r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#;
    LogOut => "log-out", r#"<path d="m16 17 5-5-5-5"/><path d="M21 12H9"/><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>"#;
    Mail => "mail", r#"<path d="m22 7-8.991 5.727a2 2 0 0 1-2.009 0L2 7"/><rect x="2" y="4" width="20" height="16" rx="2"/>"#;
    MessageSquare => "message-square", r#"<path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/>"#;
    Minus => "minus", r#"<path d="M5 12h14"/>"#;
    OctagonX => "octagon-x", r#"<path d="m15 9-6 6"/><path d="M2.586 16.726A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2h6.624a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586z"/><path d="m9 9 6 6"/>"#;
    PanelLeft => "panel-left", r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>"#;
    Paperclip => "paperclip", r#"<path d="m16 6-8.414 8.586a2 2 0 0 0 2.829 2.829l8.414-8.586a4 4 0 1 0-5.657-5.657l-8.379 8.551a6 6 0 1 0 8.485 8.485l8.379-8.551"/>"#;
    Plus => "plus", r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#;
    RefreshCw => "refresh-cw", r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#;
    Search => "search", r#"<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>"#;
    Send => "send", r#"<path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"/><path d="m21.854 2.147-10.94 10.939"/>"#;
    Settings => "settings", r#"<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>"#;
    Share => "share", r#"<path d="M12 2v13"/><path d="m16 6-4-4-4 4"/><path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8"/>"#;
    Smile => "smile", r#"<path d="M15 10V9"/><path d="M16.472 15a6 6 0 01-8.943 0"/><path d="M9 10V9"/><circle cx="12" cy="12" r="10"/>"#;
    Sparkles => "sparkles", r#"<path d="M11.017 2.814a1 1 0 0 1 1.966 0l1.051 5.558a2 2 0 0 0 1.594 1.594l5.558 1.051a1 1 0 0 1 0 1.966l-5.558 1.051a2 2 0 0 0-1.594 1.594l-1.051 5.558a1 1 0 0 1-1.966 0l-1.051-5.558a2 2 0 0 0-1.594-1.594l-5.558-1.051a1 1 0 0 1 0-1.966l5.558-1.051a2 2 0 0 0 1.594-1.594z"/><path d="M20 2v4"/><path d="M22 4h-4"/><circle cx="4" cy="20" r="2"/>"#;
    SquarePen => "square-pen", r#"<path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/>"#;
    Star => "star", r#"<path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/>"#;
    Terminal => "terminal", r#"<path d="M12 19h8"/><path d="m4 17 6-6-6-6"/>"#;
    Trash2 => "trash-2", r#"<path d="M10 11v6"/><path d="M14 11v6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"#;
    TriangleAlert => "triangle-alert", r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#;
    Underline => "underline", r#"<path d="M6 4v6a6 6 0 0 0 12 0V4"/><line x1="4" x2="20" y1="20" y2="20"/>"#;
    Upload => "upload", r#"<path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>"#;
    User => "user", r#"<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>"#;
    X => "x", r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#;
    Calculator => "calculator", r#"<rect width="16" height="20" x="4" y="2" rx="2"/><line x1="8" x2="16" y1="6" y2="6"/><line x1="16" x2="16" y1="14" y2="18"/><path d="M16 10h.01"/><path d="M12 10h.01"/><path d="M8 10h.01"/><path d="M12 14h.01"/><path d="M8 14h.01"/><path d="M12 18h.01"/><path d="M8 18h.01"/>"#;
    ArrowUpRight => "arrow-up-right", r#"<path d="M7 7h10v10"/><path d="M7 17 17 7"/>"#;
    BadgeCheck => "badge-check", r#"<path d="M3.85 8.62a4 4 0 0 1 4.78-4.77 4 4 0 0 1 6.74 0 4 4 0 0 1 4.78 4.78 4 4 0 0 1 0 6.74 4 4 0 0 1-4.77 4.78 4 4 0 0 1-6.75 0 4 4 0 0 1-4.78-4.77 4 4 0 0 1 0-6.76Z"/><path d="m16 9-5.5 5.5L8 12"/>"#;
    Bookmark => "bookmark", r#"<path d="M17 3a2 2 0 0 1 2 2v15a1 1 0 0 1-1.496.868l-4.512-2.578a2 2 0 0 0-1.984 0l-4.512 2.578A1 1 0 0 1 5 20V5a2 2 0 0 1 2-2z"/>"#;
    Cloud => "cloud", r#"<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>"#;
    FolderCode => "folder-code", r#"<path d="M10 10.5 8 13l2 2.5"/><path d="m14 10.5 2 2.5-2 2.5"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>"#;
    Popcorn => "popcorn", r#"<path d="M18 8a2 2 0 0 0 0-4 2 2 0 0 0-4 0 2 2 0 0 0-4 0 2 2 0 0 0-4 0 2 2 0 0 0 0 4"/><path d="M10 22 9 8"/><path d="m14 22 1-14"/><path d="M20 8c.5 0 .9.4.8 1l-2.6 12c-.1.5-.7 1-1.2 1H7c-.6 0-1.1-.4-1.2-1L3.2 9c-.1-.6.3-1 .8-1Z"/>"#;
    Slash => "slash", r#"<path d="M22 2 2 22"/>"#;
    GalleryVerticalEnd => "gallery-vertical-end", r#"<path d="M7 2h10"/><path d="M5 6h14"/><rect width="18" height="12" x="3" y="10" rx="2"/>"#;
    Clock => "clock", r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#;
    FileCode => "file-code", r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 12.5 8 15l2 2.5"/><path d="m14 12.5 2 2.5-2 2.5"/>"#;
    FileText => "file-text", r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>"#;
    FileWarning => "file-warning", r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#;
    RefreshCcw => "refresh-ccw", r#"<path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/>"#;
    RotateCcw => "rotate-ccw", r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>"#;
    ThumbsDown => "thumbs-down", r#"<path d="M9 18.12 10 14H4.17a2 2 0 0 1-1.92-2.56l2.33-8A2 2 0 0 1 6.5 2H20a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2.76a2 2 0 0 0-1.79 1.11L12 22a3.13 3.13 0 0 1-3-3.88Z"/><path d="M17 14V2"/>"#;
    ThumbsUp => "thumbs-up", r#"<path d="M15 5.88 14 10h5.83a2 2 0 0 1 1.92 2.56l-2.33 8A2 2 0 0 1 17.5 22H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2.76a2 2 0 0 0 1.79-1.11L12 2a3.13 3.13 0 0 1 3 3.88Z"/><path d="M7 10v12"/>"#;
    CornerDownLeft => "corner-down-left", r#"<path d="M20 4v7a4 4 0 0 1-4 4H4"/><path d="m9 10-5 5 5 5"/>"#;
}

impl IconName {
    /// The icon's SVG document, stroked with `currentColor`.
    pub fn svg_markup(self) -> String {
        format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" "#,
                r#"fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" "#,
                r#"stroke-linejoin="round">{}</svg>"#
            ),
            self.body()
        )
    }
}

/// A decorative icon, 16 points (shadcn's `size-4`), in the text color
/// around it. Hidden from screen readers: the control beside it says what
/// it means. Size it with `.app_class("size-3")` or `.size(px(..))`.
pub fn icon(name: IconName) -> Svg {
    svg(ImageSrc::svg_markup(&name.svg_markup()))
        .size(px(16.0))
        .flex_shrink_0()
        .accessibility_elements_hidden(true)
}

/// An icon that means something on its own: screen readers say `label`.
pub fn labeled_icon(name: IconName, label: impl Into<SharedString>) -> Svg {
    svg(ImageSrc::svg_markup(&name.svg_markup()))
        .size(px(16.0))
        .flex_shrink_0()
        .accessible(true)
        .role(AccessibilityRole::Image)
        .label(label.into())
}
