// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from trycua/cua; see NOTICE for source and license.

//! SkyLight SPI bridge — Rust port of Swift's `SkyLightEventPost`.
//!
//! Two-layer story (matches Swift reference exactly):
//!
//! 1. **Post path** — `SLEventPostToPid` goes through `SLEventPostToPSN` →
//!    `CGSTickleActivityMonitor` → `SLSUpdateSystemActivityWithLocation` →
//!    `IOHIDPostEvent`. The public `CGEventPostToPid` skips the activity-monitor
//!    tickle so Chromium/Catalyst targets don't accept those events as live input.
//!
//! 2. **Authentication** (keyboard only) — on macOS 14+, WindowServer gates
//!    synthetic keyboard events on Chromium-like targets on an attached
//!    `SLSEventAuthenticationMessage`. We build one via the ObjC factory and
//!    attach it with `SLEventSetAuthenticationMessage` before posting.
//!
//! All symbols are resolved once at first use via `dlopen` + `dlsym`.
//! If anything fails to resolve the functions return `false` and callers
//! fall back to the public `CGEvent::post_to_pid`.

use libc::pid_t;
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_int, c_uint};
use std::sync::OnceLock;

// ── Function-pointer typedefs ──────────────────────────────────────────────

/// `void SLEventPostToPid(pid_t, CGEventRef)`
type PostToPidFn = unsafe extern "C" fn(pid_t, *mut c_void);

/// `void SLEventSetAuthenticationMessage(CGEventRef, id)`
type SetAuthMsgFn = unsafe extern "C" fn(*mut c_void, *mut c_void);

/// `void CGEventSetWindowLocation(CGEventRef, double x, double y)`
///
/// NOTE: CGPoint on 64-bit ARM/x86 is two f64 values packed consecutively.
/// We pass them as two separate f64 arguments which has identical ABI.
type SetWindowLocFn = unsafe extern "C" fn(*mut c_void, f64, f64);

/// `void SLEventSetIntegerValueField(CGEventRef, uint32_t field, int64_t value)`
type SetIntFieldFn = unsafe extern "C" fn(*mut c_void, u32, i64);

/// `uint32_t CGSMainConnectionID(void)`
type ConnectionIDFn = unsafe extern "C" fn() -> u32;
type FactoryMsgSendFn = unsafe extern "C" fn(
    *mut c_void, // Class (receiver)
    *mut c_void, // SEL
    *mut c_void, // SLSEventRecord*
    c_int,       // pid
    c_uint,      // version
) -> *mut c_void;

// ── Symbol resolution ──────────────────────────────────────────────────────

/// Load SkyLight once so all dlsym lookups via RTLD_DEFAULT find it.
fn ensure_skylight_loaded() {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let path = b"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight\0";
        unsafe {
            libc::dlopen(
                path.as_ptr() as *const c_char,
                libc::RTLD_LAZY | libc::RTLD_GLOBAL,
            );
        }
    });
}

/// Look up a symbol by name via RTLD_DEFAULT (after loading SkyLight).
/// Returns `None` when the symbol doesn't resolve.
fn find_sym(name: &[u8]) -> Option<*mut c_void> {
    ensure_skylight_loaded();
    let ptr = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const c_char) };
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

/// Reinterpret a raw symbol pointer as a function pointer of type `T`.
/// Safety: caller guarantees T matches the symbol's actual signature.
unsafe fn as_fn<T: Copy>(ptr: *mut c_void) -> T {
    std::mem::transmute_copy::<*mut c_void, T>(&ptr)
}

// ── Lazily-resolved handles ────────────────────────────────────────────────

fn post_to_pid_fn() -> Option<PostToPidFn> {
    static SYM: OnceLock<Option<PostToPidFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLEventPostToPid\0").map(|p| unsafe { as_fn(p) }))
}

fn set_auth_msg_fn() -> Option<SetAuthMsgFn> {
    static SYM: OnceLock<Option<SetAuthMsgFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLEventSetAuthenticationMessage\0").map(|p| unsafe { as_fn(p) }))
}

fn set_window_loc_fn() -> Option<SetWindowLocFn> {
    static SYM: OnceLock<Option<SetWindowLocFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"CGEventSetWindowLocation\0").map(|p| unsafe { as_fn(p) }))
}

fn set_int_field_fn() -> Option<SetIntFieldFn> {
    static SYM: OnceLock<Option<SetIntFieldFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLEventSetIntegerValueField\0").map(|p| unsafe { as_fn(p) }))
}

fn connection_id_fn() -> Option<ConnectionIDFn> {
    static SYM: OnceLock<Option<ConnectionIDFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"CGSMainConnectionID\0").map(|p| unsafe { as_fn(p) }))
}

