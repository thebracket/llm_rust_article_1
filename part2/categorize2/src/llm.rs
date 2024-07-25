use serde::Deserialize;
use serde_json::json;

const LLM_API: &str = "http://localhost:11434/api/generate";

#[derive(Deserialize)]
struct Response {
    response: String,
}

pub async fn llm_completion(prompt: &str) -> anyhow::Result<String> {
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