//! Files dropped onto a remote session's pane.
//!
//! A path on this computer means nothing on the server, so each dropped
//! file is copied to the server first and the server's path is typed into
//! the pane instead. A file is streamed over the stdin of `ssh <host> cat >
//! file`, as `install.rs` uploads jojod, so the server needs neither scp nor
//! SFTP. Every drop gets its own folder, `~/.jojo/drops/<stamp>/`, so two
//! files with the same name never overwrite each other, and each upload
//! first deletes the drop folders older than `KEEP_DAYS` days.

use crate::ssh::{run_with_timeout, SshTarget};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The largest file copied to a server, in bytes (50 MB). The whole file is
/// read into memory and sent over one SSH connection.
pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

/// Drop folders older than this many days are deleted on the next upload.
pub const KEEP_DAYS: u32 = 7;

/// A file name that is safe inside a double-quoted POSIX shell word and in
/// a path typed into a terminal: letters, digits, `.`, `-` and `_` are kept
/// and every other character (spaces, quotes, `$`, non-ASCII) becomes `_`.
/// An empty or dot-only result becomes `file`.
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    if cleaned.chars().all(|c| c == '.') {
        "file".to_owned()
    } else {
        cleaned
    }
}

/// The names of the files in one drop, made safe (`safe_name`) and unique:
/// a second `a.png` becomes `a-2.png`.
pub fn unique_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let base = safe_name(name);
        let mut candidate = base.clone();
        let mut n = 2;
        while out.contains(&candidate) {
            candidate = match base.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{n}.{ext}"),
                _ => format!("{base}-{n}"),
            };
            n += 1;
        }
        out.push(candidate);
    }
    out
}

/// The shell script that stores stdin as `<folder>/<name>` under
/// `~/.jojo/drops` and prints the file's absolute path. It first deletes
/// drop folders older than KEEP_DAYS days (errors ignored). `folder` and
/// `name` must already be safe (`safe_name`), because they are placed inside
/// double quotes.
pub fn upload_script(folder: &str, name: &str) -> String {
    format!(
        "root=\"$HOME/.jojo/drops\"; \
         [ -d \"$root\" ] && find \"$root\" -mindepth 1 -maxdepth 1 -type d -mtime +{KEEP_DAYS} -exec rm -rf {{}} + 2>/dev/null; \
         d=\"$root/{folder}\"; mkdir -p \"$d\" && cat > \"$d/{name}\" && printf '%s\\n' \"$d/{name}\""
    )
}

/// Copies each file in `paths` to the server and returns the server's
/// paths in the same order. Fails without copying anything when a path is
/// a folder, cannot be read or is larger than MAX_FILE_BYTES; a failed
/// upload stops at that file.
pub async fn upload(target: &SshTarget, paths: &[String]) -> Result<Vec<String>, String> {
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let p = Path::new(path);
        let meta = std::fs::metadata(p).map_err(|e| format!("Cannot read {path}: {e}"))?;
        if meta.is_dir() {
            return Err(format!("{path} is a folder. Only files can be copied to {}.", target.host));
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!(
                "{path} is {} MB. Files copied to a remote session can be at most {} MB.",
                meta.len() / (1024 * 1024),
                MAX_FILE_BYTES / (1024 * 1024)
            ));
        }
        files.push(p);
    }
    let names = unique_names(files.iter().map(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("file")));
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let folder = format!("{stamp}");
    let mut remote = Vec::with_capacity(files.len());
    for (path, name) in files.iter().zip(&names) {
        let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        // 30 s plus 32 KB/s, as for the jojod upload.
        let limit = Duration::from_secs(30 + bytes.len() as u64 / (32 * 1024));
        let out = run_with_timeout(target, &upload_script(&folder, name), &bytes, limit)
            .await
            .map_err(|e| format!("Copying {} to {} failed: {e}", path.display(), target.host))?;
        let printed = out.stdout.lines().last().unwrap_or("").trim().to_owned();
        if out.code != Some(0) || printed.is_empty() {
            return Err(format!("Copying {} to {} failed: {}", path.display(), target.host, out.stderr.trim()));
        }
        remote.push(printed);
    }
    Ok(remote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_only_safe_characters() {
        assert_eq!(safe_name("Screenshot 2026-11-25 at 10.42.png"), "Screenshot_2026-11-25_at_10.42.png");
        assert_eq!(safe_name("a\"$(rm).txt"), "a___rm_.txt");
        assert_eq!(safe_name("café.jpg"), "caf_.jpg");
        assert_eq!(safe_name(".."), "file");
        assert_eq!(safe_name(""), "file");
    }

    #[test]
    fn repeated_names_get_a_number_before_the_extension() {
        assert_eq!(unique_names(["a.png", "a.png", "a.png", "notes"]), ["a.png", "a-2.png", "a-3.png", "notes"]);
        assert_eq!(unique_names(["Makefile", "Makefile"]), ["Makefile", "Makefile-2"]);
        assert_eq!(unique_names([".env", ".env"]), [".env", ".env-2"]);
    }

    #[test]
    fn upload_script_cleans_old_drops_then_stores_stdin_and_prints_the_path() {
        let script = upload_script("1764000000000", "shot.png");
        assert!(script.contains("-mtime +7 -exec rm -rf {} +"), "{script}");
        assert!(script.ends_with("d=\"$root/1764000000000\"; mkdir -p \"$d\" && cat > \"$d/shot.png\" && printf '%s\\n' \"$d/shot.png\""), "{script}");
    }
}
