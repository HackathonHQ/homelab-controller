//! The application-level messages exchanged between client and server.

use serde::{Deserialize, Serialize};

/// A command sent from the client to the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    /// Liveness check; expects [`Response::Pong`].
    Ping,
    /// Ask the host for its current status.
    Status,
    /// Perform a power action on the controlled host.
    Power(PowerAction),
    /// Run a shell command on the host and return its output.
    Exec { command: String },
}

/// Power control actions, mirroring the buttons on a physical KVM.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PowerAction {
    On,
    Off,
    Reset,
}

/// A reply sent from the server back to the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Response {
    /// Reply to [`Request::Ping`].
    Pong,
    /// Reply to [`Request::Status`].
    Status(HostStatus),
    /// A command completed successfully with no payload.
    Ok,
    /// Output of an [`Request::Exec`] command.
    Output {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    /// The request could not be fulfilled.
    Error(String),
}

/// A snapshot of the controlled host's state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostStatus {
    pub hostname: String,
    /// Seconds the server process has been running.
    pub uptime_secs: u64,
    /// Whether the host is considered powered on (always true while the
    /// server is reachable; placeholder for real GPIO/IPMI integration).
    pub powered_on: bool,
}
