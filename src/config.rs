use std::path::PathBuf;

use anyhow::Result;
use async_imap::{Client, Session};
use async_native_tls::{TlsConnector, TlsStream};
use kovi::{
    log::warn,
    tokio::{fs, net::TcpStream},
};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub(crate) struct MailConfig {
    server: String,
    port: Option<u16>,
    pub(crate) email: String,
    password: String,
    inbox: Option<String>,
    pub(crate) notify_users: Option<Vec<i64>>,
    pub(crate) notify_groups: Option<Vec<i64>>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Config {
    pub(crate) interval: u64,
    pub(crate) mails: Vec<MailConfig>,
}

pub(crate) async fn init(path: PathBuf) -> Result<Config> {
    let config_path = path.join("config.toml");

    let config_txt = match fs::read_to_string(&config_path).await {
        Ok(txt) => txt,
        Err(e) => {
            warn!("[mail] Failed to read config file: {e}");
            String::new()
        }
    };

    Ok(toml::from_str::<Config>(&config_txt)?)
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
