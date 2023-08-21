/// Module containing error types used by protocol handlers.
pub mod errors;

/// Module containing the SSHFS protocol handler implementation.
#[cfg(feature = "protocols-sshfs")]
pub mod sshfs;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Unmounted: ProtocolHandler + Send + Sync {
    /// Mounts the filesystem using the specified protocol.
    ///
    /// This method is responsible for performing the necessary steps to mount the filesystem according to the protocol's requirements.
    async fn mount(self) -> Result<Box<dyn Mounted + Send + Sync>>;
}

#[async_trait]
pub trait Mounted: ProtocolHandler + Send + Sync {
    /// Unmounts the mounted filesystem.
    ///
    /// This method is responsible for performing the necessary steps to unmount the filesystem.
    async fn unmount(self) -> Result<Box<dyn Unmounted + Send + Sync>>;
}

/// `ProtocolHandler` is a trait that defines the common interface for various filesystem protocol handlers.
#[async_trait]
// this trait should either have methods from Mounted or Unmounted, but never both
pub trait ProtocolHandler {
    /// Checks for missing dependencies required for the protocol handler to work.
    ///
    /// Returns an option containing a vector of missing dependency names if any dependencies are missing,
    /// or None if all dependencies are available.
    fn missing_dependencies(&self) -> Option<Vec<String>>;

    /// Returns the protocol associated with this protocol handler.
    ///
    /// Returns one of the enum values defined in the `Protocol` enum.
    fn protocol(&self) -> Protocol;
}

/// Represents different protocols that can be handled by the `ProtocolHandler`.
#[derive(Debug)]
pub enum Protocol {
    #[cfg(feature = "protocols-sshfs")]
    Sshfs,
}
