use crate::args::Args;
use anyhow::Result;
use reqwest::{
    blocking::{Client, Response},
    header,
};

#[derive(Debug)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(args: &Args) -> Result<Self> {
        Ok(Self {
            client: ApiClient::build_client(args)?,
            base_url: args.base_url.clone(),
        })
    }

    pub fn get(&self, path: &str) -> Result<Response> {
        self.client
            .get(format!("{}/{}", self.base_url, path))
            .send()
            .map_err(Into::into)
    }

    fn build_client(args: &Args) -> Result<Client> {
        let mut headers = header::HeaderMap::new();
        let mut auth_value = header::HeaderValue::from_str(&format!(
            "Bearer {}",
            Args::parse_secret(&args.api_key)?
        ))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        Client::builder()
            .default_headers(headers)
            .timeout(None)
            .build()
            .map_err(Into::into)
    }
}
