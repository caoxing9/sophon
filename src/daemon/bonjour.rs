//! Bonjour through the system's `dns_sd` (mDNSResponder), which iOS allows
//! without the multicast entitlement a raw mDNS socket needs: the Mac's
//! bridge advertises `_sophon._tcp` with its public key, and phones (or
//! another Mac) find it on the LAN by that key, whatever its address now.

use std::{
    ffi::{CStr, CString, c_char, c_void},
    time::{Duration, Instant},
};

pub const SERVICE: &str = "_sophon._tcp";

type Ref = *mut c_void;

const FLAG_ADD: u32 = 0x2;
const PROTOCOL_IPV4: u32 = 0x1;
const AF_INET: u8 = 2;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn DNSServiceRegister(
        sd: *mut Ref,
        flags: u32,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        host: *const c_char,
        port_be: u16,
        txt_len: u16,
        txt: *const c_void,
        callback: Option<extern "C" fn(Ref, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void)>,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceBrowse(
        sd: *mut Ref,
        flags: u32,
        interface: u32,
        regtype: *const c_char,
        domain: *const c_char,
        callback: extern "C" fn(Ref, u32, u32, i32, *const c_char, *const c_char, *const c_char, *mut c_void),
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceResolve(
        sd: *mut Ref,
        flags: u32,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        callback: extern "C" fn(Ref, u32, u32, i32, *const c_char, *const c_char, u16, u16, *const u8, *mut c_void),
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceGetAddrInfo(
        sd: *mut Ref,
        flags: u32,
        interface: u32,
        protocol: u32,
        hostname: *const c_char,
        callback: extern "C" fn(Ref, u32, u32, i32, *const c_char, *const u8, u32, *mut c_void),
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceRefSockFD(sd: Ref) -> i32;
    fn DNSServiceProcessResult(sd: Ref) -> i32;
    fn DNSServiceRefDeallocate(sd: Ref);
    fn poll(fds: *mut PollFd, n: u32, timeout_ms: i32) -> i32;
}

/// A `dns_sd` operation, ended when dropped.
struct Op(Ref);

impl Drop for Op {
    fn drop(&mut self) {
        unsafe { DNSServiceRefDeallocate(self.0) }
    }
}

// The reference is only handed to mDNSResponder's client calls, which may
// run on any one thread at a time.
unsafe impl Send for Op {}

impl Op {
    /// Handles results until `deadline`, or `done` says enough.
    fn pump(&self, deadline: Instant, done: impl Fn() -> bool) {
        let fd = unsafe { DNSServiceRefSockFD(self.0) };
        if fd < 0 {
            return;
        }
        while !done() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            let mut p = PollFd { fd, events: 1, revents: 0 };
            let n = unsafe { poll(&mut p, 1, left.as_millis().min(i32::MAX as u128) as i32) };
            if n < 0 {
                return;
            }
            if n > 0 && unsafe { DNSServiceProcessResult(self.0) } != 0 {
                return;
            }
        }
    }
}

/// A TXT record from `key=value` pairs.
fn txt(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in entries {
        let e = format!("{k}={v}");
        if e.len() <= 255 {
            out.push(e.len() as u8);
            out.extend_from_slice(e.as_bytes());
        }
    }
    out
}

/// A TXT record's value for `key`.
fn txt_value(record: &[u8], key: &str) -> Option<String> {
    let mut i = 0;
    while i < record.len() {
        let n = record[i] as usize;
        let entry = record.get(i + 1..i + 1 + n)?;
        if let Some(v) = std::str::from_utf8(entry).ok()?.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
            return Some(v.to_owned());
        }
        i += 1 + n;
    }
    None
}

/// A service instance name: at most 63 bytes, cut at a character.
fn instance_name(name: &str) -> String {
    let mut end = name.len().min(63);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

/// Advertises the bridge at `port` as `name` (this computer's), with its
/// public key (base64), for as long as the process runs.
///
/// The name is given rather than left to mDNSResponder: a service left
/// unnamed takes the computer's name, and a conflict on it is settled by
/// renaming the computer ("MacBook Pro (2)"). Named, only the service
/// becomes "name (2)".
pub fn advertise(name: &str, port: u16, public_key_b64: &str) -> Result<(), String> {
    let record = txt(&[("v", "1"), ("pk", public_key_b64)]);
    let regtype = CString::new(SERVICE).map_err(|e| e.to_string())?;
    let name = CString::new(instance_name(name)).map_err(|e| e.to_string())?;
    let mut sd: Ref = std::ptr::null_mut();
    let err = unsafe {
        DNSServiceRegister(
            &mut sd,
            0,
            0,
            name.as_ptr(),
            regtype.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            port.to_be(),
            record.len() as u16,
            record.as_ptr().cast(),
            None,
            std::ptr::null_mut(),
        )
    };
    if err != 0 {
        return Err(format!("Bonjour registration failed ({err})"));
    }
    // Kept for the life of the process: dropping it withdraws the service.
    std::mem::forget(Op(sd));
    Ok(())
}

/// A service seen while browsing: interface, name, type, domain.
type Seen = (u32, String, String, String);

extern "C" fn on_browse(_: Ref, flags: u32, interface: u32, err: i32, name: *const c_char, regtype: *const c_char, domain: *const c_char, ctx: *mut c_void) {
    if err != 0 || flags & FLAG_ADD == 0 {
        return;
    }
    let list = unsafe { &mut *(ctx as *mut Vec<Seen>) };
    let s = |p: *const c_char| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    list.push((interface, s(name), s(regtype), s(domain)));
}

/// A bridge found on the LAN.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The computer's name, as it advertises itself.
    pub name: String,
    /// The bridge's public key, base64.
    pub key: String,
    /// `ip:port`.
    pub addr: String,
}

/// The bridges on the LAN, looked for during `wait` (blocking).
pub fn browse(wait: Duration) -> Vec<Found> {
    let regtype = CString::new(SERVICE).expect("service");
    let mut seen: Box<Vec<Seen>> = Box::default();
    let mut sd: Ref = std::ptr::null_mut();
    let err = unsafe { DNSServiceBrowse(&mut sd, 0, 0, regtype.as_ptr(), std::ptr::null(), on_browse, (&mut *seen as *mut Vec<_>).cast()) };
    if err != 0 {
        return vec![];
    }
    let op = Op(sd);
    op.pump(Instant::now() + wait, || false);
    drop(op);

    let mut found: Vec<Found> = Vec::new();
    for (interface, name, regtype, domain) in seen.iter() {
        let Some(f) = resolve(*interface, name, regtype, domain) else { continue };
        // One computer on several interfaces is one entry, at its LAN
        // address rather than loopback or link-local.
        let weak = |a: &str| a.starts_with("127.") || a.starts_with("169.254.");
        match found.iter_mut().find(|x| x.key == f.key) {
            Some(x) if weak(&x.addr) && !weak(&f.addr) => *x = f,
            Some(_) => {}
            None => found.push(f),
        }
    }
    found
}

/// The address of the bridge whose key is `key`, as soon as it is found
/// within `wait` (blocking).
pub fn find(key: &str, wait: Duration) -> Option<String> {
    let deadline = Instant::now() + wait;
    let regtype = CString::new(SERVICE).ok()?;
    let mut seen: Box<Vec<Seen>> = Box::default();
    let mut sd: Ref = std::ptr::null_mut();
    let err = unsafe { DNSServiceBrowse(&mut sd, 0, 0, regtype.as_ptr(), std::ptr::null(), on_browse, (&mut *seen as *mut Vec<_>).cast()) };
    if err != 0 {
        return None;
    }
    let op = Op(sd);
    let mut done = 0;
    let mut weak: Option<String> = None;
    while Instant::now() < deadline {
        let ptr: *const Vec<Seen> = &*seen;
        op.pump(deadline, || unsafe { (*ptr).len() } > done);
        while done < seen.len() {
            let (interface, name, regtype, domain) = seen[done].clone();
            done += 1;
            if let Some(f) = resolve(interface, &name, &regtype, &domain).filter(|f| f.key == key) {
                if f.addr.starts_with("127.") || f.addr.starts_with("169.254.") {
                    weak = Some(f.addr);
                } else {
                    return Some(f.addr);
                }
            }
        }
    }
    weak
}

/// A found service's key and IPv4 address.
fn resolve(interface: u32, name: &str, regtype: &str, domain: &str) -> Option<Found> {
    #[derive(Default)]
    struct Resolved {
        host: Option<String>,
        port: u16,
        key: Option<String>,
    }
    extern "C" fn on_resolve(_: Ref, _: u32, _: u32, err: i32, _: *const c_char, host: *const c_char, port_be: u16, txt_len: u16, txt: *const u8, ctx: *mut c_void) {
        if err != 0 {
            return;
        }
        let r = unsafe { &mut *(ctx as *mut Resolved) };
        r.host = Some(unsafe { CStr::from_ptr(host) }.to_string_lossy().into_owned());
        r.port = u16::from_be(port_be);
        let record = unsafe { std::slice::from_raw_parts(txt, txt_len as usize) };
        r.key = txt_value(record, "pk");
    }
    extern "C" fn on_addr(_: Ref, _: u32, _: u32, err: i32, _: *const c_char, addr: *const u8, _: u32, ctx: *mut c_void) {
        if err != 0 || addr.is_null() {
            return;
        }
        // `sockaddr_in`: length, family, port, then the address.
        let raw = unsafe { std::slice::from_raw_parts(addr, 8) };
        if raw[1] == AF_INET {
            let ip = unsafe { &mut *(ctx as *mut Option<String>) };
            *ip = Some(format!("{}.{}.{}.{}", raw[4], raw[5], raw[6], raw[7]));
        }
    }

    let (n, t, d) = (CString::new(name).ok()?, CString::new(regtype).ok()?, CString::new(domain).ok()?);
    let mut r: Box<Resolved> = Box::default();
    let mut sd: Ref = std::ptr::null_mut();
    let err = unsafe { DNSServiceResolve(&mut sd, 0, interface, n.as_ptr(), t.as_ptr(), d.as_ptr(), on_resolve, (&mut *r as *mut Resolved).cast()) };
    if err != 0 {
        return None;
    }
    let op = Op(sd);
    let ptr: *const Resolved = &*r;
    op.pump(Instant::now() + Duration::from_secs(2), || unsafe { (*ptr).host.is_some() });
    drop(op);
    let host = CString::new(r.host.clone()?).ok()?;
    let key = r.key.clone()?;

    let mut ip: Box<Option<String>> = Box::default();
    let mut sd: Ref = std::ptr::null_mut();
    let err = unsafe { DNSServiceGetAddrInfo(&mut sd, 0, interface, PROTOCOL_IPV4, host.as_ptr(), on_addr, (&mut *ip as *mut Option<String>).cast()) };
    if err != 0 {
        return None;
    }
    let op = Op(sd);
    let ptr: *const Option<String> = &*ip;
    op.pump(Instant::now() + Duration::from_secs(2), || unsafe { (*ptr).is_some() });
    drop(op);
    Some(Found { name: name.to_owned(), key, addr: format!("{}:{}", ip.take()?, r.port) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_records_round_trip() {
        let r = txt(&[("v", "1"), ("pk", "abc+/=")]);
        assert_eq!(txt_value(&r, "pk").as_deref(), Some("abc+/="));
        assert_eq!(txt_value(&r, "v").as_deref(), Some("1"));
        assert_eq!(txt_value(&r, "x"), None);
        assert_eq!(txt_value(&[9, b'a'], "a"), None);
    }

    #[test]
    fn instance_name_fits_a_label() {
        assert_eq!(instance_name("Aries"), "Aries");
        assert_eq!(instance_name(&"a".repeat(70)).len(), 63);
        let cut = instance_name(&format!("{}é", "a".repeat(62)));
        assert_eq!(cut, "a".repeat(62));
    }

    /// Advertises a test service and finds it again (the local mDNSResponder).
    #[test]
    #[ignore]
    fn advertise_and_browse_live() {
        advertise("Sophon test", 65001, "dGVzdA==").unwrap();
        let found = browse(Duration::from_secs(5));
        eprintln!("{found:?}");
        assert!(found.iter().any(|f| f.key == "dGVzdA==" && f.addr.ends_with(":65001")));
        let at = find("dGVzdA==", Duration::from_secs(2));
        eprintln!("find: {at:?}");
        assert!(at.is_some_and(|a| a.ends_with(":65001")));
        assert_eq!(find("bm9uZQ==", Duration::from_millis(500)), None);
    }
}
