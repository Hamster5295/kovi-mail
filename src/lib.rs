mod config;

use std::{collections::HashMap, sync::Arc};

use async_imap::{Session, types::Fetch};
use async_native_tls::TlsStream;
use futures::TryStreamExt;
use kovi::{
    PluginBuilder as plugin, RuntimeBot,
    chrono::{DateTime, FixedOffset, Utc},
    log::{info, warn},
    tokio::{net::TcpStream, sync::RwLock, time},
};

use crate::config::MailConfig;

type MailSession = Session<TlsStream<TcpStream>>;
type MailSessions = HashMap<String, Arc<RwLock<MailSession>>>;

struct State {
    date: DateTime<FixedOffset>,
}

#[derive(Debug)]
struct MailInfo {
    subject: String,
    date: DateTime<FixedOffset>,
}

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let config = config::init(bot.get_data_path()).await.unwrap();

    let sessions: Arc<RwLock<MailSessions>> = Arc::new(RwLock::new(MailSessions::new()));

    info!("[mail] Connecting to mail servers.");

    for cfg in config.mails {
        let state = State {
            date: Utc::now().fixed_offset(),
        };
        let state = Arc::new(RwLock::new(state));

        info!("[mail] {} initialized.", &cfg.email);

        plugin::cron(&format!("0 0/{} * * * ?", config.interval), {
            let bot = bot.clone();
            let state = state.clone();
            let sessions = sessions.clone();
            move || check_mails(cfg.clone(), bot.clone(), sessions.clone(), state.clone())
        })
        .unwrap();
    }

    plugin::drop({
        let sessions = sessions.clone();
        move || on_drop(sessions.clone())
    });

    info!("[mail] Ready to put eyes on mails!")
}

async fn pull_mails(session: &Arc<RwLock<MailSession>>) -> Result<MailInfo, String> {
    let mut session = session.write().await;
    let mails = session
        .fetch("*", "ALL")
        .await
        .map_err(|e| format!("Error when pulling mails: {}", e))?;
    let messages: Vec<Fetch> = mails
        .try_collect()
        .await
        .map_err(|e| format!("Error when pulling mails: {}", e))?;
    let fetch = messages.last().ok_or("No available mail found!")?;
    Ok(MailInfo {
        subject: {
            let sub_bytes = &fetch
                .envelope()
                .ok_or("No envelop found for the latest mail!")?
                .subject
                .clone()
                .ok_or("No subject found for the latest mail!")?;
            encoded_words::decode(
                str::from_utf8(&sub_bytes)
                    .map_err(|e| format!("Invalid UTF-8 Encoding of Mail Subject: {}", e))?,
            )
            .map_err(|e| format!("Error when decoding subject: {}", e))?
            .decoded
        },
        date: fetch
            .internal_date()
            .ok_or("No date found for the latest mail!")?,
    })
}

async fn check_mails(
    cfg: MailConfig,
    bot: Arc<RuntimeBot>,
    sessions: Arc<RwLock<MailSessions>>,
    state: Arc<RwLock<State>>,
) {
    info!("[mail] Checking mails...");

    let session = match time::timeout(time::Duration::from_secs(10), cfg.build_session()).await {
        Ok(session) => session,
        Err(e) => {
            warn!("[mail] Timeout when connecting to mail server: {e}.");
            return;
        }
    };

    if let Err(e) = session {
        warn!("[mail] Failed to connect to mail server: {e}.");
        return;
    }

    let session = Arc::new(RwLock::new(session.unwrap()));
    sessions
        .write()
        .await
        .insert(cfg.email.clone(), session.clone());

    info!("[mail] Connected to {}.", &cfg.email);

    let mail = pull_mails(&session).await;
    if mail.is_err() {
        warn!("[mail] <{}> {}", cfg.email, mail.unwrap_err());
        return;
    }

    let mail = mail.unwrap();

    let mut state = state.write().await;

    if mail.date > state.date {
        state.date = mail.date;
        info!("[mail] New mail detected!");
        let message = format!("{} 收到新邮件！\n{}", &cfg.email, mail.subject);
        if let Some(users) = &cfg.notify_users {
            for user in users {
                bot.send_private_msg(user.to_owned(), message.clone());
            }
        }
        if let Some(groups) = &cfg.notify_groups {
            for group in groups {
                bot.send_private_msg(group.to_owned(), message.clone());
            }
        }
    } else {
        info!("[mail] No new mail detected.");
    }

    if let Err(e) = session.write().await.logout().await {
        warn!("[mail] Error when logging out: {e}.");
    } else {
        info!("[mail] Logged out from {}.", &cfg.email);
    }
    sessions.write().await.remove(&cfg.email);
}

async fn on_drop(sessions: Arc<RwLock<MailSessions>>) {
    let mut sessions = sessions.write().await;
    for (_, s) in sessions.iter() {
        let mut session = s.write().await;
        session.logout().await.unwrap();
    }
    sessions.clear();
    info!("[mail] Logged out mail sessions");
}
