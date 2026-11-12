//! Push notifications on the phone (`docs/push.md`): the device's token
//! from Apple, the notifications' actions (Allow, Deny, Reply), and what a
//! pressed one does. NSH shows local notifications and reports a press;
//! remote ones, their actions and a press that must act while the app is
//! not open are done here, on UIKit and UserNotifications directly.
//!
//! - [`install`] makes this file the notification center's delegate in
//!   place of NSH's, as the app finishes launching: an action can start
//!   the app in the background, with no screen, and is lost if nobody
//!   listens by then. It runs before `main` (a module initializer) and
//!   again when the app's root mounts, whichever comes first.
//! - A press on the notification itself opens its agent: [`next_press`].
//! - Allow, Deny and Reply go to the computer through the gateway
//!   ([`crate::daemon::push_gateway::deliver`]) on a thread of their own,
//!   from what [`remember_host`] kept: the app's root may not be there.
//! - [`register`] asks Apple for the device's token: [`next_token`].

use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock, atomic::{AtomicBool, Ordering}},
};

use objc2::{
    AnyThread, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, NSObject, Sel},
    sel,
};
use objc2_foundation::{NSArray, NSString};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::daemon::push::Act;

/// `UNNotificationPresentationOptionBadge | Sound | List | Banner`.
const PRESENTATION: usize = 1 | (1 << 1) | (1 << 3) | (1 << 4);
/// `UNNotificationActionOptionAuthenticationRequired`, `…Destructive`.
const AUTHENTICATION_REQUIRED: usize = 1;
const DESTRUCTIVE: usize = 1 << 1;

const ALLOW: &str = "SOPHON_ALLOW";
const DENY: &str = "SOPHON_DENY";
const REPLY: &str = "SOPHON_REPLY";

unsafe extern "C" {
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    fn _Block_release(block: *const c_void);
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dlopen(path: *const std::ffi::c_char, mode: i32) -> *mut c_void;
}

/// The head of an Objective-C block: enough to call it.
#[repr(C)]
struct BlockHead {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

/// A completion handler kept until its work is done, on any thread.
struct Completion(*mut c_void);
// SAFETY: a copied block; UserNotifications' completion handlers may be
// called from any thread.
unsafe impl Send for Completion {}

impl Completion {
    /// SAFETY: `block` is a live block taking no arguments.
    unsafe fn copy(block: *mut c_void) -> Self {
        Self(unsafe { _Block_copy(block) })
    }

    fn call(self) {
        // SAFETY: a block `void (^)(void)`, copied in `copy`.
        unsafe {
            let invoke: extern "C" fn(*mut c_void) = std::mem::transmute((*(self.0 as *const BlockHead)).invoke);
            invoke(self.0);
            _Block_release(self.0);
        }
    }
}

/// A queue filled from the system's callbacks and read by the app's root,
/// which may come later (or never, for a background launch).
struct Queue {
    tx: mpsc::UnboundedSender<String>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<String>>>,
}

impl Queue {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx: Mutex::new(Some(rx)) }
    }

    fn take(&self) -> Option<mpsc::UnboundedReceiver<String>> {
        self.rx.lock().ok()?.take()
    }
}

static PRESSES: OnceLock<Queue> = OnceLock::new();
static TOKENS: OnceLock<Queue> = OnceLock::new();
/// The agent on screen while the app is in front: its own notifications
/// are not shown over it.
static ON_SCREEN: Mutex<Option<String>> = Mutex::new(None);
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// The presses of notifications (`agent:<id>`), those before now first.
/// Once: the app's root takes it.
pub(crate) fn presses() -> Option<mpsc::UnboundedReceiver<String>> {
    PRESSES.get_or_init(Queue::new).take()
}

/// The device's tokens from Apple (hex; empty when it gave none). Once.
pub(crate) fn tokens() -> Option<mpsc::UnboundedReceiver<String>> {
    TOKENS.get_or_init(Queue::new).take()
}

/// The agent on screen now, or none (the app behind, or another screen).
pub(crate) fn set_on_screen(agent: Option<String>) {
    if let Ok(mut on) = ON_SCREEN.lock() {
        *on = agent;
    }
}

// ------------------------------------------------------------ the delegate

