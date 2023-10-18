use std::net::IpAddr;

use reqwest::Url;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[derive(Debug, thiserror::Error)]
pub enum WanIpError {
    #[error("IO error: {0}")]
    Io(tokio::io::Error),
    #[error("Query failed: {0}")]
    QueryFailed(anyhow::Error),
    #[error("URL parse error: {0}")]
    UrlParse(url::ParseError),
    #[error("There are no WAN IP API endpoints configured")]
    NoApiEndpointsConfigured,
}

impl From<tokio::io::Error> for WanIpError {
    fn from(err: tokio::io::Error) -> Self {
        WanIpError::Io(err)
    }
}

impl From<url::ParseError> for WanIpError {
    fn from(err: url::ParseError) -> Self {
        WanIpError::UrlParse(err)
    }
}

const DEFAULT_APIS: [&str; 2] = ["https://api.seeip.org", "https://api64.ipify.org"];
const FILE_PATH: &str = "api_urls.txt";

pub async fn query_wan_ip() -> Result<IpAddr, WanIpError> {
    let api_urls = load_api_urls().await?;
    let mut last_error: Option<anyhow::Error> = None;

    if api_urls.is_empty() {
        return Err(WanIpError::NoApiEndpointsConfigured);
    }

    for api_url in api_urls {
        let response = reqwest::get(api_url).await;

        match response {
            Ok(response) => match response.text().await {
                Ok(text) => match text.parse::<IpAddr>() {
                    Ok(ip) => return Ok(ip),
                    Err(err) => last_error = Some(err.into()),
                },
                Err(err) => last_error = Some(err.into()),
            },
            Err(err) => last_error = Some(err.into()),
        }
    }

    Err(WanIpError::QueryFailed(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("Failed to query WAN IP")
    })))
}

async fn load_api_urls() -> Result<Vec<Url>, WanIpError> {
    let file = File::open(FILE_PATH).await;

    let api_urls = match file {
        Ok(mut file) => {
            let mut contents = String::new();
            file.read_to_string(&mut contents).await?;
            contents
                .split('\n')
                .filter(|url| !url.is_empty())
                .map(|url| Url::parse(url))
                .collect::<Result<Vec<_>, _>>()?
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                let mut file = File::create(FILE_PATH).await?;

                for api_url in DEFAULT_APIS {
                    file.write_all(format!("{}\n", api_url).as_bytes()).await?;
                }

                DEFAULT_APIS
                    .into_iter()
                    .map(Url::parse)
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                return Err(WanIpError::Io(err));
            }
        }
    };

    Ok(api_urls)
}
