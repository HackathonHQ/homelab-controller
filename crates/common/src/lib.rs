//! Shared types and wire protocol for the homelab-controller.
//!
//! Both the server (running on a controlled host) and the client (the
//! operator's machine) depend on this crate so they agree on the message
//! format sent over TCP.

pub mod frame;
pub mod protocol;

pub use frame::{read_message, write_message, FrameError};
pub use protocol::{HostStatus, PowerAction, Request, Response};

/// Default TCP port the server listens on.
pub const DEFAULT_PORT: u16 = 7878;
