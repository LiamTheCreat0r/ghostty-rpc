// src/discord.rs

use discord_presence::Client;
use crate::config::Config;
use crate::monitor::TerminalState;
use std::time::{Duration, Instant};

/// Even when nothing changed, re-send the activity this often so presence is
/// restored after Discord restarts or the IPC connection is re-established.
const RESEND_INTERVAL: Duration = Duration::from_secs(15);

/// Everything that is sent to Discord in a single `set_activity` call.
#[derive(Clone, PartialEq, Eq)]
struct ActivityPayload {
    details: String,
    state: String,
    large_image: String,
    small_image: String,
    small_text: String,
    start_time: u64,
}

/// Struct to manage the Discord Rich Presence connection.
pub struct DiscordRpc {
    client: Client,
    large_image: String,
    small_image: String,
    last_error_logged: bool,
    last_sent: Option<(ActivityPayload, Instant)>,
}

impl DiscordRpc {
    /// Creates a new instance of DiscordRpc.
    pub fn new(config: &Config) -> Self {
        let app_id = config.general.client_id.parse::<u64>().unwrap_or(1429846275737518222);
        let mut client = Client::new(app_id);
        let _ = client.start();


        Self {
            client,
            large_image: config.general.large_image.clone(),
            small_image: config.general.small_image.clone(),
            last_error_logged: false,
            last_sent: None,
        }
    }

    /// Updates the Discord Rich Presence with the enhanced terminal state.
    pub fn update_state(&mut self, state: &TerminalState) {
        let details = if state.is_idle {
            "Idle in terminal".to_string()
        } else if matches!(state.command.as_str(), "zsh" | "bash" | "fish" | "nu" | "sh") {
            format!("Active in {}", state.command)
        } else {
            format!("Running {}", state.command)
        };


        let presence_state = match (&state.git_branch, &state.git_repo) {
            (Some(branch), Some(repo)) => format!("{} (git: {})", repo, branch),
            (Some(branch), None) => format!("in {} (git: {})", state.display_cwd, branch),
            (None, _) => format!("in {}", state.display_cwd),
        };

        let large_image = if self.large_image.is_empty() { "ghostty".to_string() } else { self.large_image.clone() };
        let small_image = if state.small_image_key.is_empty() { self.small_image.clone() } else { state.small_image_key.clone() };

        let payload = ActivityPayload {
            details,
            state: presence_state,
            large_image,
            small_image,
            small_text: state.small_image_text.clone(),
            start_time: state.start_time,
        };

        // Skip redundant updates: Discord rate-limits SET_ACTIVITY (about 5 per
        // 20s), and spamming identical payloads delays the ones that matter.
        if let Some((last, sent_at)) = &self.last_sent {
            if *last == payload && sent_at.elapsed() < RESEND_INTERVAL {
                return;
            }
        }

        let result = self.client.set_activity(|activity| {
            activity
                .details(&payload.details)
                .state(&payload.state)
                .assets(|assets| {
                    assets
                        .large_image(&payload.large_image)
                        .large_text("Ghostty Terminal")
                        .small_image(&payload.small_image)
                        .small_text(&payload.small_text)
                })
                .timestamps(|timestamps| timestamps.start(payload.start_time))
        });

        if let Err(e) = result {
            self.last_sent = None;
            if !self.last_error_logged {
                log::info!("Waiting for Discord desktop app to start (connection error: {})...", e);
                self.last_error_logged = true;
            }
        } else {
            self.last_sent = Some((payload, Instant::now()));
            if self.last_error_logged {
                log::info!("Connected to Discord successfully!");
                self.last_error_logged = false;
            }
        }
    }

    /// Clears the current Discord Rich Presence activity.
    pub fn clear_activity(&mut self) {
        self.last_sent = None;
        if let Err(e) = self.client.clear_activity() {
            log::debug!("Failed to clear Discord activity: {}", e);
        }
    }
}