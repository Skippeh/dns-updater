mod digitalocean;
mod updater;
mod wan_ip_query;

use anyhow::{Context, Result};
use clap::Parser;
use wan_ip_query::WanIpError;

#[derive(Debug)]
pub enum AppError {
    TestFailedDOKeyValidation,
    TestFailedToQueryWanIp(WanIpError),
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

// Process exit codes for each type of error
impl From<AppError> for i32 {
    fn from(err: AppError) -> Self {
        match err {
            AppError::TestFailedDOKeyValidation => 1,
            AppError::TestFailedToQueryWanIp(_) => 2,
            AppError::OtherError(_) => 3,
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct AppArgs {
    /// API key for DigitalOcean
    #[clap(short('a'), long("api-key"))]
    pub do_api_key: String,
    /// How often (in minutes) to check WAN IP and update records.
    /// If unset the records will only be updated once and then the program will exit
    #[clap(short('m'), long, allow_negative_numbers(false))]
    pub update_interval: Option<i64>,
    /// If this flag is **NOT** set the program will only validate that the specified
    /// domain records are of type A/AAAA depending on WAN ip type.
    /// It will also preview the changes that would be made
    #[clap(default_value_t = false, short('A'), long)]
    pub apply: bool,
    /// List of fully qualified domain names to update the values for
    #[clap(required = true, short('d'), long("domain"))]
    pub domains: Vec<String>,
    /// If this flag is set the 10 second warning on startup will not be shown before applying record changes.
    #[clap(default_value_t = false, short('S'), long)]
    pub skip_warning: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = app_main().await {
        log::error!("A fatal error occurred: {error:#?}");
        std::process::exit(error.into())
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
        log::info!("WARNING: Applying changes to following domain records, terminate with CTRL+C to cancel (continuing in 10 seconds):");

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
