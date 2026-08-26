mod config;
mod consts;

use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use daaki_imap::{ImapConnection, SequenceSet};
use kovi::{
    PluginBuilder as plugin, RuntimeBot,
    chrono::{DateTime, FixedOffset, Utc},
    log::{info, warn},
    tokio::sync::RwLock,
};
use kovi_onebot::*;

use config::MailConfig;
use consts::*;

use crate::config::CONFIG;

type MailSession = ImapConnection;
type MailSessions = HashMap<String, Arc<RwLock<MailSession>>>;

struct State {
    date: DateTime<FixedOffset>,
}

#[derive(Debug, Clone)]
struct MailInfo {
    subject: String,
    date: DateTime<FixedOffset>,
}

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let config = config::init(bot.get_data_path())
        .await
        .with_context(|| format!("[{PLUGIN_HEAD}] Error when parsing config"))
        .unwrap();

    let sessions: Arc<RwLock<MailSessions>> = Arc::new(RwLock::new(MailSessions::new()));

    info!("[{PLUGIN_HEAD}] Connecting to mail servers.");

    for cfg in config.mails {
        let state = State {
            date: Utc::now().fixed_offset(),
        };
        let state = Arc::new(RwLock::new(state));

        info!("[{PLUGIN_HEAD}] {} initialized.", &cfg.email);

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

    info!("[{PLUGIN_HEAD}] Ready to put eyes on mails!")
}

async fn check_mails(
    cfg: MailConfig,
    bot: Arc<RuntimeBot>,
    sessions: Arc<RwLock<MailSessions>>,
    state: Arc<RwLock<State>>,
) {
    info!("[{PLUGIN_HEAD}] Checking mails...");

    let session = match cfg.build_session().await {
        Ok(session) => session,
        Err(e) => {
            warn!("[{PLUGIN_HEAD}] Failed to connecting to mail server: {e}");
            return;
        }
    };

    let session = Arc::new(RwLock::new(session));
    sessions
        .write()
        .await
        .insert(cfg.email.clone(), session.clone());

    info!("[{PLUGIN_HEAD}] Connected to {}.", &cfg.email);

    let mails = pull_mails(&session).await;
    if mails.is_err() {
        warn!("[{PLUGIN_HEAD}] <{}> {}", cfg.email, mails.unwrap_err());
        return;
    }

    let mut state = state.write().await;
    let mails = mails.unwrap();
    let mails: Vec<MailInfo> = mails
        .iter()
        .cloned()
        .filter(|m| m.date > state.date)
        .collect();

    let subjects: Vec<String> = mails
        .iter()
        .map(|mail| {
            if mail.date > state.date {
                state.date = mail.date
            }
            mail.subject.clone()
        })
        .collect();

    let message = format!("{} 收到新邮件！\n- {}", &cfg.email, subjects.join("\n- "));
    if let Some(users) = &cfg.notify_users {
        for user in users {
            bot.send_private_msg(user.try_as_i64_or_panic(), message.clone());
        }
    }
    if let Some(groups) = &cfg.notify_groups {
        for group in groups {
            bot.send_private_msg(group.try_as_i64_or_panic(), message.clone());
        }
    }

    if let Err(e) = session.write().await.logout().await {
        warn!("[{PLUGIN_HEAD}] Error when logging out: {e}.");
    } else {
        info!("[{PLUGIN_HEAD}] Logged out from {}.", &cfg.email);
    }
    sessions.write().await.remove(&cfg.email);
}

async fn pull_mails(session: &Arc<RwLock<MailSession>>) -> Result<Vec<MailInfo>> {
    let timeout = Duration::from_secs(CONFIG.get().unwrap().timeout.unwrap_or(40));
    let session = session.write().await;

    let msgs = session
        .fetch(
            &SequenceSet::new("1:10")?,
            &[daaki_imap::types::FetchAttr::Envelope],
            timeout,
        )
        .await?;
    info!("[{PLUGIN_HEAD}] mail pulled successfully");

    Ok(msgs
        .iter()
        .filter_map(|resp| resp.envelope.clone())
        .filter(|e| e.subject.is_some() && e.date.is_some())
        .filter_map(|e| {
            if let Ok(date) = DateTime::parse_from_rfc2822(&e.date.unwrap()) {
                Some(MailInfo {
                    subject: e.subject.unwrap(),
                    date: date,
                })
            } else {
                None
            }
        })
        .collect())
}

async fn on_drop(sessions: Arc<RwLock<MailSessions>>) {
    let mut sessions = sessions.write().await;
    for (_, s) in sessions.iter() {
        let session = s.write().await;
        session.logout().await.unwrap();
    }
    sessions.clear();
    info!("[{PLUGIN_HEAD}] Logged out mail sessions");
}
