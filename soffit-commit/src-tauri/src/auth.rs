use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rusqlite::Connection;
use uuid::Uuid;
use chrono::Utc;
use crate::models::User;

pub fn create_user(conn: &Connection, username: &str, password: &str) -> Result<User> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash password: {}", e))?
        .to_string();

    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, username, password_hash, created_at],
    )?;

    Ok(User { id, username: username.to_string(), created_at })
}

pub fn verify_user(conn: &Connection, username: &str, password: &str) -> Result<User> {
    let result = conn.query_row(
        "SELECT id, username, password_hash, created_at FROM users WHERE username = ?1",
        rusqlite::params![username],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        )),
    );

    match result {
        Ok((id, uname, hash, created_at)) => {
            let parsed_hash = PasswordHash::new(&hash)
                .map_err(|e| anyhow!("Invalid stored hash: {}", e))?;
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .map_err(|_| anyhow!("Invalid username or password"))?;
            Ok(User { id, username: uname, created_at })
        }
        Err(_) => Err(anyhow!("Invalid username or password")),
    }
}

pub fn user_exists(conn: &Connection) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users",
        [],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn get_user_by_id(conn: &Connection, id: &str) -> Result<User> {
    conn.query_row(
        "SELECT id, username, created_at FROM users WHERE id = ?1",
        rusqlite::params![id],
        |row| Ok(User {
            id: row.get(0)?,
            username: row.get(1)?,
            created_at: row.get(2)?,
        }),
    ).map_err(|e| anyhow!("User not found: {}", e))
}
