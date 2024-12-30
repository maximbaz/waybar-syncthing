use anyhow::Result;
use api_client::ApiClient;
use args::Args;
use clap::Parser;
use runner::Runner;

mod api_client;
mod args;
mod runner;

fn main() -> Result<()> {
    let args = Args::try_parse()?;
    let client = ApiClient::new(&args)?;

    Runner::new(client).main_loop()
}
