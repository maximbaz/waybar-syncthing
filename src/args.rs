use anyhow::Result;
use clap::Parser;
use std::{fs, path::Path};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, required = true, env = "SYNCTHING_API_KEY")]
    pub api_key: String,

    #[arg(
        short,
        long,
        default_value = "http://localhost:8384",
        env = "SYNCTHING_BASE_URL"
    )]
    pub base_url: String,
}

impl Args {
    pub fn parse_secret(input: &str) -> Result<String> {
        if Path::new(input).exists() {
            Ok(fs::read_to_string(input)?.trim().to_string())
        } else {
            Ok(input.to_string())
        }
    }
}
