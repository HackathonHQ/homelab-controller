//! Homelab controller server.
//!
//! Responds to control requests (ping, status, power, exec). It can run in
//! two modes:
//!
//! * `--stdio` — speak the framed protocol over stdin/stdout. This is how the
//!   client reaches it over SSH (`ssh host homelab-server --stdio`): sshd
//!   authenticates the connection and pipes our stdin/stdout over the
//!   encrypted channel, so there is no listening port to secure.
//! * `[ADDR]` (default `0.0.0.0:7878`) — listen for raw TCP connections, one
//!   thread per client. Handy for local development or behind an SSH tunnel.
//!
//! All diagnostic logging goes to **stderr** so it never corrupts the
//! protocol stream when running over stdio.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::thread;
use std::time::Instant;

use common::{
    read_message, write_message, FrameError, HostStatus, PowerAction, Request, Response,
    DEFAULT_PORT,
};

fn main() -> anyhow::Result<()> {
    let started = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--stdio") {
        return serve_stdio(started);
    }

    // TCP mode: optional bind address as the first positional arg.
    let addr = args
        .into_iter()
        .next()
        .unwrap_or_else(|| format!("0.0.0.0:{DEFAULT_PORT}"));
    serve_tcp(&addr, started)
}

/// Serve a single client over stdin/stdout (the SSH transport).
fn serve_stdio(started: Instant) -> anyhow::Result<()> {
    eprintln!("homelab-server: serving over stdio");
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    serve(&mut reader, &mut writer, started)?;
    Ok(())
}

/// Listen for raw TCP connections, handling each on its own thread.
fn serve_tcp(addr: &str, started: Instant) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("homelab-server listening on {}", listener.local_addr()?);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // `started` is Copy, so each thread gets its own clock origin.
                thread::spawn(move || {
                    let peer = stream
                        .peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| "<unknown>".into());
                    eprintln!("client connected: {peer}");
                    match handle_tcp_client(stream, started) {
                        Ok(()) => eprintln!("client {peer} disconnected"),
                        Err(e) => eprintln!("client {peer} disconnected: {e}"),
                    }
                });
            }
            Err(e) => eprintln!("failed to accept connection: {e}"),
        }
    }

    Ok(())
}

/// Adapt a duplex [`TcpStream`] to the generic [`serve`] loop.
fn handle_tcp_client(stream: TcpStream, started: Instant) -> Result<(), FrameError> {
    // The read and write halves are the same socket; clone for two handles.
    let mut reader = stream.try_clone()?;
    let mut writer = stream;
    serve(&mut reader, &mut writer, started)
}

/// Read requests from `reader` and write responses to `writer` until the
/// peer closes its side cleanly.
fn serve<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    started: Instant,
) -> Result<(), FrameError> {
    loop {
        let request: Request = match read_message(reader) {
            Ok(req) => req,
            // A clean hang-up between messages is a normal end-of-loop.
            Err(FrameError::Io(e)) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        };

        let response = dispatch(request, started);
        write_message(writer, &response)?;
    }
}

/// Map a single request to its response.
fn dispatch(request: Request, started: Instant) -> Response {
    match request {
        Request::Ping => Response::Pong,
        Request::Status => Response::Status(HostStatus {
            hostname: hostname(),
            uptime_secs: started.elapsed().as_secs(),
            powered_on: true,
        }),
        Request::Power(action) => {
            // Real hardware control (GPIO/IPMI/wake-on-LAN) would go here.
            eprintln!("power action requested: {action:?}");
            match action {
                PowerAction::On | PowerAction::Off | PowerAction::Reset => Response::Ok,
            }
        }
        Request::Exec { command } => exec(&command),
    }
}

/// Run a shell command and capture its output.
fn exec(command: &str) -> Response {
    let output = Command::new("sh").arg("-c").arg(command).output();
    match output {
        Ok(out) => Response::Output {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            exit_code: out.status.code().unwrap_or(-1),
        },
        Err(e) => Response::Error(format!("failed to run command: {e}")),
    }
}

/// Best-effort hostname lookup without pulling in an extra dependency.
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}
