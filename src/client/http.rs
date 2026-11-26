use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};
use tracing::warn;

const MAX_RETRIES: u32 = 4;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

pub fn new_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

pub async fn send_with_retry(req: RequestBuilder) -> reqwest::Result<Response> {
    let mut attempt = 0;
    loop {
        let Some(this_try) = req.try_clone() else {
            return req.send().await;
        };
        let resp = this_try.send().await?;
        if resp.status() != StatusCode::TOO_MANY_REQUESTS || attempt >= MAX_RETRIES {
            return Ok(resp);
        }

        let delay = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(1 << attempt))
            .min(MAX_RETRY_DELAY);
        warn!(
            "Throttled by {} (attempt {}), retrying in {delay:?}",
            resp.url().path(),
            attempt + 1
        );
        tokio::time::sleep(delay).await;
        attempt += 1;
    }
}
