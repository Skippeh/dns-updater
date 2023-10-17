use std::{collections::HashMap, net::IpAddr};

use anyhow::Context;
use log::Level;

use crate::{
    digitalocean::{DigitalOcean, Domain, QueryError},
    wan_ip_query::query_wan_ip,
    AppArgs, AppError,
};

pub async fn start(args: AppArgs) -> Result<(), AppError> {
    let digital_ocean =
        DigitalOcean::new(args.do_api_key).context("Failed to create DigitalOcean client")?;

    loop {
        if args.apply {
            log::info!("Starting records update...");
        }

        let wan_ip = query_wan_ip().await?;
        let wan_ip_type = match &wan_ip {
            IpAddr::V4(_) => "A",
            IpAddr::V6(_) => "AAAA",
        };

        let account_domains = digital_ocean
            .list_all_domains()
            .await
            .map_err(|err| match err {
                QueryError::Unauthorized(_) => AppError::TestFailedDOKeyValidation,
                err => AppError::OtherError(err.into()),
            })?;

        let (map, unknown_domains) =
            map_domain_args_to_account_domains(&args.domains, &account_domains);

        let mut domain_records_futures = HashMap::with_capacity(map.len());

        for (domain, _) in &map {
            domain_records_futures.insert(domain, digital_ocean.query_domain_records(&domain.name));
        }

        let mut results = Vec::new();

        for (domain, future) in domain_records_futures {
            match future.await {
                Ok(records) => {
                    for arg_domain in map
                        .get(domain)
                        .expect("Map should always contain the domain")
                    {
                        let arg_domain_lowercase = arg_domain.to_lowercase();
                        let record = records.iter().find(|rec| {
                            rec.ty == wan_ip_type
                                && format!("{}.{}", rec.name.to_lowercase(), &domain.name)
                                    == arg_domain_lowercase
                        });

                        match record {
                            Some(record) => {
                                if !args.apply {
                                    results.push((
                                        Level::Info,
                                        format!(
                                            "{} -> {} (current: {}, TTL: {})",
                                            arg_domain, wan_ip, record.data, record.ttl
                                        ),
                                    ));
                                } else {
                                    // Update record
                                    match digital_ocean
                                        .update_record(
                                            &domain.name,
                                            record.id,
                                            &record.ty,
                                            &wan_ip.to_string(),
                                        )
                                        .await
                                    {
                                        Ok(new_record) => results.push((
                                            Level::Info,
                                            format!(
                                                "✓ {}.{} -> {} (current: {}, TTL: {})",
                                                record.name,
                                                domain.name,
                                                new_record.data,
                                                record.data,
                                                record.ttl
                                            ),
                                        )),
                                        Err(err) => {
                                            results.push((
                                                Level::Error,
                                                format!("✗ {arg_domain}: {err}"),
                                            ));
                                        }
                                    }
                                }
                            }
                            None => {
                                results.push((
                                    Level::Error,
                                    format!(
                                        "{}{}: Record does not exist, or is not of type {}",
                                        if args.apply { "✗ " } else { "" },
                                        arg_domain,
                                        wan_ip_type
                                    ),
                                ));
                            }
                        }
                    }
                }
                Err(err) => {
                    for domain in map
                        .get(domain)
                        .expect("Map should always contain the domain")
                    {
                        results.push((Level::Error, format!("{domain}: {err:#?}")));
                    }
                }
            }
        }

        for arg_domain in unknown_domains {
            results.push((
                Level::Error,
                format!(
                    "{}{}: Domain does not exist on this DigitalOcean account",
                    if args.apply { "✗ " } else { "" },
                    arg_domain
                ),
            ))
        }

        for (log_level, message) in results {
            log::log!(log_level, "{}", message);
        }

        if args.apply {
            if let Some(interval) = args.update_interval {
                if interval == 0 {
                    break;
                }

                let wait_duration = std::time::Duration::from_secs(interval as u64 * 60);
                let next_update_time = chrono::Local::now() + chrono::Duration::minutes(interval);

                log::info!("Next update: {}", next_update_time);

                tokio::time::sleep(wait_duration).await;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    Ok(())
}

fn map_domain_args_to_account_domains<'a, 'b>(
    domain_args: &'a [String],
    account_domains: &'b [Domain],
) -> (HashMap<&'b Domain, Vec<&'a str>>, Vec<&'a str>) {
    let mut map = HashMap::new();
    let mut unknown_domains = vec![];

    for domain_arg in domain_args {
        let domain = account_domains.iter().find(|domain| {
            domain_arg
                .to_lowercase()
                .ends_with(&domain.name.to_lowercase())
        });

        if let Some(domain) = domain {
            map.entry(domain.clone())
                .or_insert(vec![])
                .push(domain_arg.as_str());
        } else {
            unknown_domains.push(domain_arg.as_str());
        }
    }

    (map, unknown_domains)
}