define_class!(
    /// The notification center's delegate, and the observer of the launch.
    #[unsafe(super(NSObject))]
    #[name = "SophonPushDelegate"]
    struct Delegate;

    impl Delegate {
        /// The app finished launching: NSH has set its delegate; this one
        /// takes its place, before any response is delivered.
        #[unsafe(method(sophonLaunched:))]
        fn launched(&self, _notification: &AnyObject) {
            become_delegate();
        }

        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(&self, _center: &AnyObject, notification: &AnyObject, done: *mut c_void) {
            let (_, data) = describe(notification);
            let agent = data.get("agent").and_then(Value::as_str).unwrap_or_default();
            let on_screen = !agent.is_empty() && ON_SCREEN.lock().ok().is_some_and(|on| on.as_deref() == Some(agent));
            // SAFETY: a block `void (^)(UNNotificationPresentationOptions)`.
            unsafe {
                let invoke: extern "C" fn(*mut c_void, usize) = std::mem::transmute((*(done as *const BlockHead)).invoke);
                invoke(done, if on_screen { 0 } else { PRESENTATION });
            }
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(&self, _center: &AnyObject, response: &AnyObject, done: *mut c_void) {
            // SAFETY: the documented properties of a notification response.
            let (action, text, notification) = unsafe {
                let action: Retained<NSString> = msg_send![response, actionIdentifier];
                let typed: bool = msg_send![response, respondsToSelector: sel!(userText)];
                let text: Option<Retained<NSString>> = if typed { msg_send![response, userText] } else { None };
                let notification: Retained<AnyObject> = msg_send![response, notification];
                (action.to_string(), text.map(|t| t.to_string()).unwrap_or_default(), notification)
            };
            let (id, data) = describe(&notification);
            // SAFETY: the completion handler, called once when the work ends.
            let done = unsafe { Completion::copy(done) };
            let field = |k: &str| data.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            let act = match action.as_str() {
                ALLOW | DENY => Some(Act::Permission { agent: field("agent"), permission: field("permission"), allow: action == ALLOW }),
                REPLY => Some(Act::Reply { agent: field("agent"), text }),
                _ => None,
            };
            match act {
                // Sent while the app may have no screen: on its own thread,
                // and the system waits for `done`.
                Some(act) => {
                    let host = field("host");
                    std::thread::spawn(move || {
                        if let Err(e) = send_act(&host, &act) {
                            log::warn!("push action not delivered: {e}");
                        }
                        done.call();
                    });
                }
                // The notification itself: its agent opens.
                None => {
                    let agent = field("agent");
                    let press = if agent.is_empty() { id } else { format!("agent:{agent}") };
                    let _ = PRESSES.get_or_init(Queue::new).tx.send(press);
                    done.call();
                }
            }
        }
    }
);

/// A notification's request id and its `sophon` data (`{}` for one NSH
/// showed itself).
fn describe(notification: &AnyObject) -> (String, Value) {
    // SAFETY: the documented chain from a notification to its user info;
    // NSJSONSerialization turns the dictionary into text when it can.
    unsafe {
        let request: Retained<AnyObject> = msg_send![notification, request];
        let id: Retained<NSString> = msg_send![&*request, identifier];
        let content: Retained<AnyObject> = msg_send![&*request, content];
        let info: Option<Retained<AnyObject>> = msg_send![&*content, userInfo];
        let data = info
            .and_then(|info| {
                let sophon: Option<Retained<AnyObject>> = msg_send![&*info, objectForKey: &*NSString::from_str("sophon")];
                let sophon = sophon?;
                let json = AnyClass::get(c"NSJSONSerialization")?;
                let valid: bool = msg_send![json, isValidJSONObject: &*sophon];
                if !valid {
                    return None;
                }
                let bytes: Option<Retained<AnyObject>> = msg_send![json, dataWithJSONObject: &*sophon, options: 0usize, error: std::ptr::null_mut::<*mut AnyObject>()];
                let bytes = bytes?;
                let (ptr, len): (*const u8, usize) = (msg_send![&*bytes, bytes], msg_send![&*bytes, length]);
                if ptr.is_null() {
                    return None;
                }
                serde_json::from_slice(std::slice::from_raw_parts(ptr, len)).ok()
            })
            .unwrap_or_else(|| json!({}));
        (id.to_string(), data)
    }
}

fn center() -> Option<Retained<AnyObject>> {
    // SAFETY: loading a system framework; the documented way to the center.
    unsafe {
        dlopen(c"/System/Library/Frameworks/UserNotifications.framework/UserNotifications".as_ptr(), 1);
        let class = AnyClass::get(c"UNUserNotificationCenter")?;
        Some(msg_send![class, currentNotificationCenter])
    }
}

fn delegate() -> &'static Delegate {
    struct Kept(Retained<Delegate>);
    // SAFETY: the delegate has no state; its methods are called by the
    // system on its own threads.
    unsafe impl Sync for Kept {}
    unsafe impl Send for Kept {}
    static DELEGATE: OnceLock<Kept> = OnceLock::new();
    // SAFETY: a plain NSObject subclass.
    &DELEGATE.get_or_init(|| Kept(unsafe { msg_send![Delegate::alloc(), init] })).0
}