fn factory_msg_send_fn() -> Option<FactoryMsgSendFn> {
    static SYM: OnceLock<Option<FactoryMsgSendFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"objc_msgSend\0").map(|p| unsafe { as_fn(p) }))
}

fn objc_class(name: &CStr) -> *mut c_void {
    type GetClassFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    static SYM: OnceLock<Option<GetClassFn>> = OnceLock::new();
    let f = *SYM.get_or_init(|| find_sym(b"objc_getClass\0").map(|p| unsafe { as_fn(p) }));
    match f {
        Some(f) => unsafe { f(name.as_ptr()) },
        None => std::ptr::null_mut(),
    }
}

/// Register / look up an ObjC selector by C-string name via `sel_registerName`.
fn sel_register(name: &CStr) -> *mut c_void {
    type SelRegFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    static SYM: OnceLock<Option<SelRegFn>> = OnceLock::new();
    let f = *SYM.get_or_init(|| find_sym(b"sel_registerName\0").map(|p| unsafe { as_fn(p) }));
    match f {
        Some(f) => unsafe { f(name.as_ptr()) },
        None => std::ptr::null_mut(),
    }
}

/// Whether `cls` actually implements `sel`, via `class_respondsToSelector`.
///
/// macOS 14 (Sonoma) compatibility guard: `SLSEventAuthenticationMessage`
/// exists on macOS 14, but `messageWithEventRecord:pid:version:` was only
/// added in macOS 15 (Sequoia). `sel_registerName` always succeeds (it just
/// interns the string), so a `!sel.is_null()` check is not enough — we must
/// confirm the class responds before calling `objc_msgSend`, or the runtime
/// raises `NSInvalidArgumentException: unrecognized selector`. See #1503.
fn class_responds_to_selector(cls: *mut c_void, sel: *mut c_void) -> bool {
    if cls.is_null() || sel.is_null() {
        return false;
    }
    type RespondsToFn = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
    static SYM: OnceLock<Option<RespondsToFn>> = OnceLock::new();
    let f =
        *SYM.get_or_init(|| find_sym(b"class_respondsToSelector\0").map(|p| unsafe { as_fn(p) }));
    match f {
        Some(f) => unsafe { f(cls, sel) },
        None => false,
    }
}

// ── SLSEventRecord extraction ──────────────────────────────────────────────

/// Extract the embedded `SLSEventRecord *` from a `CGEvent`.
///
/// Layout of `__CGEvent` (SkyLight ObjC type encodings):
///   `{CFRuntimeBase, uint32_t, SLSEventRecord *}`
/// On 64-bit: CFRuntimeBase=16, uint32=4, 4 bytes pad → record pointer at offset 24.
/// We probe offsets 24, 32, 16 for resilience across OS versions (same as Swift).
unsafe fn extract_event_record(event_ptr: *mut c_void) -> *mut c_void {
    for &offset in &[24usize, 32, 16] {
        let slot = (event_ptr as *const u8).add(offset).cast::<*mut c_void>();
        let p = std::ptr::read_unaligned(slot);
        if !p.is_null() {
            return p;
        }
    }
    std::ptr::null_mut()
}

// ── Public entry points ────────────────────────────────────────────────────

/// Post `event_ptr` (raw `CGEventRef`) to `pid` via `SLEventPostToPid`.
///
/// `attach_auth_message`: pass `true` for keyboard events (Chromium path),
/// `false` for mouse events (see Swift doc comment on `postToPid`).
///
/// Returns `true` when `SLEventPostToPid` resolved and the post was attempted.
/// Returns `false` when the SPI is absent — caller falls back to `CGEvent::post_to_pid`.
pub(super) fn post_to_pid(pid: pid_t, event_ptr: *mut c_void, attach_auth_message: bool) -> bool {
    let post_fn = match post_to_pid_fn() {
        Some(f) => f,
        None => return false,
    };

    if attach_auth_message {
        // Build and attach SLSEventAuthenticationMessage.
        //
        // macOS 14 (Sonoma) compatibility: the class exists on macOS 14 but
        // `messageWithEventRecord:pid:version:` was added in macOS 15. Guard
        // with `class_respondsToSelector` (a `!sel.is_null()` check is not
        // enough — `sel_registerName` interns any name); when the selector is
        // absent we skip the auth envelope and fall through to the plain
        // `SLEventPostToPid` below. Chromium-class targets may not receive the
        // event on macOS 14, but the daemon no longer crashes. See #1503.
        let cls = objc_class(c"SLSEventAuthenticationMessage");
        let sel = sel_register(c"messageWithEventRecord:pid:version:");
        let factory = factory_msg_send_fn();

        if class_responds_to_selector(cls, sel) {
            if let Some(factory_fn) = factory {
                let record = unsafe { extract_event_record(event_ptr) };
                if !record.is_null() {
                    let msg = unsafe { factory_fn(cls, sel, record, pid as c_int, 0u32) };
                    if !msg.is_null() {
                        if let Some(set_auth) = set_auth_msg_fn() {
                            unsafe { set_auth(event_ptr, msg) };
                        }
                    }
                }
            }
        }
    }

    unsafe { post_fn(pid, event_ptr) };
    true
}

