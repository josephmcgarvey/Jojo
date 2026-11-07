//! Security attributes that grant pipe access only to the current user's
//! SID (spec section 8.1). Without them, the default named pipe DACL grants
//! read access to Everyone.

use std::ffi::c_void;
use std::io;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, TokenElevation, TokenUser, TOKEN_ELEVATION, PSECURITY_DESCRIPTOR,
    SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

pub struct UserOnly {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: SECURITY_ATTRIBUTES,
}

impl UserOnly {
    /// Boxed so the pointer returned by `as_ptr` stays valid while the box lives.
    pub fn new() -> io::Result<Box<UserOnly>> {
        let sid = current_user_sid()?;
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})").encode_utf16().chain(Some(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: sddl is NUL-terminated; descriptor receives a LocalAlloc'd
        // buffer freed in Drop.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Box::new(UserOnly {
            descriptor,
            attributes: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        }))
    }

    pub fn as_ptr(&mut self) -> *mut c_void {
        &mut self.attributes as *mut SECURITY_ATTRIBUTES as *mut c_void
    }
}

impl Drop for UserOnly {
    fn drop(&mut self) {
        // SAFETY: descriptor came from ConvertStringSecurityDescriptorToSecurityDescriptorW.
        unsafe {
            LocalFree(self.descriptor as _);
        }
    }
}

/// The TOKEN_USER structure of `token`, in u64 storage for alignment.
/// SAFETY: `token` must be a valid token handle opened with TOKEN_QUERY.
unsafe fn token_user(token: HANDLE) -> io::Result<Vec<u64>> {
    let mut len = 0u32;
    GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    if GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, len, &mut len) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

/// Opens the token of `process` for querying. The caller closes it.
/// SAFETY: `process` must be a valid process handle.
unsafe fn open_token(process: HANDLE) -> io::Result<HANDLE> {
    let mut token: HANDLE = null_mut();
    if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(token)
}

/// The current process user's SID in string form, for example `S-1-5-21-...`.
fn current_user_sid() -> io::Result<String> {
    // SAFETY: standard token query sequence; every handle and buffer is
    // released before returning.
    unsafe {
        let token = open_token(GetCurrentProcess())?;
        let user = token_user(token);
        CloseHandle(token);
        let user = user?;
        let user = &*(user.as_ptr() as *const TOKEN_USER);
        let mut wide: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut wide) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut n = 0;
        while *wide.add(n) != 0 {
            n += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(wide, n));
        LocalFree(wide as _);
        Ok(sid)
    }
}

/// True when this process runs with an elevated (administrator) token.
pub fn is_elevated() -> io::Result<bool> {
    // SAFETY: standard token query; the handle is closed on every path.
    unsafe {
        let token = open_token(GetCurrentProcess())?;
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let result = if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(elevation.TokenIsElevated != 0) };
        CloseHandle(token);
        result
    }
}

/// Fails unless the process serving `pipe` runs as the same user as this
/// process. Without this check, another local user who creates
/// `\\.\pipe\jojo-<username>` first would receive this user's keystrokes.
pub fn verify_server_is_current_user(pipe: HANDLE) -> io::Result<()> {
    // SAFETY: every handle opened here is closed before returning; SID
    // pointers point into buffers that live until EqualSid returns.
    unsafe {
        let mut pid = 0u32;
        if GetNamedPipeServerProcessId(pipe, &mut pid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let server_token = open_token(process);
        CloseHandle(process);
        let server_token = server_token?;
        let server_user = token_user(server_token);
        CloseHandle(server_token);
        let server_user = server_user?;

        let own_token = open_token(GetCurrentProcess())?;
        let own_user = token_user(own_token);
        CloseHandle(own_token);
        let own_user = own_user?;

        let server_sid = (*(server_user.as_ptr() as *const TOKEN_USER)).User.Sid;
        let own_sid = (*(own_user.as_ptr() as *const TOKEN_USER)).User.Sid;
        if EqualSid(server_sid, own_sid) == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the jojod pipe is served by a process of another user",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;

    #[test]
    fn elevation_can_be_queried() {
        is_elevated().expect("query token elevation");
    }

    #[tokio::test]
    async fn a_pipe_created_by_this_process_passes_the_owner_check() {
        let name = format!(r"\\.\pipe\jojo-owner-test-{}", std::process::id());
        let mut security = UserOnly::new().unwrap();
        let mut options = tokio::net::windows::named_pipe::ServerOptions::new();
        options.first_pipe_instance(true);
        // SAFETY: the security attributes outlive the create call.
        let _server = unsafe { options.create_with_security_attributes_raw(&name, security.as_ptr()) }.unwrap();
        let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&name).unwrap();
        verify_server_is_current_user(client.as_raw_handle() as HANDLE).expect("owner check");
    }
}