/// Takes the notification center's delegate and names the actions.
fn become_delegate() {
    let Some(center) = center() else { return };
    // SAFETY: documented properties and constructors; the delegate lives
    // for the process (the property is weak).
    unsafe {
        let _: () = msg_send![&*center, setDelegate: delegate()];
        let action = |id: &str, title: &str, options: usize| -> Option<Retained<AnyObject>> {
            let class = AnyClass::get(c"UNNotificationAction")?;
            Some(msg_send![class, actionWithIdentifier: &*NSString::from_str(id), title: &*NSString::from_str(title), options: options])
        };
        let reply = || -> Option<Retained<AnyObject>> {
            let class = AnyClass::get(c"UNTextInputNotificationAction")?;
            Some(msg_send![
                class,
                actionWithIdentifier: &*NSString::from_str(REPLY),
                title: &*NSString::from_str(word("Reply", "回复")),
                options: 0usize,
                textInputButtonTitle: &*NSString::from_str(word("Send", "发送")),
                textInputPlaceholder: &*NSString::from_str(word("Tell the agent what to do next", "告诉 Agent 接下来做什么")),
            ])
        };
        let category = |id: &str, actions: Vec<Retained<AnyObject>>| -> Option<Retained<AnyObject>> {
            let class = AnyClass::get(c"UNNotificationCategory")?;
            let none: Retained<NSArray<NSString>> = NSArray::new();
            Some(msg_send![
                class,
                categoryWithIdentifier: &*NSString::from_str(id),
                actions: &*NSArray::from_retained_slice(&actions),
                intentIdentifiers: &*none,
                options: 0usize,
            ])
        };
        // Allowing asks for the phone to be unlocked: it lets an agent act.
        let allow = action(ALLOW, word("Allow", "允许"), AUTHENTICATION_REQUIRED);
        let deny = action(DENY, word("Deny", "拒绝"), DESTRUCTIVE);
        let categories: Vec<Retained<AnyObject>> = [
            category("PERMISSION", allow.into_iter().chain(deny).collect()),
            category("FINISHED", reply().into_iter().collect()),
            category("ATTENTION", reply().into_iter().collect()),
            category("USAGE", Vec::new()),
        ]
        .into_iter()
        .flatten()
        .collect();
        let set: Retained<AnyObject> = msg_send![objc2::class!(NSSet), setWithArray: &*NSArray::from_retained_slice(&categories)];
        let _: () = msg_send![&*center, setNotificationCategories: &*set];
    }
}

/// `en`, or `zh` when the phone's first language is Chinese: the actions'
/// names are set once, before the app's own language is known.
fn word(en: &'static str, zh: &'static str) -> &'static str {
    // SAFETY: a documented class method returning an array of strings.
    let first: Option<Retained<NSString>> = unsafe {
        let langs: Retained<NSArray<NSString>> = msg_send![objc2::class!(NSLocale), preferredLanguages];
        msg_send![&*langs, firstObject]
    };
    if first.is_some_and(|l| l.to_string().starts_with("zh")) { zh } else { en }
}

/// Listens for the launch (before `main`) and, when the app already runs,
/// takes the delegate now. Safe to call again.
pub(crate) fn install() {
    // Keeps the initializer below in the binary: it is in this function's
    // object file.
    std::hint::black_box(&INIT);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    // SAFETY: a documented observer registration; `delegate()` lives for
    // the process. On the main thread or before `main`.
    unsafe {
        let center: Retained<AnyObject> = msg_send![objc2::class!(NSNotificationCenter), defaultCenter];
        let _: () = msg_send![
            &*center,
            addObserver: delegate(),
            selector: sel!(sophonLaunched:),
            name: &*NSString::from_str("UIApplicationDidFinishLaunchingNotification"),
            object: std::ptr::null::<AnyObject>(),
        ];
        // Called once the app runs (its root mounted): launching is over.
        let app: Option<Retained<AnyObject>> = AnyClass::get(c"UIApplication").and_then(|c| msg_send![c, sharedApplication]);
        if app.is_some_and(|app| {
            let delegate: Option<Retained<AnyObject>> = msg_send![&*app, delegate];
            delegate.is_some()
        }) {
            become_delegate();
        }
    }
}

extern "C" fn init() {
    install();
}

/// Runs [`install`] as the program loads, before `main`.
#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static INIT: extern "C" fn() = init;