/// Stamp the event's screen-space `(x, y)` point via
/// `CGEventSetWindowLocation`. Both the SkyLight and public PID routes receive
/// this screen-space value; WindowServer derives the receiving window's local
/// point during routing. Returns `true` when the SPI resolved.
pub(super) fn set_window_location(event_ptr: *mut c_void, x: f64, y: f64) -> bool {
    match set_window_loc_fn() {
        Some(f) => {
            unsafe { f(event_ptr, x, y) };
            true
        }
        None => false,
    }
}

/// Stamp `value` onto `event_ptr` at raw SkyLight field index `field` via
/// `SLEventSetIntegerValueField`. Returns `false` when SPI absent.
pub(super) fn set_integer_field(event_ptr: *mut c_void, field: u32, value: i64) -> bool {
    match set_int_field_fn() {
        Some(f) => {
            unsafe { f(event_ptr, field, value) };
            true
        }
        None => false,
    }
}

/// Return the Skylight main connection ID for the current process.
pub fn main_connection_id() -> Option<u32> {
    connection_id_fn().map(|f| unsafe { f() })
}

type StealKeyFocusReturningIdFn = unsafe extern "C" fn(u32, u32, *mut i32) -> i32;

/// `CGError SLPSReleaseKeyFocusWithID(int32_t theft_id)`
type ReleaseKeyFocusWithIdFn = unsafe extern "C" fn(i32) -> i32;

fn steal_key_focus_fn() -> Option<StealKeyFocusReturningIdFn> {
    static SYM: OnceLock<Option<StealKeyFocusReturningIdFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLPSStealKeyFocusReturningID\0").map(|p| unsafe { as_fn(p) }))
}

fn release_key_focus_fn() -> Option<ReleaseKeyFocusWithIdFn> {
    static SYM: OnceLock<Option<ReleaseKeyFocusWithIdFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLPSReleaseKeyFocusWithID\0").map(|p| unsafe { as_fn(p) }))
}

/// `OSStatus SLPSGetKeyFocusProcess(ProcessSerialNumber *psn, uint32_t *connection)`.
/// Both are out-parameters; the second must be a valid pointer.
type GetKeyFocusProcessFn = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

/// `OSStatus GetProcessPID(const ProcessSerialNumber *psn, pid_t *pid)`
type GetProcessPidFn = unsafe extern "C" fn(*const c_void, *mut pid_t) -> i32;

fn get_key_focus_process_fn() -> Option<GetKeyFocusProcessFn> {
    static SYM: OnceLock<Option<GetKeyFocusProcessFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLPSGetKeyFocusProcess\0").map(|p| unsafe { as_fn(p) }))
}

fn get_process_pid_fn() -> Option<GetProcessPidFn> {
    static SYM: OnceLock<Option<GetProcessPidFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"GetProcessPID\0").map(|p| unsafe { as_fn(p) }))
}

/// The pid of the process WindowServer gives key focus to: normally the
/// frontmost app, or a non-activating panel (Spotlight-like launchers) or a
/// menu that took it.
pub fn key_focus_pid() -> Option<pid_t> {
    let get = get_key_focus_process_fn()?;
    let to_pid = get_process_pid_fn()?;
    let mut psn = [0u8; 8];
    // Room for whatever the second out-parameter holds (a connection id).
    let mut second = [0u64; 2];
    let err = unsafe {
        get(
            psn.as_mut_ptr() as *mut c_void,
            second.as_mut_ptr() as *mut c_void,
        )
    };
    if err != 0 || psn == [0u8; 8] {
        return None;
    }
    let mut pid: pid_t = 0;
    if unsafe { to_pid(psn.as_ptr() as *const c_void, &mut pid) } != 0 || pid <= 0 {
        return None;
    }
    Some(pid)
}

/// Steal key focus for this process's own connection and return the theft id.
///
/// WindowServer numbers thefts from one session-wide counter, so the id tells
/// the caller which id the next theft will get. AppKit uses the same call,
/// with the same options, when it opens a menu in an app that is not active.
pub fn steal_key_focus() -> Option<i32> {
    let steal = steal_key_focus_fn()?;
    let cid = main_connection_id()?;
    let mut id = 0i32;
    let err = unsafe { steal(cid, 0xC80, &mut id) };
    (err == 0 && id != 0).then_some(id)
}

/// Release a key-focus theft by id, whichever connection made it.
/// Returns the CGError (0 on success; 1010 when no such theft is live).
pub fn release_key_focus(theft_id: i32) -> i32 {
    match release_key_focus_fn() {
        Some(release) => unsafe { release(theft_id) },
        None => -1,
    }
}

