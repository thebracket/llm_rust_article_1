use anyhow::Result;
use futures::future::join_all;
use itertools::Itertools;
use rand::prelude::SliceRandom;
use reqwest::header;
use scraper::Html;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use load_data::load_asn_domains;

const LLM_API: &str = "http://localhost:11434/api/generate";

#[derive(Deserialize)]
struct Response {
    response: String,
}

async fn llm_completion(prompt: &str) -> Result<String> {
    let request = json!({
        "model": "llama3.1",
        "prompt": prompt,
    });

    let client = reqwest::Client::new();
    let mut res = client.post(LLM_API)
        .json(&request)
        .send()
        .await?;

    let mut response = String::new();
    while let Some(chunk) = res.chunk().await? {
        let chunk: Response = serde_json::from_slice(&chunk)?;
        response.push_str(&chunk.response);
    }

    Ok(response)
}

fn find_content(selector: &str, document: &Html) -> Vec<String> {
    let selector = scraper::Selector::parse(selector).unwrap();
    let mut content = Vec::new();
    for element in document.select(&selector) {
        // Get all text elements matching the selector
        let e: String = element.text().collect::<String>();

        // Split at whitespace, and filter out words shorter than 3 characters and
        // convert to lowercase.
        let e: Vec<String> = e.split_whitespace()
            .filter(|s| s.len() > 3)
            .map(|s| s.trim().to_lowercase())
            .collect();

        if !e.is_empty() {
            content.extend(e);
        }
    }

    content
}

async fn website_text(domain: &str) -> Result<String> {
    let url = format!("http://{}/", domain);

    // Build a header with a Firefox user agent
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("Mozilla/5.0 (platform; rv:geckoversion) Gecko/geckotrail Firefox/firefoxversion")
    );

    // Setup Reqwest with the header
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    // Fetch the website
    let body = client
        .get(&url).send().await?
        .text().await?;

    // Parse the HTML
    let doc = scraper::Html::parse_document(&body);
    // Search for parts of the site with text in likely places
    let mut content = Vec::new();
    for items in ["title", "meta", "ul,li", "h1", "p"] {
        content.extend(find_content(items, &doc));
    }
    // We now have a big list of words (hopefully) from the website
    let result = content
        .into_iter() // Consuming iterator
        .sorted() // Sort alphabetically
        .dedup_with_count()// Deduplicatae, and return a tuple (count, word)
        .sorted_by(|a, b| b.0.cmp(&a.0)) // Sort by count, descending
        .map(|(count, word)| word)// Take only the word
        .take(100)// Take the top 100 words
        .join(" "); // Join them into a string

    Ok(result)
}

async fn append_to_file(filename: &str, line: &str) -> Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, format!("{}\n", line).as_bytes()).await?;
    Ok(())
}

async fn failures() -> Sender<String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
    tokio::spawn(async move {
        while let Some(domain) = rx.recv().await {
            println!("Failed to scrape: {}", domain);
            // Append to "failures.txt"
            if let Err(e) = append_to_file("failures.txt", &domain).await {
                eprintln!("Failed to write to file: {}", e);
            }
        }
    });
    return tx;
}

struct Domain {
    domain: String,
    category: String,
}

async fn success() -> Sender<Domain> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Domain>(32);
    tokio::spawn(async move {
        while let Some(domain) = rx.recv().await {
            println!("Domain: {}, Category: {}", domain.domain, domain.category);
            // Append to "categories.csv"
            if let Err(e) = append_to_file("categories.csv", &format!("{},{}", domain.domain, domain.category)).await {
                eprintln!("Failed to write to file: {}", e);
            }
        }
    });
    return tx;
}

async fn categorize_domain(domain: &str, text: &str) -> Result<Domain> {
    let prompt = format!("Please categorize this domain with a single keyword in English. \
            Do not elaborate, do not explain or otherwise enhance the answer. \
            The domain is: {domain}. Here are some items from the website: {text}");

    let response = llm_completion(&prompt).await?;
    Ok(Domain {
        domain: domain.to_string(),
        category: response,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load the domains
    let mut domains = load_asn_domains()?;

    // Shuffle the domains (so in test runs we aren't always hitting the same ones)
    domains.shuffle(&mut rand::thread_rng());

    // Create the channels for results
    let report_success = success().await;
    let report_failures = failures().await;

    // Create a big set of tasks
    let mut futures = Vec::new();
    for domain in domains.into_iter().take(10) {
        // Clone the channels - they are designed for this.
        let my_success = report_success.clone();
        let my_failure = report_failures.clone();
        let future = tokio::spawn(async move {
            match website_text(&domain).await {
                Ok(text) => {
                    match categorize_domain(&domain, &text).await {
                        Ok(domain) => { let _ = my_success.send(domain).await; },
                        Err(_) => { let _ = my_failure.send(domain).await; },
                    }
                }
                Err(_) => {
                    let _ = my_failure.send(domain).await;
                }
            }
        });
        futures.push(future);
    }

    // Await completion of all tasks
    join_all(futures).await;
    Ok(())
}
