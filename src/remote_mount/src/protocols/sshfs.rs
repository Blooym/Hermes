use async_trait::async_trait;
use dotenv::dotenv;
use std::path::Path;
use tokio::process;

use super::{
    errors::{MountError, ProtocolError, UnmountError},
    FromEnv, ProtocolHandler,
};

/// The sshfs binary to use.
const SSHFS_BIN: &str = "sshfs";

/// The umount binary to use.
const UMOUNT_BIN: &str = "fusermount";

#[derive(Debug)]
pub struct Sshfs {
    /// Whether the remote filesystem is mounted.
    mounted: bool,

    /// The mountpoint of the remote filesystem.
    mountpoint: String,

    /// The connection string for sshfs.
    connection_string: String,

    /// Options to pass to sshfs.
    options: String,

    /// The password for the sshfs connection.
    password: String,

    /// Additional arguments to pass to sshfs.
    extra_args: String,
}

#[async_trait]
impl ProtocolHandler<'_> for Sshfs {
    fn is_mounted(&self) -> bool {
        self.mounted
    }

    async fn mount(&'_ mut self) -> Result<String, MountError> {
        if self.is_mounted() {
            return Err(MountError::AlreadyMounted);
        }

        // Check if the mountpoint exists.
        if !Path::new(&self.mountpoint).exists() {
            return Err(MountError::MountFailed(format!(
                "Path {} does not exist",
                &self.mountpoint
            )));
        }

        // Create the options string.
        let options_str = if self.options.is_empty() {
            String::new()
        } else {
            format!("-o password_stdin,{}", self.options)
        };

        // Format the command.
        let cmd = format!(
            "echo '{}' | {} {} {} {} {}",
            self.password,
            SSHFS_BIN,
            self.connection_string,
            self.mountpoint,
            options_str,
            self.extra_args
        );

        // Spawn the process and wait for it to finish.
        let proc = process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .await;

        // Return the result.
        match proc {
            Ok(output) => {
                let stderr = String::from_utf8(output.stderr).unwrap_or_default();
                if !stderr.is_empty() {
                    return Err(MountError::MountFailed(stderr));
                }

                self.mounted = true;
                Ok(String::from_utf8(output.stdout).unwrap_or_default())
            }
            Err(e) => Err(MountError::MountFailed(e.to_string())),
        }
    }

    async fn unmount(&mut self) -> Result<String, UnmountError> {
        if !self.is_mounted() {
            return Err(UnmountError::NotMounted);
        }

        let proc = process::Command::new(UMOUNT_BIN)
            .arg("-u")
            .arg(&self.mountpoint)
            .output()
            .await;

        match proc {
            Ok(output) => {
                self.mounted = false;
                Ok(String::from_utf8(output.stdout).unwrap_or_default())
            }
            Err(e) => Err(UnmountError::UnmountFailed(e.to_string())),
        }
    }
}

const CONNECTION_STRING_ENV_VAR: &str = "HERMES_SSHFS_CONNECTION_STRING";
const PASSWORD_ENV_VAR: &str = "HERMES_SSHFS_PASSWORD";
const OPTIONS_ENV_VAR: &str = "HERMES_SSHFS_OPTIONS";
const EXTRA_ARGS_ENV_VAR: &str = "HERMES_SSHFS_EXTRA_ARGS";

impl FromEnv for Sshfs {
    fn with_mountpoint_from_env(mountpoint: String) -> Result<Self, ProtocolError> {
        dotenv().ok();

        let connection_string = std::env::var(CONNECTION_STRING_ENV_VAR).map_err(|_| {
            ProtocolError::MissingConfigurationOption(CONNECTION_STRING_ENV_VAR.to_string())
        })?;
        let password = std::env::var(PASSWORD_ENV_VAR)
            .map_err(|_| ProtocolError::MissingConfigurationOption(PASSWORD_ENV_VAR.to_string()))?;
        let options = std::env::var(OPTIONS_ENV_VAR).unwrap_or_default();
        let extra_args = std::env::var(EXTRA_ARGS_ENV_VAR).unwrap_or_default();

        Ok(Sshfs {
            mounted: false,
            mountpoint,
            connection_string,
            options,
            password,
            extra_args,
        })
    }
}