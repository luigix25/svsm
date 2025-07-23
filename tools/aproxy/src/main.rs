// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Red Hat, Inc
//
// Author: Stefano Garzarella <sgarzare@redhat.com>
// Author: Tyler Fanelli <tfanelli@redhat.com>

mod attest;
mod backend;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use std::{fs, os::unix::net::UnixListener};
use vsock::{VMADDR_CID_ANY, VsockAddr, VsockListener};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
#[clap(group(clap::ArgGroup::new("transport").required(true)))]
struct Args {
    /// HTTP url to KBS (e.g. http://server:4242)
    #[clap(long)]
    url: String,

    /// Backend attestation protocol that the server implements.
    #[clap(long = "protocol")]
    backend: ArgsBackend,

    /// UNIX domain socket path to the SVSM serial port
    #[clap(long, group = "transport")]
    unix: Option<String>,

    /// vsock listening port where SVSM will connect [default: 1995]
    #[clap(long, group = "transport", num_args = 0..=1, default_missing_value = "1995")]
    vsock: Option<u32>,

    /// Force Unix domain socket removal before bind
    #[clap(long, short, conflicts_with = "vsock", default_value_t = false)]
    force: bool,
}

/// Enum to represent possible backends in the CLI.
/// This must not have any attached data to its variants for `ValueEnum`
/// to work.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ArgsBackend {
    Kbs,
}

macro_rules! accept_loop {
    ($listener:expr, $url:expr, $backend:expr) => {
        for stream in $listener.incoming() {
            let mut stream = stream.context("Failed to accept connection")?;
            let mut http_client = backend::HttpClient::new($url.clone(), $backend.into())?;
            attest::attest(&mut stream, &mut http_client)?;
        }
    };
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(port) = args.vsock {
        let listener = VsockListener::bind(&VsockAddr::new(VMADDR_CID_ANY, port))
            .context("bind and listen failed")?;
        accept_loop!(listener, args.url, args.backend);
    } else if let Some(unix) = args.unix {
        if args.force {
            let _ = fs::remove_file(&unix);
        }

        let listener = UnixListener::bind(unix).context("unable to bind to UNIX socket")?;
        accept_loop!(listener, args.url, args.backend);
    }

    Ok(())
}
