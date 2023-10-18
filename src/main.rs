mod digitalocean;
mod updater;
mod wan_ip_query;

use anyhow::{Context, Result};
use clap::Parser;
use wan_ip_query::WanIpError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("The DigitalOcean key is invalid")]
    TestFailedDOKeyValidation,
    #[error("Failed to query WAN IP: {0}")]
    TestFailedToQueryWanIp(WanIpError),
    #[error("An unexpected error occurred: {0}")]
    OtherError(anyhow::Error),
}

// Generic error handling
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::OtherError(err)
    }
}

impl From<wan_ip_query::WanIpError> for AppError {
    fn from(err: wan_ip_query::WanIpError) -> Self {
        AppError::TestFailedToQueryWanIp(err)
    }
}

impl AppError {
    pub fn error_code(&self) -> i32 {
        match self {
            AppError::TestFailedDOKeyValidation => 1,
            AppError::TestFailedToQueryWanIp(_) => 2,
            AppError::OtherError(_) => 3,
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct AppArgs {
    /// API key for DigitalOcean
    #[clap(short('a'), long("api-key"), env, hide_env_values = true)]
    // hide_env_values = true to avoid leaking secrets
    pub do_api_key: String,
    /// How often (in minutes) to check WAN IP and update records.
    /// If unset the records will only be updated once and then the program will exit
    #[clap(short('m'), long, allow_negative_numbers(false), env)]
    pub update_interval: Option<i64>,
    /// If this flag is **NOT** set the program will only validate that the specified
    /// domain records are of type A/AAAA depending on WAN ip type.
    /// It will also preview the changes that would be made
    #[clap(default_value_t = false, short('A'), long, env)]
    pub apply: bool,
    /// List of fully qualified domain names to update the values for
    #[clap(required = true, short('d'), long("domain"), env)]
    pub domains: Vec<String>,
    /// If this flag is set the 10 second warning on startup will not be shown before applying record changes.
    #[clap(default_value_t = false, short('S'), long, env)]
    pub skip_warning: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = app_main().await {
        let error_code = error.error_code();
        log::error!("A fatal error occurred: {:?}", anyhow::format_err!(error));
        std::process::exit(error_code)
    }
}

async fn app_main() -> Result<(), AppError> {
    simple_logger::SimpleLogger::new()
        .with_local_timestamps()
        .with_level(get_minimum_log_level())
        .env()
        .with_timestamp_format(time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second]"
        ))
        .init()
        .context("Failed to initialize logger")?;

    let args = AppArgs::parse();
    let apply = args.apply;

    if apply && !args.skip_warning {
        log::info!("WARNING: Applying changes to following domain records, terminate with CTRL+C to cancel (continuing in 10 seconds, pass -S to skip this warning):");

        for domain in &args.domains {
            log::info!("- {domain}");
        }

        tokio::time::sleep(std::time::Duration::from_secs(10)).await
    }

    updater::start(args).await?;

    if !apply {
        log::info!("Run with -A to apply changes to domain records. Specify -m to repeatedly update records");
    }

    Ok(())
}

fn get_minimum_log_level() -> log::LevelFilter {
    if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    }
}
