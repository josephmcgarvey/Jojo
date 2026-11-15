//! Passwords of remote locations that sign in with a password.
//!
//! Each password is kept in the system's credential store, Windows
//! Credential Manager or the macOS Keychain, under the service "Jojo SSH"
//! and the location's id (machines.toml). machines.toml records only that
//! the location uses a password (`password = true`), never the password.

const SERVICE: &str = "Jojo SSH";

fn entry(machine_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, machine_id).map_err(|e| format!("the system credential store is unavailable: {e}"))
}

/// Saves the password of `machine_id`, replacing an earlier one.
pub fn store(machine_id: &str, password: &str) -> Result<(), String> {
    entry(machine_id)?.set_password(password).map_err(|e| format!("could not save the password: {e}"))
}

/// The saved password of `machine_id`, or None when there is none or the
/// store cannot be read.
pub fn load(machine_id: &str) -> Option<String> {
    entry(machine_id).ok()?.get_password().ok()
}

/// Removes the saved password of `machine_id`; nothing happens when there is none.
pub fn delete(machine_id: &str) {
    if let Ok(entry) = entry(machine_id) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    /// Writes, reads back and deletes a throwaway entry in the real
    /// credential store. Ignored by default so the suite never touches the
    /// store; run with `cargo test -p jojo-app --lib credentials -- --ignored`.
    #[test]
    #[ignore]
    fn round_trips_through_the_system_store() {
        let id = "jojo-test-throwaway";
        super::store(id, "not-a-real-password").unwrap();
        assert_eq!(super::load(id).as_deref(), Some("not-a-real-password"));
        // Checked from outside the process, so an in-memory fallback store
        // (keyring without its platform feature) cannot pass.
        #[cfg(windows)]
        {
            let listed = std::process::Command::new("cmdkey").arg("/list").output().unwrap();
            assert!(String::from_utf8_lossy(&listed.stdout).contains(id), "not in Windows Credential Manager");
        }
        super::delete(id);
        assert_eq!(super::load(id), None);
    }
}
