// Project: shoptet-scraper
// File: network.rs
// Description: Retrying HTTP sessions with redirect and peer checks.
// Author: Jan Alexandr Kopřiva
// License: AGPL-3.0-or-later
use crate::{
    security::{is_blocked_ip, is_safe_url, read_limited, MAX_IMAGE_DOWNLOAD_BYTES},
    Result,
};
use reqwest::{
    blocking::{Client, Response},
    header, Method, Url,
};
use std::{
    io::Read,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};
type PeerPolicy = Arc<dyn Fn(IpAddr) -> bool + Send + Sync>;
/// Resolves names itself and drops refused addresses, so the connection is made
/// to an address that passed the check (a second lookup cannot swap it).
struct GuardedResolver(PeerPolicy);
impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let peer = Arc::clone(&self.0);
        Box::pin(async move {
            let allowed: Vec<SocketAddr> = (name.as_str(), 0)
                .to_socket_addrs()?
                .filter(|a| peer(a.ip()))
                .collect();
            if allowed.is_empty() {
                return Err("Refused unsafe peer address".into());
            }
            Ok(Box::new(allowed.into_iter()) as reqwest::dns::Addrs)
        })
    }
}
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
    peer_policy: PeerPolicy,
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
        fn client(ua: &str, peer: &PeerPolicy) -> Result<Client> {
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
                .dns_resolver(Arc::new(GuardedResolver(Arc::clone(peer))))
                .build()?)
        }
        let peer: PeerPolicy = Arc::new(peer);
        Ok(Self {
            browser: client(BROWSER_UA, &peer)?,
            feed: client(REQUESTS_UA, &peer)?,
            policy: Arc::new(url),
            peer_policy: peer,
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
                            let delay = r
                                .headers()
                                .get(header::RETRY_AFTER)
                                .and_then(|v| v.to_str().ok())
                                .and_then(retry_after)
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
        let mut remaining = data.as_slice();
        let mut text = String::new();
        while !remaining.is_empty() {
            match std::str::from_utf8(remaining) {
                Ok(valid) => {
                    text.push_str(valid);
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    text.push_str(std::str::from_utf8(&remaining[..valid])?);
                    // Python's discovery preview decodes with errors="ignore".
                    let skipped = error.error_len().unwrap_or(remaining.len() - valid);
                    remaining = &remaining[valid + skipped..];
                }
            }
        }
        Ok(text)
    }
}
pub fn backoff(attempt: usize, factor: f64) -> Duration {
    if attempt == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64((factor * 2f64.powi(attempt as i32)).min(120.0))
    }
}
// A server-controlled Retry-After must not suspend a worker for years.
pub fn retry_after(value: &str) -> Option<Duration> {
    if value.bytes().all(|b| b.is_ascii_digit()) && !value.is_empty() {
        return Some(Duration::from_secs(
            value.parse::<u64>().unwrap_or(u64::MAX).min(120),
        ));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    Some(Duration::from_secs(
        date.timestamp()
            .saturating_sub(chrono::Utc::now().timestamp())
            .clamp(0, 120) as u64,
    ))
}
