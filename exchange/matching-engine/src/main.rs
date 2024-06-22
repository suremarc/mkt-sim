use std::{convert::Infallible, net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand};
use matching_engine::journaler::server;
use monoio::{fs::OpenOptions, net::TcpListener};

#[derive(Debug, Clone, Parser)]
struct Args {
    #[clap(short = 'f')]
    logfile: PathBuf,

    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Journal { addr: SocketAddr },
    Process,
}

#[monoio::main]
async fn main() -> anyhow::Result<Infallible> {
    let args = Args::parse();

    let mut opts = OpenOptions::new();
    let logfile = match args.cmd {
        Command::Journal { .. } => opts.append(true),
        Command::Process => opts.read(true),
    }
    .open(args.logfile)
    .await?;

    match args.cmd {
        Command::Journal { addr } => {
            let listener = TcpListener::bind(addr)?;
            server(logfile, listener).await
        }
        Command::Process => todo!(),
    }
}
