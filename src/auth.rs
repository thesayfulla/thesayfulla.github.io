//! Passwords and the user commands.
//!
//! Hashes use Django's format (`pbkdf2_sha256$<rounds>$<salt>$<hash>`), so
//! accounts made by the old Django version keep their passwords.

use std::io::Write;

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use subtle::ConstantTimeEq;

use crate::config::Config;
use crate::db::{Db, User};
use crate::text::random_string;

const ALGORITHM: &str = "pbkdf2_sha256";
const ROUNDS: u32 = 1_500_000;

fn pbkdf2(password: &str, salt: &str, rounds: u32) -> String {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt.as_bytes(), rounds, &mut out);
    BASE64.encode(out)
}

pub fn hash_password(password: &str) -> String {
    let salt = random_string(22);
    format!("{ALGORITHM}${ROUNDS}${salt}${}", pbkdf2(password, &salt, ROUNDS))
}

/// Slow on purpose (~1s): call it off the async runtime.
pub fn verify_password(password: &str, encoded: &str) -> bool {
    let mut parts = encoded.splitn(4, '$');
    let (Some(ALGORITHM), Some(rounds), Some(salt), Some(expected)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Ok(rounds) = rounds.parse() else { return false };
    pbkdf2(password, salt, rounds).as_bytes().ct_eq(expected.as_bytes()).into()
}

/// Spend the same time as a real check, so a missing username can't be told apart by timing.
pub fn waste_time(password: &str) {
    pbkdf2(password, "timing-equalizer", ROUNDS);
}

fn validate_password(password: &str, username: &str) -> Result<()> {
    const COMMON: &[&str] = &["password", "12345678", "123456789", "qwertyuiop", "iloveyou", "password1", "11111111"];
    if password.chars().count() < 8 {
        bail!("This password is too short. It must contain at least 8 characters.");
    }
    if password.chars().all(|c| c.is_ascii_digit()) {
        bail!("This password is entirely numeric.");
    }
    if COMMON.contains(&password.to_lowercase().as_str()) {
        bail!("This password is too common.");
    }
    if password.to_lowercase().contains(&username.to_lowercase()) {
        bail!("The password is too similar to the username.");
    }
    Ok(())
}

fn prompt_new_password(username: &str) -> Result<String> {
    loop {
        let first = rpassword::prompt_password("Password: ").context("can't read the password")?;
        let second = rpassword::prompt_password("Password (again): ").context("can't read the password")?;
        if first != second {
            eprintln!("Error: Your passwords didn't match.");
            continue;
        }
        match validate_password(&first, username) {
            Ok(()) => return Ok(first),
            Err(err) => eprintln!("Error: {err}"),
        }
    }
}

fn ask(question: &str) -> Result<String> {
    print!("{question}");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().to_string())
}

pub fn create_user_cli(config: &Config, username: Option<String>, email: Option<String>) -> Result<()> {
    let db = Db::open(&config.db_path)?;
    let username = match username {
        Some(name) => name,
        None => loop {
            let name = ask("Username: ")?;
            if !name.is_empty() {
                break name;
            }
        },
    };
    if User::by_username(&mut db.conn(), &username)?.is_some() {
        bail!("Username {username:?} is already taken.");
    }
    let email = match email {
        Some(email) => email,
        None => ask("Email (optional): ")?,
    };
    let password = prompt_new_password(&username)?;
    User::create(&mut db.conn(), &username, &email, &hash_password(&password))?;
    println!("User {username:?} created. Log in at /login/.");
    Ok(())
}

pub fn change_password_cli(config: &Config, username: &str) -> Result<()> {
    let db = Db::open(&config.db_path)?;
    let user = User::by_username(&mut db.conn(), username)?.with_context(|| format!("user {username:?} does not exist"))?;
    println!("Changing password for user {username:?}");
    let password = prompt_new_password(username)?;
    User::set_password(&mut db.conn(), user.id, &hash_password(&password))?;
    println!("Password changed successfully for user {username:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn django_hash_roundtrip() {
        // Low round count to keep the test fast; same format as Django's.
        let encoded = format!("{ALGORITHM}$1000$somesalt${}", pbkdf2("secret pw", "somesalt", 1000));
        assert!(verify_password("secret pw", &encoded));
        assert!(!verify_password("wrong", &encoded));
        assert!(!verify_password("secret pw", "argon2$whatever"));
    }

    #[test]
    fn matches_django() {
        // django.contrib.auth.hashers.PBKDF2PasswordHasher().encode("hunter2!", "abc", iterations=1000)
        assert!(verify_password("hunter2!", "pbkdf2_sha256$1000$abc$zUrg0C/hjLSM/WUek4qpFVpYsHqNXF6/zZt5qop4UCQ="));
    }
}