// ------------------------------------------------------------- the token

extern "C-unwind" fn did_register(_this: &AnyObject, _cmd: Sel, _app: &AnyObject, token: &AnyObject) {
    // SAFETY: `token` is the NSData the system hands the app delegate.
    let hex = unsafe {
        let (ptr, len): (*const u8, usize) = (msg_send![token, bytes], msg_send![token, length]);
        if ptr.is_null() { String::new() } else { std::slice::from_raw_parts(ptr, len).iter().map(|b| format!("{b:02x}")).collect() }
    };
    let _ = TOKENS.get_or_init(Queue::new).tx.send(hex);
}

extern "C-unwind" fn did_fail(_this: &AnyObject, _cmd: Sel, _app: &AnyObject, error: &AnyObject) {
    // SAFETY: an NSError.
    let why: Retained<NSString> = unsafe { msg_send![error, localizedDescription] };
    log::warn!("no push token: {why}");
    let _ = TOKENS.get_or_init(Queue::new).tx.send(String::new());
}

extern "C" fn register_on_main(_: *mut c_void) {
    // SAFETY: on the main thread; the app delegate's class gains the two
    // methods UIKit calls with the token (NSH's delegate has neither).
    unsafe {
        let Some(app): Option<Retained<AnyObject>> = AnyClass::get(c"UIApplication").and_then(|c| msg_send![c, sharedApplication]) else { return };
        let delegate: Option<Retained<AnyObject>> = msg_send![&*app, delegate];
        let Some(delegate) = delegate else { return };
        let class: *const AnyClass = msg_send![&*delegate, class];
        let types = c"v@:@@".as_ptr();
        let add = |sel: Sel, imp: extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &AnyObject)| {
            objc2::ffi::class_addMethod(class.cast_mut().cast(), sel, std::mem::transmute::<*const c_void, objc2::runtime::Imp>(imp as *const c_void), types);
        };
        add(sel!(application:didRegisterForRemoteNotificationsWithDeviceToken:), did_register);
        add(sel!(application:didFailToRegisterForRemoteNotificationsWithError:), did_fail);
        let _: () = msg_send![&*app, registerForRemoteNotifications];
    }
}

/// Asks Apple for this device's token ([`tokens`] hears it). After the
/// user allowed notifications.
pub(crate) fn register() {
    // SAFETY: the main queue; the work takes no context.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, std::ptr::null_mut(), register_on_main) };
}

/// Which of Apple's servers knows this build's tokens: `development` for
/// one signed for development (its provisioning profile says so) or run
/// in the simulator, else `production` (TestFlight and the App Store ship
/// no profile).
pub(crate) fn environment() -> &'static str {
    if cfg!(target_abi = "sim") {
        return "development";
    }
    let profile = std::env::current_exe().ok().and_then(|exe| std::fs::read(exe.parent()?.join("embedded.mobileprovision")).ok());
    match profile {
        Some(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let development = text.split("<key>aps-environment</key>").nth(1).is_some_and(|rest| rest.trim_start().starts_with("<string>development"));
            if development { "development" } else { "production" }
        }
        None => "production",
    }
}

// --------------------------------------------------------- actions' hosts

fn hosts_path() -> std::path::PathBuf {
    crate::daemon::data_dir().join("push-hosts.json")
}

/// Keeps where the computer `id` takes actions: its gateway, its bridge's
/// `key` and this device's pairing `token` there. Read when an action is
/// pressed, by a process that may have nothing else.
pub(crate) fn remember_host(id: &str, gateway: &str, tls: bool, key: &[u8; 32], token: &str) {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    if id.is_empty() {
        return;
    }
    let path = hosts_path();
    let mut all: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    all[id] = json!({ "gateway": gateway, "tls": tls, "key": B64.encode(key), "token": token });
    let _ = path.parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(&path, all.to_string());
}

/// Delivers `act` to the computer `host` (blocking, at most 20 s).
fn send_act(host: &str, act: &Act) -> Result<(), String> {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let all: Value = std::fs::read_to_string(hosts_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("no computer known")?;
    let h = all.get(host).ok_or("that computer is not known here")?;
    let s = |k: &str| h.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let key: [u8; 32] = B64.decode(s("key")).ok().and_then(|k| k.try_into().ok()).ok_or("no key")?;
    let (gateway, tls, token) = (s("gateway"), h.get("tls").and_then(Value::as_bool).unwrap_or(false), s("token"));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(20), crate::daemon::push_gateway::deliver(&gateway, tls, host, &key, &token, act))
            .await
            .map_err(|_| "timed out".to_owned())?
    })
}
