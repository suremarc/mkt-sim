use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Parser)]
struct Args {
    #[clap(short = 'f')]
    logfile: PathBuf,

    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Journal,
    Process,
}

fn main() {
    let args = Args::parse();

    match args.cmd {
        Command::Journal => todo!(),
        Command::Process => todo!(),
    }
}
