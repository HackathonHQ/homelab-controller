//! Homelab controller client.
//!
//! Connects to the server, sends a single control command, prints the
//! response, and exits.
//!
//! Two transports are supported:
//!
//! * `--ssh DEST` (recommended) — launches `ssh DEST homelab-server --stdio`
//!   and runs the protocol over that encrypted channel. SSH itself
//!   authenticates the connection (via `authorized_keys`), so access is only
//!   possible when SSH between the two devices is permitted.
//! * `--addr HOST:PORT` — connect directly over TCP (for local development or
//!   through an existing SSH tunnel). Defaults to `127.0.0.1:7878`.
//!
//! Usage:
//!   homelab-client [--ssh DEST [--server-bin PATH] | --addr HOST:PORT] <command>
//!
//! Commands:
//!   ping                  liveness check
//!   status                show host status
//!   power <on|off|reset>  power control
//!   exec <shell command>  run a command on the host

use std::net::TcpStream;
use std::process::{Command, ExitCode, Stdio};

use common::{read_message, write_message, PowerAction, Request, Response, DEFAULT_PORT};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> anyhow::Result<()> {
    let mut addr: Option<String> = None;
    let mut ssh_dest: Option<String> = None;
    // The server binary as invoked on the remote host (must be on its PATH,
    // or pass an absolute path with --server-bin).
    let mut server_bin = "homelab-server".to_string();

    // Parse leading flags; the first non-flag token begins the command.
    let mut it = args.iter();
    let mut command_tokens: Vec<String> = Vec::new();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--addr" => {
                addr = Some(value_for(&mut it, "--addr")?);
            }
            "--ssh" => {
                ssh_dest = Some(value_for(&mut it, "--ssh")?);
            }
            "--server-bin" => {
                server_bin = value_for(&mut it, "--server-bin")?;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => {
                command_tokens.push(arg.clone());
                command_tokens.extend(it.cloned());
                break;
            }
        }
    }

    let request = parse_command(&command_tokens)?;

    let response = match ssh_dest {
        Some(dest) => over_ssh(&dest, &server_bin, &request)?,
        None => {
            let addr = addr.unwrap_or_else(|| format!("127.0.0.1:{DEFAULT_PORT}"));
            over_tcp(&addr, &request)?
        }
    };

    print_response(&response);
    Ok(())
}

/// Pull the value that follows a flag, erroring if it is missing.
fn value_for<'a>(it: &mut impl Iterator<Item = &'a String>, flag: &str) -> anyhow::Result<String> {
    it.next()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
}

/// Send one request over a direct TCP connection.
fn over_tcp(addr: &str, request: &Request) -> anyhow::Result<Response> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|e| anyhow::anyhow!("could not connect to {addr}: {e}"))?;
    write_message(&mut stream, request)?;
    let response = read_message(&mut stream)?;
    Ok(response)
}

/// Send one request by launching the server on the remote host over SSH and
/// speaking the protocol across `ssh`'s stdin/stdout.
fn over_ssh(dest: &str, server_bin: &str, request: &Request) -> anyhow::Result<Response> {
    // stderr is inherited so SSH auth prompts and remote server logs surface
    // on the operator's terminal; password/passphrase prompts use the tty.
    let mut child = Command::new("ssh")
        .arg(dest)
        .arg(server_bin)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to launch ssh: {e}"))?;

    let mut remote_stdin = child.stdin.take().expect("stdin was piped");
    let mut remote_stdout = child.stdout.take().expect("stdout was piped");

    write_message(&mut remote_stdin, request)?;
    // Closing our write side signals EOF, so the remote server exits after
    // replying to this one request.
    drop(remote_stdin);

    let response = read_message::<_, Response>(&mut remote_stdout);
    let status = child.wait()?;

    response.map_err(|e| {
        if !status.success() {
            anyhow::anyhow!(
                "ssh/remote exited with {status}; is `{server_bin}` installed and on PATH \
                 on the host? (override with --server-bin)"
            )
        } else {
            anyhow::anyhow!("failed to read response: {e}")
        }
    })
}

/// Turn the remaining CLI tokens into a [`Request`].
fn parse_command(tokens: &[String]) -> anyhow::Result<Request> {
    let (cmd, args) = tokens
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("{USAGE}"))?;

    match cmd.as_str() {
        "ping" => Ok(Request::Ping),
        "status" => Ok(Request::Status),
        "power" => {
            let action = args
                .first()
                .ok_or_else(|| anyhow::anyhow!("power needs one of: on, off, reset"))?;
            let action = match action.as_str() {
                "on" => PowerAction::On,
                "off" => PowerAction::Off,
                "reset" => PowerAction::Reset,
                other => anyhow::bail!("unknown power action: {other}"),
            };
            Ok(Request::Power(action))
        }
        "exec" => {
            if args.is_empty() {
                anyhow::bail!("exec needs a command to run");
            }
            Ok(Request::Exec {
                command: args.join(" "),
            })
        }
        other => anyhow::bail!("unknown command: {other}\n{USAGE}"),
    }
}

fn print_response(response: &Response) {
    match response {
        Response::Pong => println!("pong"),
        Response::Ok => println!("ok"),
        Response::Status(status) => {
            println!("hostname:   {}", status.hostname);
            println!("uptime:     {}s", status.uptime_secs);
            println!("powered on: {}", status.powered_on);
        }
        Response::Output {
            stdout,
            stderr,
            exit_code,
        } => {
            print!("{stdout}");
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
            println!("[exit code {exit_code}]");
        }
        Response::Error(msg) => eprintln!("server error: {msg}"),
    }
}

const USAGE: &str = "usage: homelab-client [--ssh DEST [--server-bin PATH] | --addr HOST:PORT] \
                     <ping|status|power <on|off|reset>|exec <cmd>>";
