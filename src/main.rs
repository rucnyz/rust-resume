mod adapter;
mod adapters;
mod cli;
mod config;
mod index;
mod query;
mod search;
mod session;

fn main() -> anyhow::Result<()> {
    cli::run()
}
