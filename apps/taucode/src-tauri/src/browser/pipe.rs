//! Windows pipe ACL restricted to the process token's user, with remote access disabled.
use std::{
    ffi::{c_void, OsStr},
    io, ptr,
};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

type Handle = *mut c_void;
#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        info: *mut c_void,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, string: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        string: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}
struct Token(Handle);
impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct LocalMemory(*mut c_void);
impl Drop for LocalMemory {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn create(name: &OsStr, first: bool) -> io::Result<NamedPipeServer> {
    // All pointers refer to owned, correctly aligned buffers kept alive through
    // CreateNamedPipeW. The OS copies the security descriptor during creation.
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            0x0008, /* TOKEN_QUERY */
            &mut token,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let token = Token(token);
        let mut needed = 0;
        GetTokenInformation(
            token.0,
            1, /* TokenUser */
            ptr::null_mut(),
            0,
            &mut needed,
        );
        if needed == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut info = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        if GetTokenInformation(token.0, 1, info.as_mut_ptr().cast(), needed, &mut needed) == 0 {
            return Err(io::Error::last_os_error());
        }
        // TOKEN_USER starts with SID_AND_ATTRIBUTES, whose first field is PSID.
        let sid = *info.as_ptr().cast::<*mut c_void>();
        let mut sid_string = ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut sid_string) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _sid_memory = LocalMemory(sid_string.cast());
        let mut len = 0;
        while *sid_string.add(len) != 0 {
            len += 1;
        }
        let sid = String::from_utf16(std::slice::from_raw_parts(sid_string, len))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        // Explicit owner and a protected DACL with exactly one user ACE.
        let sddl: Vec<u16> = format!("O:{sid}D:P(A;;GA;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let _descriptor = LocalMemory(descriptor);
        let mut attrs = SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor,
            inherit: 0,
        };
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attrs as *mut SecurityAttributes).cast(),
            )
    }
}
