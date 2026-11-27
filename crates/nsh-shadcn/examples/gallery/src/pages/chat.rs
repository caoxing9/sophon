//! The chat family's pages: Message, Message Scroller, Bubble, Attachment,
//! Marker and Direction, laid out as shadcn's examples show them (the
//! `max-w-sm` columns of `apps/v4/examples/radix/<name>-*.tsx`).

use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

use super::Page;

pub fn pages() -> Vec<Page> {
    vec![
        Page { id: "message", title: "Message", body: message },
        Page { id: "message-scroller", title: "Message Scroller", body: message_scroller },
        Page { id: "bubble", title: "Bubble", body: bubble },
        Page { id: "attachment", title: "Attachment", body: attachment },
        Page { id: "marker", title: "Marker", body: marker },
        Page { id: "direction", title: "Direction", body: direction },
    ]
}

/// Two `max-w-sm` columns side by side.
fn columns(left: View, right: View) -> View {
    view().flex_row().gap_12().items_start().child(left).child(right)
}

/// A `max-w-sm` column of examples, `gap` points apart.
fn column(gap: f32) -> View {
    view().w(px(384.0)).gap(px(gap))
}

/// Muted small text beside an example.
fn note(s: &'static str) -> Text {
    text(s).text_sm().text_color(move || nsh_base::theme().muted_foreground)
}

/// shadcn's `Avatar` with its `AvatarFallback` (the gallery shows no
/// images, so the fallback shows), drawn with avatar.tsx's classes.
fn avatar(fallback: &'static str) -> View {
    view()
        .app_class(Some(SharedString::from(
            "relative flex size-8 shrink-0 overflow-hidden rounded-full select-none",
        )))
        .child(
            view()
                .app_class(Some(SharedString::from(
                    "flex size-full items-center justify-center rounded-full bg-muted text-sm text-muted-foreground",
                )))
                .child(text(fallback)),
        )
}

/// A photo for image attachments: an SVG landscape (deterministic, and
/// the same `data:` URI in the reference page).
pub const PHOTO: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='96' height='96' viewBox='0 0 96 96'%3E\
%3Cdefs%3E%3ClinearGradient id='s' x1='0' y1='0' x2='0' y2='1'%3E%3Cstop offset='0' stop-color='%2393c5fd'/%3E\
%3Cstop offset='1' stop-color='%23e0f2fe'/%3E%3C/linearGradient%3E%3C/defs%3E\
%3Crect width='96' height='96' fill='url(%23s)'/%3E%3Ccircle cx='68' cy='28' r='10' fill='%23fde68a'/%3E\
%3Cpath d='M0 72 30 40l22 22 14-12 30 26v20H0z' fill='%2365a30d'/%3E\
%3Cpath d='M0 84 24 64l20 14 18-10 34 18v10H0z' fill='%23365314'/%3E%3C/svg%3E";

/// shadcn's `message-demo`, and `message-attachment` beside it.
fn message() -> View {
    let demo = column(24.0)
        .child(
            ui::message()
                .end()
                .child(ui::message_avatar().child(avatar("ME")))
                .child(
                    ui::message_content()
                        .child(ui::bubble().child(ui::bubble_content("Deploying to prod real quick."))),
                ),
        )
        .child(
            ui::message()
                .child(ui::message_avatar().child(avatar("R")))
                .child(
                    ui::message_content()
                        .child(ui::bubble().muted().child(ui::bubble_content("It's 4:55 PM. On a Friday."))),
                ),
        )
        .child(
            ui::message()
                .end()
                .child(ui::message_avatar().child(avatar("ME")))
                .child(
                    ui::message_content()
                        .child(ui::bubble().child(ui::bubble_content("It's a one-line change.")))
                        .child(ui::message_footer().text(Some("Delivered".into()))),
                ),
        )
        .child(
            ui::message()
                .child(ui::message_avatar().child(avatar("R")))
                .child(
                    ui::message_content().child(
                        ui::bubble_group()
                            .child(
                                ui::bubble()
                                    .muted()
                                    .child(ui::bubble_content("It's always a one-line change 😭.")),
                            )
                            .child(
                                ui::bubble()
                                    .muted()
                                    .child(ui::bubble_content("Alright, let me take a look."))
                                    .child(
                                        ui::bubble_reactions()
                                            .accessibility_label(Some("Reactions: thumbs up".into()))
                                            .child(text("👍")),
                                    ),
                            ),
                    ),
                ),
        )
        .child(
            ui::marker().child(
                ui::marker_content("").child(
                    text("")
                        .child(text("Oliver").font_weight(FontWeight::MEDIUM))
                        .child(text(" is typing...")),
                ),
            ),
        );
    let with_attachment = column(32.0)
        .child(
            ui::message().end().child(
                ui::message_content()
                    .child(
                        ui::attachment().vertical().child(
                            ui::attachment_media()
                                .variant(ui::AttachmentMediaVariant::Image)
                                .src(Some(PHOTO.into()))
                                .alt(Some("Workspace".into())),
                        ),
                    )
                    .child(ui::bubble().child(ui::bubble_content(
                        "Here's the image. Can you add it to the PDF? Use it for the cover page.",
                    ))),
            ),
        )
        .child(
            ui::message().child(
                ui::message_content()
                    .child(ui::bubble().muted().child(ui::bubble_content(
                        "Done. Here's the PDF with the image added as the cover page.",
                    )))
                    .child(
                        ui::attachment()
                            .child(ui::attachment_media().icon(Some(IconName::FileText)))
                            .child(
                                ui::attachment_content()
                                    .child(ui::attachment_title("sales-dashboard.pdf"))
                                    .child(ui::attachment_description("PDF · 2.4 MB")),
                            )
                            .child(
                                ui::attachment_actions().child(
                                    ui::attachment_action(IconName::Download, "Download")
                                        .size(ui::ButtonSize::IconSm)
                                        .variant(ui::ButtonVariant::Secondary),
                                ),
                            ),
                    ),
            ),
        )
        .child(
            ui::message()
                .end()
                .child(ui::message_content().child(ui::bubble().child(ui::bubble_content("Thanks. Looks good.")))),
        );
    columns(demo, with_attachment)
}

