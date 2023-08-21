pub mod sshfs;

use self::sshfs::SshfsOptions;
use crate::traits::from_env::FromEnv;
use remote_mount::protocols::{Protocol, Unmounted};
use std::process::exit;
use tracing::error;

/// Get the protocol handler for the given protocol.
pub fn get_protocol_handler<'r>(protocol: &Protocol) -> Box<dyn Unmounted + Send + Sync> {
    let handler = match protocol {
        Protocol::Sshfs => match SshfsOptions::from_env() {
            Ok(options) => options,
            Err(e) => {
                error!("Failed to get SSHFS options from environment: {:#?}", e);
                exit(1);
            }
        }
        .create_handler_from_self(),
    };

    Box::new(handler)
}