/// `OSStatus _SLPSGetFrontProcess(void *psn)`
/// Writes the current frontmost process's 8-byte PSN into `psn`.
type GetFrontProcessFn = unsafe extern "C" fn(*mut c_void) -> i32;
fn get_front_process_fn() -> Option<GetFrontProcessFn> {
    static SYM: OnceLock<Option<GetFrontProcessFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"_SLPSGetFrontProcess\0").map(|p| unsafe { as_fn(p) }))
}

pub fn front_process_pid() -> Option<pid_t> {
    let get_front = get_front_process_fn()?;
    let to_pid = get_process_pid_fn()?;
    let mut psn = [0u8; 8];
    if unsafe { get_front(psn.as_mut_ptr() as *mut c_void) } != 0 || psn == [0u8; 8] {
        return None;
    }
    let mut pid: pid_t = 0;
    if unsafe { to_pid(psn.as_ptr() as *const c_void, &mut pid) } != 0 || pid <= 0 {
        return None;
    }
    Some(pid)
}

/// `OSStatus SLSGetWindowOwner(uint32_t cid, uint32_t wid, uint32_t *out_cid)`
type GetWindowOwnerFn = unsafe extern "C" fn(u32, u32, *mut u32) -> i32;

/// `OSStatus SLSGetConnectionPSN(uint32_t cid, void *psn)`
type GetConnectionPSNFn = unsafe extern "C" fn(u32, *mut c_void) -> i32;
type PostEventRecordToFn = unsafe extern "C" fn(*const c_void, *const u8) -> i32;
/// `OSStatus GetProcessForPID(pid_t, void *psn)`
/// Deprecated but still resolves. Writes the target pid's 8-byte PSN.
type GetProcessForPIDFn = unsafe extern "C" fn(pid_t, *mut c_void) -> i32;
fn get_window_owner_fn() -> Option<GetWindowOwnerFn> {
    static SYM: OnceLock<Option<GetWindowOwnerFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLSGetWindowOwner\0").map(|p| unsafe { as_fn(p) }))
}

fn get_connection_psn_fn() -> Option<GetConnectionPSNFn> {
    static SYM: OnceLock<Option<GetConnectionPSNFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLSGetConnectionPSN\0").map(|p| unsafe { as_fn(p) }))
}

fn post_event_record_to_fn() -> Option<PostEventRecordToFn> {
    static SYM: OnceLock<Option<PostEventRecordToFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"SLPSPostEventRecordTo\0").map(|p| unsafe { as_fn(p) }))
}

fn get_process_for_pid_fn() -> Option<GetProcessForPIDFn> {
    static SYM: OnceLock<Option<GetProcessForPIDFn>> = OnceLock::new();
    *SYM.get_or_init(|| find_sym(b"GetProcessForPID\0").map(|p| unsafe { as_fn(p) }))
}

fn focus_record(wid: u32) -> [u8; 0xF8] {
    let mut buf = [0u8; 0xF8];
    buf[0x04] = 0xF8;
    buf[0x08] = 0x0D;
    buf[0x3C..0x40].copy_from_slice(&wid.to_le_bytes());
    buf
}
pub fn get_process_psn_for_window(window_id: u32, pid: libc::pid_t, out_psn: &mut [u8; 8]) -> bool {
    // Try modern path: CGSMainConnectionID → SLSGetWindowOwner → SLSGetConnectionPSN
    if let (Some(get_owner), Some(get_psn), Some(conn_id_fn)) = (
        get_window_owner_fn(),
        get_connection_psn_fn(),
        connection_id_fn(),
    ) {
        let main_cid = unsafe { conn_id_fn() };
        let mut owner_cid: u32 = 0;
        let ok = unsafe { get_owner(main_cid, window_id, &mut owner_cid) } == 0;
        if ok && owner_cid != 0 {
            let psn_ok = unsafe { get_psn(owner_cid, out_psn.as_mut_ptr() as *mut c_void) } == 0;
            if psn_ok {
                return true;
            }
        }
    }
    // Fallback: GetProcessForPID
    if let Some(get_pid_psn) = get_process_for_pid_fn() {
        return unsafe { get_pid_psn(pid, out_psn.as_mut_ptr() as *mut c_void) } == 0;
    }
    false
}

// Target-only half of activate_without_raise_with(..., false). Never defocuses the user's app.
pub fn prime_window(pid: i32, window: u32) -> bool {
    let Some(post) = post_event_record_to_fn() else {
        return false;
    };
    let mut psn = [0u8; 8];
    if !get_process_psn_for_window(window, pid, &mut psn) {
        return false;
    }
    let mut record = focus_record(window);
    record[0x8A] = 0x01;
    unsafe { post(psn.as_ptr().cast(), record.as_ptr()) == 0 }
}