/// One message of the scripted conversation.
#[derive(Clone, PartialEq)]
struct Line {
    id: SharedString,
    user: bool,
    /// What shows so far: a reply streams in word by word.
    text: Signal<String>,
}

/// The conversation of shadcn's `message-scroller-demo`, in order.
const SCRIPT: [(bool, &str); 8] = [
    (
        true,
        "I'm building a chat for our app and the scroll behavior is driving me nuts. Every time the AI streams a reply, the whole thread jumps around.",
    ),
    (
        false,
        "That's the classic streaming scroll problem. Wrap your message list in MessageScroller and turn on autoScroll: the viewport pins to the bottom as tokens arrive, so users always see the latest text land in place.",
    ),
    (
        true,
        "Okay, but when someone sends a new message the view still feels jarring, like the whole conversation reloads from the top.",
    ),
    (
        false,
        "MessageScrollerItem fixes that with turn anchoring. Set scrollAnchor on the turn that should settle near the top instead of blindly snapping to the document bottom.",
    ),
    (
        true,
        "And if they've scrolled up to re-read an older answer? I don't want to yank them back down.",
    ),
    (
        false,
        "You won't. Auto-scroll only runs when the viewport is already pinned to the bottom, so scrolling up is a deliberate opt-out. When there is content they haven't seen yet, MessageScrollerButton appears at the bottom of the viewport.",
    ),
    (true, "Last one: does this work with assistive tech?"),
    (
        false,
        "MessageScrollerContent is a live region, so screen readers announce new messages as they stream in. The scroll button is a real button, disabled while you are already at the end, so it is no stop for the keyboard focus then. And with autoScroll on, a reply longer than the viewport is followed as it streams in: once it fills the space reserved under the question, the viewport sticks to its last line until you scroll away from it.",
    ),
];

/// A row of the transcript: the user's message in a muted bubble at the
/// end, the assistant's in a ghost bubble (as shadcn's `MessageAnimated`).
fn transcript_row(line: Line) -> impl IntoElement {
    let bubble = if line.user { ui::bubble().muted() } else { ui::bubble().ghost() };
    let text = line.text;
    ui::message_scroller_item().child(
        ui::message()
            .align(if line.user { ui::MessageAlign::End } else { ui::MessageAlign::Start })
            .child(
                ui::message_content()
                    .child(bubble.child(ui::bubble_content("").text(move || Some(text.get().into())))),
            ),
    )
}

