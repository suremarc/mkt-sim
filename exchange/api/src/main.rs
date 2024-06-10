use clap::{Parser, Subcommand};
use rocket::launch;

#[derive(Parser)]
struct Args {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Api,
    Process,
}

#[launch]
fn rocket() -> _ {
    tracing_subscriber::fmt::init();

    match Args::parse().cmd {
        Command::Api => exchange::api::rocket(),
        Command::Process => exchange::process::rocket(),
    }
}
