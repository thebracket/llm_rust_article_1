use std::time::Duration;
use itertools::Itertools;
use reqwest::header;
use scraper::Html;

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

pub async fn website_text(domain: &str) -> anyhow::Result<String> {
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
        .timeout(Duration::from_secs(30))
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
        .map(|(_count, word)| word)// Take only the word
        .take(100)// Take the top 100 words
        .join(" "); // Join them into a string

    Ok(result)
}