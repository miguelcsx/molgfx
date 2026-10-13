//! A lost device as a typed value rather than a message string.

/// Why the backend reported the device lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceLossReason {
    /// The device was destroyed on purpose, by the application or on teardown.
    Destroyed,
    /// The driver, adapter or browser dropped the device: a reset, a removed
    /// adapter, a timeout or an unrecoverable fault.
    Unknown,
}

impl DeviceLossReason {
    /// A stable lowercase name for logs and the browser event.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Destroyed => "destroyed",
            Self::Unknown => "unknown",
        }
    }
}

/// The recorded cause of a device loss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceLoss {
    /// Classified reason.
    pub reason: DeviceLossReason,
    /// Backend-supplied detail, possibly empty.
    pub message: String,
}

impl DeviceLoss {
    /// Whether opening a fresh device and rebuilding every resource from the
    /// scene is expected to succeed.
    ///
    /// A device the application destroyed is gone by intent; any other loss
    /// leaves the adapter available to open again.
    #[must_use]
    pub const fn recoverable(&self) -> bool {
        match self.reason {
            DeviceLossReason::Destroyed => false,
            DeviceLossReason::Unknown => true,
        }
    }
}

#[cfg(test)]
#[path = "device_loss_tests.rs"]
mod tests;
