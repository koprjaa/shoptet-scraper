// Project: shoptet-scraper
// File: network.rs
// Description: Retrying HTTP sessions with redirect and peer checks.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use crate::{
    security::{is_blocked_ip, is_safe_url, read_limited, MAX_IMAGE_DOWNLOAD_BYTES},
    Result,
};
use reqwest::{
    blocking::{Client, Response},
    header, Method, Url,
};
use std::{io::Read, sync::Arc, time::Duration};
pub const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
pub const REQUESTS_UA: &str = "python-requests/2.33.0";
#[derive(Clone, Copy)]
pub enum Session {
    Discovery,
    Feed,
}
#[derive(Clone)]
pub struct Http {
    browser: Client,
    feed: Client,
    policy: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    peer_policy: Arc<dyn Fn(std::net::IpAddr) -> bool + Send + Sync>,
}
impl Http {
    pub fn new() -> Result<Self> {
        Self::with_policy(is_safe_url, |ip| !is_blocked_ip(ip))
    }
    // Explicit dependency injection keeps offline tests independent of public DNS.
    pub fn with_policy(
        url: impl Fn(&str) -> bool + Send + Sync + 'static,
        peer: impl Fn(std::net::IpAddr) -> bool + Send + Sync + 'static,
    ) -> Result<Self> {
        fn client(ua: &str) -> Result<Client> {
            let mut headers = header::HeaderMap::new();
            headers.insert(header::ACCEPT, header::HeaderValue::from_static("*/*"));
            headers.insert(
                header::CONNECTION,
                header::HeaderValue::from_static("keep-alive"),
            );
            Ok(Client::builder()
                .user_agent(ua)
                .default_headers(headers)
                .cookie_store(true)
                .pool_max_idle_per_host(100)
                .redirect(reqwest::redirect::Policy::none())
                .build()?)
        }
        Ok(Self {
            browser: client(BROWSER_UA)?,
            feed: client(REQUESTS_UA)?,
            policy: Arc::new(url),
            peer_policy: Arc::new(peer),
        })
    }
    pub fn fresh(&self) -> Result<Self> {
        let url = Arc::clone(&self.policy);
        let peer = Arc::clone(&self.peer_policy);
        Self::with_policy(move |u| url(u), move |ip| peer(ip))
    }
    pub fn safe(&self, url: &str) -> bool {
        (self.policy)(url)
    }
    pub fn request(
        &self,
        method: Method,
        url: &str,
        timeout: u64,
        session: Session,
    ) -> Result<Response> {
        let (client, retries, factor, statuses): (_, usize, f64, &[u16]) = match session {
            Session::Discovery => (&self.browser, 3, 1.0, &[500, 502, 504, 408, 429]),
            Session::Feed => (&self.feed, 5, 0.5, &[500, 502, 504]),
        };
        let mut current = Url::parse(url)?;
        for redirect in 0..=30 {
            if !(self.policy)(current.as_str()) {
                return Err(format!("Refused unsafe URL: {current}").into());
            }
            let mut response = None;
            for attempt in 0..=retries {
                match client
                    .request(method.clone(), current.clone())
                    .timeout(Duration::from_secs(timeout))
                    .send()
                {
                    Ok(r) => {
                        if r.remote_addr().is_some_and(|a| !(self.peer_policy)(a.ip())) {
                            return Err("Refused unsafe peer address".into());
                        }
                        let retry = statuses.contains(&r.status().as_u16())
                            || (r.headers().contains_key(header::RETRY_AFTER)
                                && matches!(r.status().as_u16(), 413 | 429 | 503));
                        if retry && attempt < retries {
                            let delay = retry_after(&r)
                                .filter(|d| !d.is_zero())
                                .unwrap_or_else(|| backoff(attempt, factor));
                            drop(r);
                            std::thread::sleep(delay);
                        } else {
                            response = Some(r);
                            break;
                        }
                    }
                    Err(e) => {
                        if attempt == retries {
                            return Err(e.into());
                        }
                        std::thread::sleep(backoff(attempt, factor));
                    }
                }
            }
            let r = response.ok_or("Request exhausted retries")?;
            if method != Method::HEAD && matches!(r.status().as_u16(), 301 | 302 | 303 | 307 | 308)
            {
                if let Some(location) = r.headers().get(header::LOCATION) {
                    if redirect == 30 {
                        return Err("Exceeded 30 redirects".into());
                    }
                    current = current.join(location.to_str()?)?;
                    continue;
                }
            }
            return Ok(r);
        }
        Err("Exceeded 30 redirects".into())
    }
    pub fn bytes(&self, url: &str, timeout: u64, session: Session) -> Result<Vec<u8>> {
        Ok(self
            .request(Method::GET, url, timeout, session)?
            .error_for_status()?
            .bytes()?
            .to_vec())
    }
    pub fn image(&self, url: &str) -> Result<Option<Vec<u8>>> {
        if !self.safe(url) {
            return Ok(None);
        }
        let r = self
            .request(Method::GET, url, 10, Session::Feed)?
            .error_for_status()?;
        let declared = r
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        Ok(Some(read_limited(
            r,
            declared.as_deref(),
            MAX_IMAGE_DOWNLOAD_BYTES,
        )?))
    }
    pub fn preview(&self, mut r: Response) -> Result<String> {
        let mut data = vec![];
        Read::take(&mut r, 500).read_to_end(&mut data)?;
        Ok(String::from_utf8_lossy(&data).into_owned())
    }
}
pub fn backoff(attempt: usize, factor: f64) -> Duration {
    if attempt == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64((factor * 2f64.powi(attempt as i32)).min(120.0))
    }
}
fn retry_after(r: &Response) -> Option<Duration> {
    let value = r.headers().get(header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(n) = value.parse::<u64>() {
        return Some(Duration::from_secs(n));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    Some(Duration::from_secs(
        (date.timestamp() - chrono::Utc::now().timestamp()).max(0) as u64,
    ))
}
