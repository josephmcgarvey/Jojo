//! jojod as ssh's password helper (SSH_ASKPASS).
//!
//! For a remote location that signs in with a password, the app runs ssh
//! with `SSH_ASKPASS` pointing at its bundled jojod, `SSH_ASKPASS_REQUIRE=force`
//! (OpenSSH 8.4 and later; Windows' OpenSSH 9.5 and macOS's both honor it),
//! `JOJO_SSH_ASKPASS=1` and the password in `JOJO_SSH_PASSWORD`. ssh then runs
//! `jojod "<prompt>"` whenever it needs an answer and reads the answer from
//! jojod's stdout. The environment is set only on that one ssh process, so
//! the password is never written to disk or passed on a command line.

/// The variable that makes jojod answer as the password helper.
pub const ASKPASS_VAR: &str = "JOJO_SSH_ASKPASS";
/// The variable that holds the password.
pub const PASSWORD_VAR: &str = "JOJO_SSH_PASSWORD";

/// The answer to one ssh prompt. A yes/no question (an unknown host key,
/// "Are you sure you want to continue connecting (yes/no/[fingerprint])?")
/// is answered "no", so the password is never given in its place; the app
/// accepts new host keys with `StrictHostKeyChecking=accept-new`, so that
/// question is not normally asked. Every other prompt (`user@host's
/// password:`, a keyboard-interactive `Password:`) gets the password.
pub fn answer(prompt: &str, password: &str) -> String {
    if prompt.contains("(yes/no") {
        "no".to_owned()
    } else {
        password.to_owned()
    }
}

/// Runs jojod as the password helper: prints the answer for the prompt in
/// the first argument and returns the process exit code (1 when no password
/// was provided, which makes ssh treat the prompt as cancelled).
pub fn run() -> i32 {
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let Ok(password) = std::env::var(PASSWORD_VAR) else { return 1 };
    println!("{}", answer(&prompt, &password));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_password_prompts_and_refuses_yes_no_questions() {
        assert_eq!(answer("deploy@192.0.2.5's password: ", "s3cret"), "s3cret");
        assert_eq!(answer("Password: ", "s3cret"), "s3cret");
        assert_eq!(
            answer("Are you sure you want to continue connecting (yes/no/[fingerprint])? ", "s3cret"),
            "no"
        );
    }
}
