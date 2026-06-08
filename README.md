# homelab-controller

A makeshift "software"-focused version of [PiKVM](https://pikvm.org/) — remotely
control a homelab host over the network, no dedicated KVM hardware required.

## Layout

A Cargo workspace with three crates:

| Crate                | Binary            | Purpose                                                      ---|
| -------------------- | ----------------- | ----------------------------------------------------------------|
| `crates/common`      | —                 | Shared code, data structures, functions and more                |
| `crates/server`      | `homelab-server`  | Server code running crons, authentication, GPIO controllers     |
| `crates/client`      | `homelab-client`  | Client libraries for authentication, UI, data tansport          |

### Networking & framing

Each message on the wire is a **length-prefixed JSON frame** (`u32` big-endian
length + JSON payload), so a byte stream can be split back into discrete
messages — see `crates/common/src/frame.rs`. Because the framing is generic
over any `Read`/`Write`, the same protocol runs over either transport below.

## Transports & authentication

### SSH (recommended)

The client launches the server on the host **through SSH** and speaks the
protocol over `ssh`'s stdin/stdout:

```sh
homelab-client --ssh user@host status
```

This runs `ssh user@host homelab-server --stdio`. SSH authenticates the
connection with your existing keys (`~/.ssh/authorized_keys` on the host) and
encrypts everything — so **access is only possible when SSH between the two
devices is permitted**. There is no listening port on the network, so there is
nothing extra to firewall or attack.

Requirements:

- The `homelab-server` binary must be installed on the host and on its `PATH`
  (or pass an absolute path with `--server-bin /opt/bin/homelab-server`).
- Anything you'd put in `~/.ssh/config` (port, identity file, jump host, user)
  applies automatically, since we just shell out to `ssh`.

### Direct TCP (development / behind a tunnel)

The server can also listen for raw TCP connections:

```sh
homelab-server                 # binds 0.0.0.0:7878
homelab-server 127.0.0.1:7878  # bind explicitly
homelab-client --addr 127.0.0.1:7878 status
```

> ⚠️ The raw TCP transport is **unauthenticated and unencrypted**. Use it only
> on a trusted network, or bind it to `127.0.0.1` and reach it through an SSH
> tunnel (`ssh -L 7878:localhost:7878 user@host`). For remote control, prefer
> the `--ssh` transport above.

## Build & test

```sh
cargo build
cargo test
```

## Deploy the server to a host

```sh
cargo build --release
scp target/release/homelab-server user@host:/usr/local/bin/
# then from your machine:
cargo run -p client -- --ssh user@host status
```

## Commands

```sh
homelab-client --ssh user@host ping
homelab-client --ssh user@host status
homelab-client --ssh user@host power reset      # on | off | reset
homelab-client --ssh user@host exec "uptime"
```

## Roadmap

- [x] SSH-based authentication (via the `--ssh` stdio transport)
- [ ] Real power control (GPIO / IPMI / wake-on-LAN) behind `Request::Power`
- [ ] Streaming / interactive sessions instead of one-shot commands
- [ ] Optional TLS + token auth for the standalone TCP daemon
