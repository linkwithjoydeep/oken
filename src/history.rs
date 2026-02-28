use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config;

fn db_path() -> Result<PathBuf> {
    Ok(config::data_dir()?.join("history.db"))
}

fn open_db() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(&path)
        .with_context(|| format!("failed to open history db: {}", path.display()))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS connections (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            host_alias   TEXT NOT NULL,
            hostname     TEXT,
            user         TEXT,
            port         INTEGER,
            connected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );
        CREATE INDEX IF NOT EXISTS idx_connections_host_alias ON connections (host_alias);",
    )?;
    Ok(conn)
}

pub fn record_connection(alias: &str, hostname: Option<&str>, user: Option<&str>, port: Option<u16>) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO connections (host_alias, hostname, user, port) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![alias, hostname, user, port],
    )?;
    Ok(())
}

#[derive(Debug)]
pub struct RecentHost {
    pub alias: String,
    pub last_connected: String,
}

pub fn last_connected_hosts() -> Result<Vec<RecentHost>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT host_alias, MAX(connected_at) as last_connected
         FROM connections
         GROUP BY host_alias
         ORDER BY last_connected DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RecentHost {
            alias: row.get(0)?,
            last_connected: row.get(1)?,
        })
    })?;
    let mut hosts = Vec::new();
    for row in rows {
        hosts.push(row?);
    }
    Ok(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS connections (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                host_alias   TEXT NOT NULL,
                hostname     TEXT,
                user         TEXT,
                port         INTEGER,
                connected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO connections (host_alias, hostname, user, port) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["prod-web", "10.0.1.50", "deploy", 22],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO connections (host_alias, hostname, user, port) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["staging", "10.0.2.10", Option::<String>::None, Option::<u16>::None],
        )
        .unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT host_alias, MAX(connected_at) as last_connected
                 FROM connections GROUP BY host_alias ORDER BY last_connected DESC",
            )
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(rows.len(), 2);
    }
}
