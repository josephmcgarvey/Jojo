//! Opens links clicked in a terminal in the user's default browser.
//!
//! The URL comes from terminal output, which any program running in a
//! session controls, so it is validated before it reaches the shell: only
//! `http://` and `https://` URLs with a host are accepted, and whitespace or
//! control characters anywhere in the input reject it outright instead of
//! being trimmed or percent-encoded by the parser. Backslashes and extra
//! slashes before the host are rejected for the same reason.

use tauri::Url;

/// Longest URL accepted, in bytes. Links in terminal output are far shorter;
/// the cap keeps an arbitrarily long string from being handed to the shell.
pub const MAX_URL_LEN: usize = 4096;

/// Parses `raw` and returns it only if it is an `http` or `https` URL with a
/// host. Returns a message naming the reason otherwise.
pub fn validate_url(raw: &str) -> Result<Url, String> {
    if raw.is_empty() {
        return Err("empty URL".into());
    }
    if raw.len() > MAX_URL_LEN {
        return Err(format!("URL is longer than {MAX_URL_LEN} bytes"));
    }
    if raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("URL contains whitespace or control characters".into());
    }
    // The scheme check runs on the raw text as well as on the parsed URL:
    // the parser accepts forms such as `http:example.com` and normalizes
    // them, and only the explicit `scheme://` form is accepted here.
    let lower = raw.get(..8).unwrap_or(raw).to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("only http:// and https:// URLs can be opened".into());
    }
    // The parser treats `\` as `/` and skips extra slashes after the scheme
    // (`https:///host` parses as `https://host/`); both are rejected so that
    // the URL opened is the one written in the terminal.
    if raw.contains('\\') {
        return Err("URL contains a backslash".into());
    }
    let after_scheme = &raw[raw.find("://").map_or(0, |i| i + 3)..];
    if after_scheme.is_empty() || after_scheme.starts_with('/') {
        return Err("URL has no host".into());
    }
    let url = Url::parse(raw).map_err(|e| format!("invalid URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only http:// and https:// URLs can be opened".into());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("URL has no host".into());
    }
    Ok(url)
}

/// Validates `raw` and opens it with the default browser.
pub fn open_url(raw: &str) -> Result<(), String> {
    let url = validate_url(raw)?;
    shell_open(url.as_str())
}

#[cfg(windows)]
fn shell_open(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let _com = ComApartment::enter();
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = wide("open");
    let file = wide(url);
    // SAFETY: `verb` and `file` are NUL-terminated UTF-16 buffers that
    // outlive the call; the parameters and directory are null, which
    // ShellExecuteW accepts, and a null owner window is allowed.
    let result = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    };
    // ShellExecuteW returns a value greater than 32 on success; 32 or less
    // is an error code (documented as an HINSTANCE for compatibility).
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed with code {code}"))
    }
}

/// Initializes COM on the calling thread for as long as the value lives.
/// ShellExecuteW's documentation requires COM to be initialized before the
/// call, because the protocol handlers and shell extensions it may run use
/// COM. `open_url` runs on a blocking worker thread (see main.rs), which has
/// no COM apartment of its own.
#[cfg(windows)]
struct ComApartment {
    initialized: bool,
}

#[cfg(windows)]
impl ComApartment {
    fn enter() -> ComApartment {
        use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
        // SAFETY: the reserved pointer must be null, and the flags are the
        // combination ShellExecuteW's documentation recommends.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32) };
        // S_OK (0) and S_FALSE (1, already initialized on this thread) both
        // need a matching CoUninitialize; a failure such as
        // RPC_E_CHANGED_MODE (negative) does not, and ShellExecuteW then runs
        // in the apartment the thread already has.
        ComApartment { initialized: hr >= 0 }
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: balances the successful CoInitializeEx in `enter`; the
            // value is created and dropped inside `shell_open` on one thread.
            unsafe { windows_sys::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// `open <url>` on macOS and `xdg-open <url>` elsewhere. The URL is one
/// argument (no shell), and `open_url` has already checked that it is an
/// http or https URL, so it cannot name a local file or another scheme.
#[cfg(unix)]
fn shell_open(url: &str) -> Result<(), String> {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let status = std::process::Command::new(program).arg(url).status().map_err(|e| format!("{program} could not start: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert_eq!(validate_url("https://example.com/a?b=c#d").unwrap().as_str(), "https://example.com/a?b=c#d");
        assert_eq!(validate_url("http://localhost:1420").unwrap().as_str(), "http://localhost:1420/");
        assert!(validate_url("HTTPS://Example.com/").is_ok());
    }

    #[test]
    fn rejects_other_schemes() {
        for raw in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "ms-settings:privacy",
            "mailto:a@example.com",
            "ftp://example.com/",
            "C:\\Windows\\System32\\cmd.exe",
            "\\\\server\\share\\x.exe",
            "example.com",
            "http:example.com",
            "https:/example.com",
            "https:\\example.com",
            "https://example.com\\path",
            "https:///example.com",
        ] {
            assert!(validate_url(raw).is_err(), "accepted {raw}");
        }
    }

    #[test]
    fn rejects_whitespace_and_control_characters() {
        for raw in [
            " https://example.com",
            "https://example.com ",
            "https://exa mple.com",
            "https://example.com/\t",
            "https://example.com/\r\nx",
            "https://example.com/\u{0}",
            "https://example.com/\u{7f}",
            "https://example.com/\u{85}",
        ] {
            assert!(validate_url(raw).is_err(), "accepted {raw:?}");
        }
    }

    #[test]
    fn rejects_empty_hostless_and_oversized_urls() {
        assert!(validate_url("").is_err());
        assert!(validate_url("http://").is_err());
        assert!(validate_url("https:///path").is_err());
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
        assert!(validate_url(&long).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_worker_thread_gets_a_com_apartment_and_releases_it() {
        std::thread::spawn(|| {
            let first = ComApartment::enter();
            assert!(first.initialized, "CoInitializeEx failed on a fresh thread");
            // A second enter on the same thread returns S_FALSE, which also
            // needs its own CoUninitialize, so it also counts as initialized.
            let second = ComApartment::enter();
            assert!(second.initialized);
            drop(second);
            drop(first);
        })
        .join()
        .unwrap();
    }
}
