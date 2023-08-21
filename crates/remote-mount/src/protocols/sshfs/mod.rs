use super::{
    errors::{MountError, UnmountError},
    Mounted, Protocol, ProtocolHandler, Unmounted,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info};
use std::{marker::PhantomData, process::Output};
use tokio::process::Command;

const SSHFS_BIN: &str = "sshfs";
const UMOUNT_BIN: &str = "fusermount";
const SHELL_BIN: &str = "sh";

pub const DEPENDENCIES: &[&str] = &[SSHFS_BIN, UMOUNT_BIN, SHELL_BIN];

pub struct MountedState;
pub struct UnmountedState;

#[derive(Debug)]
pub struct Sshfs<State = UnmountedState> {
    mountpoint: String,
    connection_string: String,
    options: String,
    password: String,
    extra_args: String,
    state: PhantomData<State>,
}

impl<State> Sshfs<State> {
    pub fn new(
        mountpoint: String,
        connection_string: String,
        options: String,
        password: String,
        extra_args: String,
    ) -> Self {
        Self {
            mountpoint,
            connection_string,
            options,
            password,
            extra_args,
            state: Default::default(),
        }
    }

    async fn execute_command(&self, shell: &str, command: &str) -> Result<Output> {
        info!(
            "Executing command: {}",
            command.replace(&self.password, "**********")
        );

        let shell_location = which::which(shell)
            .context("Unable to find shell binary in path")?
            .canonicalize()?;

        let cmd = format!("{} -c '{}'", shell_location.display(), command);

        let proc = Command::new(SHELL_BIN)
            .args(["-c", &cmd])
            .output()
            .await
            .context("Process returned non-zero exit code")?;
        debug!("Command output: {:?}", proc);

        Ok(proc)
    }
}

#[async_trait]
impl Unmounted for Sshfs<UnmountedState> {
    async fn mount(self) -> Result<Box<dyn Mounted + Send + Sync>> {
        info!("Mounting filesystem at {}", self.mountpoint);

        if let Some(missing_deps) = self.missing_dependencies() {
            anyhow::bail!(
                "Unable to unmount filesystem, the following dependencies are missing or not in $PATH: {:#?}",
                missing_deps
            );
        }

        let sshfs_location = which::which(SSHFS_BIN)
            .context("Unable to find sshfs binary in path")?
            .canonicalize()?;

        let options_str = if self.options.is_empty() {
            String::new()
        } else {
            format!("-o password_stdin,{}", self.options)
        };

        let cmd = format!(
            "echo '{}' | {} {} {} {} {}",
            self.password,
            sshfs_location.display(),
            self.connection_string,
            self.mountpoint,
            options_str,
            self.extra_args
        );

        let proc = self.execute_command(SHELL_BIN, &cmd).await?;

        let stderr = String::from_utf8_lossy(&proc.stderr);
        if !stderr.is_empty() {
            anyhow::bail!(MountError::MountFailed(stderr.to_string()));
        }

        info!("Successfully mounted filesystem at {}", self.mountpoint);
        Ok(Box::new(Sshfs::<MountedState> {
            mountpoint: self.mountpoint,
            connection_string: self.connection_string,
            options: self.options,
            password: self.password,
            extra_args: self.extra_args,
            state: Default::default(),
        }))
    }
}

#[async_trait]
impl Mounted for Sshfs<MountedState> {
    async fn unmount(self) -> Result<Box<dyn Unmounted + Send + Sync>> {
        info!("Unmounting filesystem at {}", self.mountpoint);

        if let Some(missing_deps) = self.missing_dependencies() {
            anyhow::bail!(
                "Unable to unmount filesystem, the following dependencies are missing or not in $PATH: {:#?}",
                missing_deps
            );
        }

        let umount_location = which::which(UMOUNT_BIN)
            .context(UnmountError::MissingDependency(UMOUNT_BIN.to_string()))?
            .canonicalize()?;

        let proc = self
            .execute_command(
                SHELL_BIN,
                &format!("{} -u {}", umount_location.display(), self.mountpoint),
            )
            .await?;

        let stderr = String::from_utf8_lossy(&proc.stderr);
        if !stderr.is_empty() {
            anyhow::bail!(UnmountError::UnmountFailed(stderr.to_string()));
        }

        info!("Successfully unmounted filesystem at {}", self.mountpoint);
        Ok(Box::new(Sshfs::<UnmountedState> {
            mountpoint: self.mountpoint,
            connection_string: self.connection_string,
            options: self.options,
            password: self.password,
            extra_args: self.extra_args,
            state: Default::default(),
        }))
    }
}

#[async_trait]
impl<State> ProtocolHandler for Sshfs<State> {
    fn missing_dependencies(&self) -> Option<Vec<String>> {
        debug!("Checking for missing dependencies from {:?}", DEPENDENCIES);

        let missing_deps = DEPENDENCIES
            .iter()
            .filter_map(|dep| {
                if which::which(dep).is_err() {
                    Some(dep.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if missing_deps.is_empty() {
            None
        } else {
            Some(missing_deps)
        }
    }

    fn protocol(&self) -> Protocol {
        Protocol::Sshfs
    }
}