/// shadcn's `message-scroller-demo`: a chat card whose transcript opens at
/// its end; Send appends the next message of the script (the user's turns
/// anchor near the top).
fn message_scroller() -> View {
    let lines = signal(
        SCRIPT[..4]
            .iter()
            .enumerate()
            .map(|(i, &(user, text))| Line { id: format!("m{i}").into(), user, text: signal(text.to_owned()) })
            .collect::<Vec<_>>(),
    );
    let next = move || lines.with(|l| SCRIPT.get(l.len()).copied());
    // The user's message lands whole; a reply streams in, a word every
    // 25 ms.
    let send = move || {
        let Some((user, full)) = next() else { return };
        let id = lines.with(|l| format!("m{}", l.len())).into();
        let text = signal(if user { full.to_owned() } else { String::new() });
        lines.update(|l| l.push(Line { id, user, text }));
        if !user {
            spawn(async move {
                let words: Vec<&str> = full.split(' ').collect();
                for n in 1..=words.len() {
                    timer(std::time::Duration::from_millis(25)).await;
                    text.set(words[..n].join(" "));
                }
            });
        }
    };
    let rows = virtual_list(move || lines.with(Vec::len), move |at| {
        dynamic(move || {
            lines
                .with(|l| l.get(at.index()).cloned())
                .map(|line| transcript_row(line).into_element())
        })
    })
    .keys(move || lines.with(|l| l.iter().map(|l| l.id.clone()).collect::<Vec<_>>()));
    let card = view()
        .app_class(Some(SharedString::from(
            "flex h-[496px] w-full max-w-sm flex-col gap-0 overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-sm",
        )))
        .child(
            view()
                .app_class(Some(SharedString::from("flex flex-row items-start gap-4 border-b border-border px-6 py-4")))
                .child(
                    view()
                        .flex_1()
                        .gap_1()
                        .child(text("New Chat").app_class(Some(SharedString::from("text-base leading-none font-semibold"))))
                        .child(
                            text("How can I help you today?")
                                .app_class(Some(SharedString::from("text-sm text-muted-foreground"))),
                        ),
                )
                .child(
                    ui::icon_button(IconName::RefreshCw, "Reset conversation")
                        .outline()
                        .on_press(move || lines.update(|l| l.truncate(4))),
                ),
        )
        .child(
            view().flex_1().min_h_0().child(
                ui::message_scroller_provider().auto_scroll(true).child(
                    ui::message_scroller()
                        .child(
                            ui::message_scroller_viewport().child(
                                ui::message_scroller_content(rows)
                                    .padding(24.0)
                                    .message_ids(move || {
                                        lines.with(|l| l.iter().map(|l| l.id.clone()).collect::<Vec<_>>())
                                    })
                                    .scroll_anchors(move || {
                                        lines.with(|l| {
                                            l.iter().filter(|l| l.user).map(|l| l.id.clone()).collect::<Vec<_>>()
                                        })
                                    }),
                            ),
                        )
                        .child(ui::message_scroller_button()),
                ),
            ),
        )
        .child(
            view()
                .app_class(Some(SharedString::from(
                    "flex flex-row items-end gap-2 border-t border-border px-6 py-4",
                )))
                .child(
                    text(move || next().map_or("No messages queued. Reset the conversation.", |(_, t)| t))
                        .number_of_lines(2)
                        .app_class(Some(SharedString::from("flex-1 text-sm text-muted-foreground"))),
                )
                .child(
                    ui::icon_button(IconName::ArrowUp, "Send")
                        .size(ui::ButtonSize::IconSm)
                        .disabled(move || next().is_none())
                        .on_press(send),
                ),
        );
    view().flex_row().gap_12().child(card).child(
        column(12.0)
            .child(note(
                "Opens at its end. Send appends the script's next message: a question anchors near the top, the reply fills the space below it.",
            ))
            .child(note("Scroll up and the button brings you back to the end. Replies stream in, and auto-scroll follows one that outgrows the view.")),
    )
}

