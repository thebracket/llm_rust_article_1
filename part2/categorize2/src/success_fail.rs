use tokio::sync::mpsc::Sender;

pub struct Domain {
    pub domain: String,
    pub category: String,
}

async fn append_to_file(filename: &str, line: &str) -> anyhow::Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, format!("{}\n", line).as_bytes()).await?;
    Ok(())
}

pub async fn failures() -> Sender<String> {
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

pub async fn success() -> Sender<Domain> {
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