use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use exchange::{Accounting, Instruments};
use rocket_db_pools::Database;

#[derive(Debug, Clone, Parser)]
struct Args {
    #[arg(short, long, num_args(0..), default_value = "local.toml")]
    config: Vec<PathBuf>,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Command {
    Migrate,
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let rocket = rocket::build()
        .attach(Instruments::init())
        .attach(Accounting::init())
        .ignite()
        .await
        .context("ignite rocket")?;

    match args.command {
        Command::Migrate => sqlx::migrate!()
            .run(&Instruments::fetch(&rocket).context("fetch database")?.0)
            .await
            .context("migrate database")?,
        Command::Serve => {
            let _ = rocket.launch().await;
        }
    };

    Ok(())
}
