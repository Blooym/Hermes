#![forbid(unsafe_code)]

use axum::Router;
use clap::Parser;
use dotenv::dotenv;
use futures::stream::StreamExt;
use hermes::protocols::get_protocol_handler;
use hermes::{create_app, AppOptions};
use remote_mount::protocols::Mounted;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::net::SocketAddr;
use std::process::exit;
use std::str::FromStr;
use tracing::{error, info};

/// All supported protocols by this binary.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Protocol {
    Sshfs,
    Local,
}

impl Protocol {
    /// Convert the protocol to the remote_mount protocol.
    pub fn as_remote_mount_protocol(&self) -> remote_mount::protocols::Protocol {
        match self {
            Self::Sshfs => remote_mount::protocols::Protocol::Sshfs,
            _ => {
                error!("Protocol is not supported for remote mounting");
                exit(1);
            }
        }
    }
}

impl FromStr for Protocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sshfs" => Ok(Self::Sshfs),
            "local" => Ok(Self::Local),
            _ => Err(format!("Invalid protocol: {}", s)),
        }
    }
}

#[derive(Parser, Debug)]
#[clap(
    name = "hermes",
    author = "Blooym",
    about = "A simple & lightweight file server that can automatically handle both local and remote filesystems using a variety of protocols."
)]
struct ProgramOptions {
    /// The address and port to listen on.
    #[clap(
        short = 's',
        long = "socket-addr",
        default_value = "0.0.0.0:8080",
        env = "HERMES_SOCKET_ADDR"
    )]
    pub socket_addr: SocketAddr,

    /// The remote filesystem protocol to use.
    #[clap(short = 'p', long = "protocol", env = "HERMES_PROTOCOL")]
    pub protocol: Protocol,

    /// The directory to serve files from.
    #[clap(short = 'd', long = "serve-dir", env = "HERMES_SERVE_DIR")]
    pub serve_dir: String,
}

impl ProgramOptions {
    /// Get the program options from the environment and command line arguments.
    pub fn from_env_and_args() -> Self {
        dotenv().ok();
        Self::parse()
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let program_options: ProgramOptions = ProgramOptions::from_env_and_args();
    let app_options: AppOptions = AppOptions {
        serve_directory: program_options.serve_dir.clone(),
    };

    if program_options.protocol == Protocol::Local {
        serve_local(program_options, app_options).await;
    } else {
        serve_remote(program_options, app_options).await;
    }
}

async fn serve_local(program_options: ProgramOptions, app_options: AppOptions) {
    info!(
        "Serving files from local filesystem at {}",
        &program_options.serve_dir
    );

    let app = create_app(app_options);
    let signals = Signals::new([SIGTERM, SIGINT, SIGQUIT]).expect("Failed to register signals");
    let handle = signals.handle();
    let signals_task = tokio::spawn(handle_exit_on_signal(signals));

    start_app(app, &program_options.socket_addr).await;

    handle.close();
    signals_task.await.unwrap();
}

async fn serve_remote(program_options: ProgramOptions, app_options: AppOptions) {
    info!("Using protocol '{:#?}'", program_options.protocol);

    let mut protocol_handler =
        get_protocol_handler(&program_options.protocol.as_remote_mount_protocol());
    if let Some(missing_deps) = protocol_handler.missing_dependencies() {
        error!(
            "Unable to use protocol, the following dependencies are missing or not in $PATH: {:#?}",
            missing_deps
        );
        exit(1);
    }

    let protocol_handler = match protocol_handler.mount().await {
        Ok(protocol_handler) => {
            info!(
                "Successfully mounted filesystem at {}",
                &program_options.serve_dir
            );
            protocol_handler
        }
        Err(e) => {
            error!("Failed to mount filesystem: {:#?}", e);
            exit(1);
        }
    };

    let signals =
        Signals::new([SIGHUP, SIGTERM, SIGINT, SIGQUIT]).expect("Failed to register signals");
    let handle = signals.handle();
    let signals_task = tokio::spawn(handle_unmount_on_signal(signals, protocol_handler));

    let app = create_app(app_options);
    info!("Serving files from {}", &program_options.serve_dir);
    start_app(app, &program_options.socket_addr).await;

    handle.close();
    signals_task.await.unwrap();
}

async fn start_app(app: Router, socket_addr: &SocketAddr) {
    info!("Server listening on {}", socket_addr);
    axum::Server::bind(socket_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_exit_on_signal(mut signals: Signals) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGTERM | SIGINT | SIGQUIT | SIGHUP => exit(0),
            _ => unreachable!(),
        }
    }
}

async fn handle_unmount_on_signal(
    mut signals: Signals,
    protocol_handler: Box<dyn Mounted + Send + Sync>,
) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGTERM | SIGINT | SIGQUIT => {
                protocol_handler.unmount();
            }
            _ => unreachable!(),
        }
    }
}
