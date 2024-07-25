mod categories;
mod asn_list;
mod success_fail;
mod scraping;
mod llm;

use anyhow::Result;
use futures::future::join_all;
use rand::prelude::SliceRandom;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};
use crate::asn_list::load_asn_domains;
use crate::scraping::website_text;
use crate::success_fail::{Domain, failures, success};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    tracing_subscriber::fmt::init();

    // Load the domains
    info!("Loading domains");
    let mut domains = load_asn_domains()?;
    info!("Loaded {} domains", domains.len());

    // Shuffle the domains (so in test runs we aren't always hitting the same ones)
    domains.shuffle(&mut rand::thread_rng());

    // Create the channels for results
    let report_success = success().await;
    let report_failures = failures().await;

    // Process the domains
    let already_done = std::fs::read_to_string("categories.csv").unwrap_or_default();
    let mut futures = Vec::new();
    for domain in domains.into_iter() {
        // Skip domains we've already done - in case we have to run it more than once
        if already_done.contains(&domain) {
            continue;
        }

        // Spawn the domain processor for this domain
        let my_success = report_success.clone();
        let my_failure = report_failures.clone();
        let future = process_domain(domain, my_success, my_failure);
        futures.push(future);
    }

    const BATCH_SIZE: usize = 32;
    while !futures.is_empty() {
        let the_future: Vec<_> = futures.drain( 0 .. usize::min(BATCH_SIZE, futures.len()) ).collect();
        let _ = join_all(the_future).await;
    }

    Ok(())
}

async fn process_domain(domain: String, on_success: Sender<Domain>, on_fail: Sender<String>) {
    //info!("Processing domain: {}", domain);
    // Scrape the website
    let detected_keywords = website_text(&domain).await;
    match detected_keywords {
        Ok(text) => {
            info!("Scraped text for domain: {}", domain);
            info!("Text: {}", text);
            // Keyword list is too short
            if text.len() < 3 {
                warn!("Keyword list too short for domain: {}", domain);
                let _ = on_fail.send(domain).await;
                return;
            }
            categorize_domain(domain, text, on_success, on_fail).await;
        }
        Err(_) => {
            // Scraping failed altogether
            warn!("Scraping failed for domain: {}", domain);
            let _ = on_fail.send(domain).await;
        }
    }
}

async fn categorize_domain(domain: String, keywords: String, on_success: Sender<Domain>, failures: Sender<String>) {
    info!("Categorizing domain: {}", domain);
    let allowed_list = categories::category_prompt();
    let prompt = format!("Please categorize this domain with a single keyword in English. \
            Do not elaborate, do not explain or otherwise enhance the answer.\n\n \
            {allowed_list} \
            The domain is: {domain}. Here are some items from the website: {keywords}");

    let response = llm::llm_completion(&prompt).await;
    match response {
        Err(_) => {
            warn!("LLM failed for domain: {}", domain);
            let _ = failures.send(domain).await;
        }
        Ok(result) => {
            // No response
            if result.is_empty() {
                warn!("No response from LLM for domain: {}", domain);
                let _ = failures.send(domain).await;
                return;
            }
            // Wordy response
            if result.split_whitespace().count() > 1 {
                warn!("LLM response too wordy for domain: {}", domain);
                let _ = failures.send(domain).await;
                return;
            }
            // Not in the allowed list
            if !categories::word_in_list(&result) {
                warn!("LLM response not in allowed list for domain: {}", domain);
                warn!("Response: {}", result);
                let _ = failures.send(domain).await;
                return;
            }
            // Success
            info!("Categorized domain: {}, Category: {}", domain, result);
            let _ = on_success.send(Domain {
                domain,
                category: result,
            }).await;
        }
    }
}