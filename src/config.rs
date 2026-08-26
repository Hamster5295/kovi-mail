use std::{path::PathBuf, sync::OnceLock, time::Duration};

use anyhow::{Result, anyhow};
use daaki_imap::{ImapConnection, TlsMode};
use kovi::{
    event::id::ID,
    log::{info, warn},
    tokio::fs,
};
use serde::Deserialize;

use crate::consts::*;

pub(crate) static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Deserialize, Clone)]
pub(crate) struct MailConfig {
    server: String,
    port: Option<u16>,
    pub(crate) email: String,
    inbox: Option<String>,
    password: String,
    pub(crate) notify_users: Option<Vec<ID>>,
    pub(crate) notify_groups: Option<Vec<ID>>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Config {
    pub(crate) interval: u64,
    pub(crate) timeout: Option<u64>,
    pub(crate) mails: Vec<MailConfig>,
}

pub(crate) async fn init(path: PathBuf) -> Result<Config> {
    let config_path = path.join(CONFIG_PATH);

    let config_txt = match fs::read_to_string(&config_path).await {
        Ok(txt) => txt,
        Err(e) => {
            warn!("[{PLUGIN_HEAD}] Failed to read config file: {e}");
            String::new()
        }
    };

    let config = toml::from_str::<Config>(&config_txt)?;
    for mail in &config.mails {
        if let Some(nus) = &mail.notify_users {
            for nu in nus {
                if let None = nu.try_as_i64() {
                    return Err(anyhow!("Invalid user id: '{nu}' should be an i64!"));
                }
            }
        }
        if let Some(nus) = &mail.notify_groups {
            for nu in nus {
                if let None = nu.try_as_i64() {
                    return Err(anyhow!("Invalid group id: '{nu}' should be an i64!"));
                }
            }
        }
    }

    Ok(CONFIG.get_or_init(|| config).clone())
}

impl MailConfig {
    pub async fn build_session(&self) -> Result<ImapConnection> {
        let timeout = Duration::from_secs(CONFIG.get().unwrap().timeout.unwrap_or(40));
        let conn = ImapConnection::connect(
            &self.server.clone(),
            self.port.unwrap_or(993),
            TlsMode::Implicit,
            timeout,
        )
        .await?;

        let ids = [
            ("name", Some("kovi-plugin-mail")),
            ("version", Some("1.1.0")),
            ("vendor", Some("hamster5295")),
            ("support-email", Some(self.email.as_str())),
        ];
        conn.id(&ids, timeout).await?;

        conn.login(&self.email, &self.password, timeout).await?;
        info!("[{PLUGIN_HEAD}] {} logged in", self.email);

        let inbox = self.inbox.clone().unwrap_or("INBOX".to_string());
        conn.select(&inbox, timeout).await?;
        info!("[{PLUGIN_HEAD}] {} selected inbox '{}'", self.email, inbox);
        Ok(conn)
    }
}
