// src/main.rs

use clap::{Command, Arg};
use env_logger::Env;
use log::{error, info};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use signal_hook::consts::signal::*;
use signal_hook::flag;

mod config;
mod discord;
mod monitor;
mod process;

/// If the main loop makes no progress for this long, the daemon is considered
/// wedged and exits so the service manager (launchd/systemd) restarts it.
const WATCHDOG_TIMEOUT_SECS: u64 = 30;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Spawns a watchdog thread.
///
/// The Discord client can wedge permanently: when Discord quits or its IPC
/// socket goes stale, the library's connection thread exits while the client
/// still reports ready, so `set_activity` blocks forever waiting for a reply.
/// The process then stays alive (so `KeepAlive` never triggers) but never
/// updates presence again. Exiting lets the service manager start a clean one.
fn spawn_watchdog(heartbeat: Arc<AtomicU64>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        let idle = now_secs().saturating_sub(heartbeat.load(Ordering::Relaxed));
        if idle >= WATCHDOG_TIMEOUT_SECS {
            error!(
                "Main loop unresponsive for {}s (Discord connection likely lost); exiting so the service manager restarts the daemon.",
                idle
            );
            std::process::exit(1);
        }
    });
}

fn main() {
    // Set up command-line interface
    let matches = Command::new("Ghostty RPC")
        .version("1.1.0")
        .about("Updates Discord Rich Presence with Ghostty terminal activity & Git status")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to custom configuration file"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(clap::ArgAction::SetTrue)
                .help("Enable debug logging level"),
        )
        .arg(
            Arg::new("once")
                .short('o')
                .long("once")
                .action(clap::ArgAction::SetTrue)
                .help("Run presence update once and exit"),
        )
        .arg(
            Arg::new("interval")
                .short('i')
                .long("interval")
                .value_name("SECONDS")
                .help("Update refresh interval in seconds (fractional allowed, e.g. 0.5)"),
        )
        .arg(
            Arg::new("status")
                .long("status")
                .action(clap::ArgAction::SetTrue)
                .help("Print current detected Ghostty terminal status and exit"),
        )
        .arg(
            Arg::new("install-hooks")
                .long("install-hooks")
                .action(clap::ArgAction::SetTrue)
                .help("Generate and print shell integration hook scripts for zsh, bash, fish"),
        )
        .get_matches();

    // Initialize logging
    let log_level = if matches.get_flag("debug") { "debug" } else { "info" };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    // Handle --install-hooks
    if matches.get_flag("install-hooks") {
        print_shell_hooks();
        return;
    }

    // Load configuration
    let config = if let Some(config_path) = matches.get_one::<String>("config") {
        config::load_config(PathBuf::from(config_path)).unwrap_or_else(|err| {
            error!("Failed to load config file: {}", err);
            std::process::exit(1);
        })
    } else {
        config::Config::load().unwrap_or_else(|err| {
            error!("Failed to load config: {}", err);
            config::Config::default()
        })
    };

    // Handle --status
    if matches.get_flag("status") {
        let state = monitor::get_terminal_state(&config);
        println!("=== Ghostty RPC Terminal Status ===");
        println!("Command:       {}", state.command);
        println!("Display CWD:   {}", state.display_cwd);
        println!("Raw CWD:       {}", state.raw_cwd);
        println!("Git Branch:    {}", state.git_branch.as_deref().unwrap_or("none"));
        println!("Git Repo:      {}", state.git_repo.as_deref().unwrap_or("none"));
        println!("Icon Key:      {}", state.small_image_key);
        println!("Is Idle:       {}", state.is_idle);
        println!("Start Time:    {}", state.start_time);
        return;
    }

    // Initialize Discord RPC client
    let mut discord_rpc = discord::DiscordRpc::new(&config);

    // Register signal handlers (SIGTERM, SIGINT, SIGHUP) for clean termination
    let term = Arc::new(AtomicBool::new(false));
    let _ = flag::register(SIGTERM, Arc::clone(&term));
    let _ = flag::register(SIGINT, Arc::clone(&term));
    let _ = flag::register(SIGHUP, Arc::clone(&term));

    // Daemon loop
    let interval_ms = matches
        .get_one::<String>("interval")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|sec| sec.is_finite() && *sec >= 0.0)
        .map(|sec| ((sec * 1000.0) as u64).max(100))
        .unwrap_or_else(|| config.general.refresh_interval_ms());

    info!("Starting Ghostty RPC daemon (refresh interval: {}ms)...", interval_ms);

    let heartbeat = Arc::new(AtomicU64::new(now_secs()));
    if !matches.get_flag("once") {
        spawn_watchdog(Arc::clone(&heartbeat));
    }

    let mut ghostty_was_running = true;
    let mut last_process_check: Option<Instant> = None;
    let process_check_interval = Duration::from_secs(1);

    loop {
        heartbeat.store(now_secs(), Ordering::Relaxed);

        if config.general.clear_when_closed {
            let check_due = match last_process_check {
                Some(last) => last.elapsed() >= process_check_interval,
                None => true,
            };

            if check_due {
                last_process_check = Some(Instant::now());
                let ghostty_running = process::is_ghostty_running();

                match process::presence_action(ghostty_running, ghostty_was_running) {
                    process::PresenceAction::Clear => {
                        log::debug!("Ghostty closed, clearing presence");
                        discord_rpc.clear_activity();
                    }
                    process::PresenceAction::Resume => {
                        log::debug!("Ghostty detected, resuming presence");
                    }
                    process::PresenceAction::None => {}
                }

                ghostty_was_running = ghostty_running;
            }
        }

        // Only update presence while Ghostty is running (or when the feature is disabled)
        if !config.general.clear_when_closed || ghostty_was_running {
            // Read current terminal activity
            let state = monitor::get_terminal_state(&config);

            // Update Discord Rich Presence
            discord_rpc.update_state(&state);

            // Log update summary
            log::debug!(
                "Updated Discord RPC -> Cmd: '{}' | CWD: '{}' | Git: {}",
                state.command,
                state.display_cwd,
                state.git_branch.as_deref().unwrap_or("none")
            );
        }

        if term.load(Ordering::Relaxed) || matches.get_flag("once") {
            if term.load(Ordering::Relaxed) {
                info!("Termination signal received, exiting Ghostty RPC.");
            }
            break;
        }

        thread::sleep(Duration::from_millis(interval_ms));
    }
}


/// Print shell integration hook scripts for Zsh, Bash, Fish, and Nushell.
fn print_shell_hooks() {
    print!("{}", r#"# Ghostty RPC Shell Integration Hooks
# Add the relevant block to your shell configuration file (~/.zshrc or ~/.bashrc).

# --- ZSH (~/.zshrc) ---
ghostty_rpc_preexec() {
  local cmd="$1"
  local cwd="$PWD"
  local ts=$(date +%s)
  printf '{"cmd":"%s","cwd":"%s","ts":%s,"running":true}' "$cmd" "$cwd" "$ts" > /tmp/ghostty_rpc_state.json 2>/dev/null
}

ghostty_rpc_precmd() {
  local cwd="$PWD"
  local ts=$(date +%s)
  printf '{"cmd":"zsh","cwd":"%s","ts":%s,"running":false}' "$cwd" "$ts" > /tmp/ghostty_rpc_state.json 2>/dev/null
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec ghostty_rpc_preexec
add-zsh-hook precmd ghostty_rpc_precmd
"#);
}