/// shadcn's `bubble-demo`, and `bubble-variants` beside it.
fn bubble() -> View {
    let demo = column(32.0)
        .child(ui::bubble().end().child(ui::bubble_content("Hey there! what's up?")))
        .child(
            ui::bubble_group()
                .child(ui::bubble().muted().child(ui::bubble_content("Hey! Want to see chat bubbles?")))
                .child(
                    ui::bubble()
                        .muted()
                        .child(ui::bubble_content(
                            "I can group messages, switch sides, and keep the whole thread easy to scan.",
                        ))
                        .child(
                            ui::bubble_reactions()
                                .accessibility_label(Some("Reaction: thumbs up".into()))
                                .child(text("👍")),
                        ),
                ),
        )
        .child(ui::bubble().end().child(ui::bubble_content("Sure. Hit me with your best demo.")))
        .child(
            ui::bubble()
                .muted()
                .child(ui::bubble_content(
                    "Yes. You are reading a demo that is demoing itself. Very meta. Very on-brand.",
                ))
                .child(
                    ui::bubble_reactions()
                        .accessibility_label(Some("Reactions: thumbs up, fire, eyes, and 2 more".into()))
                        .child(text("👍"))
                        .child(text("🔥"))
                        .child(text("👀"))
                        .child(text("+2")),
                ),
        );
    let presses = signal(0_u32);
    let variants = column(20.0)
        .child(ui::bubble().child(ui::bubble_content("This is the default primary bubble.")))
        .child(ui::bubble().secondary().end().child(ui::bubble_content("This is the secondary variant.")))
        .child(ui::bubble().muted().child(ui::bubble_content("This one is muted.")))
        .child(
            ui::bubble()
                .tinted()
                .end()
                .child(ui::bubble_content("This one is tinted, from the primary color.")),
        )
        .child(ui::bubble().outline().child(ui::bubble_content("We can also use an outlined variant.")))
        .child(
            ui::bubble()
                .destructive()
                .end()
                .child(ui::bubble_content("Or a destructive variant with a reaction."))
                .child(
                    ui::bubble_reactions()
                        .accessibility_label(Some("Reaction: fire".into()))
                        .child(text("🔥")),
                ),
        )
        .child(ui::bubble().ghost().child(ui::bubble_content(
            "Ghost bubbles work for assistant text that should not be framed, and take the full width.",
        )))
        .child(
            ui::bubble().tinted().end().child(
                ui::bubble_content("I forgot my password")
                    .pressable(true)
                    .on_press(move || presses.update(|n| *n += 1)),
            ),
        )
        .child(note("").child(text(move || format!("Quick replies: {}", presses.get()))));
    columns(demo, variants)
}

/// shadcn's `attachment-states`, and `attachment-demo` beside it.
fn attachment() -> View {
    let state = |state: ui::AttachmentState, icon: IconName, title: &'static str, description: &'static str| {
        ui::attachment()
            .state(state)
            .app_class(Some(SharedString::from("w-full")))
            .child(ui::attachment_media().icon(Some(icon)))
            .child(
                ui::attachment_content()
                    .child(ui::attachment_title(title))
                    .child(ui::attachment_description(description)),
            )
    };
    let remove = |name: &'static str| ui::attachment_action(IconName::X, format!("Remove {name}"));
    let states = column(8.0)
        .child(
            state(ui::AttachmentState::Idle, IconName::Clock, "selected-file.pdf", "Ready to upload")
                .child(ui::attachment_actions().child(remove("selected-file.pdf"))),
        )
        .child(
            state(ui::AttachmentState::Uploading, IconName::LoaderCircle, "design-system.zip", "Uploading · 64%")
                .child(ui::attachment_actions().child(ui::attachment_action(IconName::X, "Cancel upload"))),
        )
        .child(
            state(ui::AttachmentState::Processing, IconName::FileText, "market-research.pdf", "Processing document")
                .child(ui::attachment_actions().child(remove("market-research.pdf"))),
        )
        .child(
            state(ui::AttachmentState::Error, IconName::FileWarning, "financial-model.xlsx", "Upload failed. Try again.")
                .child(
                    ui::attachment_actions()
                        .child(ui::attachment_action(IconName::RefreshCw, "Retry upload"))
                        .child(remove("financial-model.xlsx")),
                ),
        )
        .child(
            state(ui::AttachmentState::Done, IconName::Check, "uploaded-report.pdf", "Uploaded · 1.8 MB")
                .child(ui::attachment_actions().child(remove("uploaded-report.pdf"))),
        );
    let tile = |name: &'static str, meta: &'static str| {
        ui::attachment()
            .vertical()
            .child(
                ui::attachment_media()
                    .variant(ui::AttachmentMediaVariant::Image)
                    .src(Some(PHOTO.into()))
                    .alt(Some(name.into())),
            )
            .child(
                ui::attachment_content()
                    .child(ui::attachment_title(name))
                    .child(ui::attachment_description(meta)),
            )
    };
    let demo = column(12.0)
        .child(
            ui::attachment_group()
                .child(tile("workspace.png", "PNG · 820 KB"))
                .child(tile("desk-reference.jpg", "JPG · 1.1 MB"))
                .child(tile("office-reference.jpg", "JPG · 940 KB")),
        )
        .child(
            ui::attachment()
                .app_class(Some(SharedString::from("w-full")))
                .child(ui::attachment_media().icon(Some(IconName::FileCode)))
                .child(
                    ui::attachment_content()
                        .child(ui::attachment_title("message-renderer.tsx"))
                        .child(ui::attachment_description("TypeScript · 12 KB")),
                )
                .child(ui::attachment_actions().child(remove("message-renderer.tsx"))),
        )
        .child(
            ui::attachment()
                .sm()
                .app_class(Some(SharedString::from("w-full")))
                .child(ui::attachment_media().icon(Some(IconName::FileText)))
                .child(
                    ui::attachment_content()
                        .child(ui::attachment_title("Small attachment"))
                        .child(ui::attachment_description("PDF · 2.4 MB")),
                ),
        )
        .child(
            ui::attachment()
                .xs()
                .app_class(Some(SharedString::from("w-full")))
                .child(ui::attachment_media().icon(Some(IconName::FileText)))
                .child(ui::attachment_content().child(ui::attachment_title("Extra small attachment"))),
        );
    columns(states, demo)
}

