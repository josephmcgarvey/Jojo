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

#[cfg(not(windows))]
fn shell_open(_url: &str) -> Result<(), String> {
    Err("opening links is only implemented on Windows".into())
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
}
