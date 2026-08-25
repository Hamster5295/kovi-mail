use std::path::PathBuf;

use anyhow::Result;
use async_imap::{Client, Session};
use async_native_tls::{TlsConnector, TlsStream};
use kovi::{
    event::id::ID,
    log::warn,
    tokio::{fs, net::TcpStream},
};
use serde::Deserialize;

use crate::consts::*;

#[derive(Deserialize, Clone)]
pub(crate) struct MailConfig {
    server: String,
    port: Option<u16>,
    pub(crate) email: String,
    password: String,
    inbox: Option<String>,
    pub(crate) notify_users: Option<Vec<ID>>,
    pub(crate) notify_groups: Option<Vec<ID>>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Config {
    pub(crate) interval: u64,
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
                    panic!("[{PLUGIN_HEAD}] Invalid user id: '{nu}' should be an i64!")
                }
            }
        }
        if let Some(nus) = &mail.notify_groups {
            for nu in nus {
                if let None = nu.try_as_i64() {
                    panic!("[{PLUGIN_HEAD}] Invalid group id: '{nu}' should be an i64!")
                }
            }
        }
    }

    Ok(config)
}

impl MailConfig {
    pub async fn build_session(&self) -> Result<Session<TlsStream<TcpStream>>> {
        let addr = (self.server.clone(), self.port.unwrap_or(993));
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls = TlsConnector::new();
        let tls_stream = tls.connect(&self.server, tcp_stream).await?;

        let mut client = Client::new(tls_stream);
        let params = [
            "name",
            &self.email,
            "version",
            "1.0.0",
            "vendor",
            "hamster5295",
            "support-email",
            &self.email,
        ];
        client
            .run_command_and_check_ok(&format!("ID (\"{}\")", params.join("\" \"")), None)
            .await?;

        let mut session = client
            .login(&self.email, &self.password)
            .await
            .map_err(|e| e.0)?;

        session
            .select(self.inbox.to_owned().unwrap_or("INBOX".to_string()))
            .await?;
        Ok(session)
    }
}