/// shadcn's `marker-demo` and `marker-variants`, and `marker-border`,
/// `marker-link-button` and `marker-separator` beside them.
fn marker() -> View {
    let presses = signal(0_u32);
    let demo = column(32.0)
        .child(
            ui::marker()
                .child(ui::marker_icon(IconName::GitBranch))
                .child(ui::marker_content("Switched to a new branch")),
        )
        .child(
            ui::marker()
                .child(ui::marker_icon(IconName::LoaderCircle))
                .child(ui::marker_content("Thinking...")),
        )
        .child(ui::marker().separator().child(ui::marker_content("Conversation compacted")))
        .child(
            ui::marker()
                .child(ui::marker_icon(IconName::Search))
                .child(ui::marker_content("Explored 4 files")),
        )
        .child(ui::marker().child(ui::marker_content("A default marker for inline notes.")))
        .child(ui::marker().border().child(ui::marker_content("A border marker for row boundaries.")));
    let right = column(12.0)
        .child(
            ui::marker()
                .border()
                .child(ui::marker_icon(IconName::GitBranch))
                .child(ui::marker_content("Switched to release-candidate")),
        )
        .child(
            ui::marker()
                .border()
                .child(ui::marker_icon(IconName::Search))
                .child(ui::marker_content("Reviewed 8 related files")),
        )
        .child(
            ui::marker()
                .border()
                .child(ui::marker_icon(IconName::FileText))
                .child(ui::marker_content("Opened implementation notes")),
        )
        .child(view().h(px(20.0)))
        .child(
            ui::marker()
                .link(true)
                .on_press(move || presses.update(|n| *n += 1))
                .child(ui::marker_icon(IconName::GitBranch))
                .child(ui::marker_content("View the pull request")),
        )
        .child(view().h(px(20.0)))
        .child(
            ui::marker()
                .pressable(true)
                .app_class(Some(SharedString::from("transition-colors hover:text-foreground")))
                .on_press(move || presses.update(|n| *n += 1))
                .child(ui::marker_icon(IconName::RotateCcw))
                .child(ui::marker_content("Revert this change")),
        )
        .child(view().h(px(20.0)))
        .child(ui::marker().separator().child(ui::marker_content("Today")))
        .child(view().h(px(20.0)))
        .child(note("").child(text(move || format!("Pressed: {}", presses.get()))));
    columns(demo, right)
}

/// A short exchange, the same on both sides of the Direction page.
fn exchange(user: &'static str, reply: &'static str, marker: &'static str) -> View {
    column(24.0)
        .child(
            ui::message()
                .end()
                .child(ui::message_avatar().child(avatar("ME")))
                .child(ui::message_content().child(ui::bubble().child(ui::bubble_content(user)))),
        )
        .child(
            ui::message()
                .child(ui::message_avatar().child(avatar("AI")))
                .child(
                    ui::message_content()
                        .child(ui::bubble().muted().child(ui::bubble_content(reply)))
                        .child(ui::message_footer().text(Some("12:04".into()))),
                ),
        )
        .child(
            ui::marker()
                .child(ui::marker_icon(IconName::GitBranch))
                .child(ui::marker_content(marker)),
        )
}

/// shadcn's `DirectionProvider`: the same exchange left to right, and
/// right to left.
fn direction() -> View {
    columns(
        view().child(ui::direction_provider(ui::Direction::Ltr).child(exchange(
            "Can you summarize the release notes?",
            "Sure. Three fixes and one new feature.",
            "Switched to a new branch",
        ))),
        view().child(ui::direction_provider(ui::Direction::Rtl).child(exchange(
            "هل يمكنك تلخيص ملاحظات الإصدار؟",
            "بالتأكيد. ثلاثة إصلاحات وميزة جديدة.",
            "تم التبديل إلى فرع جديد",
        ))),
    )
}
