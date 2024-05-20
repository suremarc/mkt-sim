use std::{ops::Deref, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use exchange::{api, Metadata};
use rocket::{Ignite, Rocket};
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

#[rocket::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let rocket = api::rocket().ignite().await.context("ignite rocket")?;

    match args.command {
        Command::Migrate => migrate(rocket).await?,
        Command::Serve => {
            let _ = rocket.launch().await;
        }
    };

    Ok(())
}

async fn migrate(rocket: Rocket<Ignite>) -> Result<()> {
    let db = Metadata::fetch(&rocket).context("fetch database")?;
    sqlx::migrate!()
        .run(db.deref())
        .await
        .context("migrate database")?;

    Ok(())
}
