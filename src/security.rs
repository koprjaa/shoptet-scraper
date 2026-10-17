// Project: shoptet-scraper
// File: security.rs
// Description: URL, address, XML and spreadsheet trust boundaries.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use crate::Result;
use std::{
    io::Read,
    net::{IpAddr, ToSocketAddrs},
};
pub const MAX_IMAGE_DOWNLOAD_BYTES: usize = 30 * 1024 * 1024;
pub fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|c| if "\\/*?:\"<>|".contains(c) { '_' } else { c })
        .collect()
}
pub fn sanitize_xlsx_value(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            let s = ip.segments();
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (s[0] & 0xe000 != 0x2000)
                || (s[0] == 0x2001 && s[1] < 0x200)
                || (s[0] == 0x2001 && s[1] == 0xdb8)
                || (s[0] & 0xfff0 == 0x3ff0)
        }
    }
}
pub fn safe_resolved_url(url: &str, resolve: impl FnOnce(&str, u16) -> Vec<IpAddr>) -> bool {
    if url
        .split_once("://")
        .is_none_or(|(_, rest)| rest.is_empty() || rest.starts_with('/'))
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let addresses = resolve(
        host.trim_matches(['[', ']']),
        url.port_or_known_default().unwrap_or(80),
    );
    !addresses.is_empty() && addresses.into_iter().all(|ip| !is_blocked_ip(ip))
}
pub fn is_safe_url(url: &str) -> bool {
    safe_resolved_url(url, |host, port| {
        (host, port)
            .to_socket_addrs()
            .map(|a| a.map(|a| a.ip()).collect())
            .unwrap_or_default()
    })
}
pub fn read_limited(
    mut reader: impl Read,
    declared: Option<&str>,
    limit: usize,
) -> Result<Vec<u8>> {
    if let Some(n) = declared
        .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        .map(|s| s.parse::<u64>().unwrap_or(u64::MAX))
    {
        if n > limit as u64 {
            return Err(format!("image declares {n} bytes, over the limit").into());
        }
    }
    let mut output = vec![];
    let mut buf = [0; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(output);
        }
        if output.len() + n > limit {
            return Err("image went over the size limit while downloading".into());
        }
        output.extend_from_slice(&buf[..n]);
    }
}

pub fn safe_peer(peer: Option<IpAddr>) -> bool {
    peer.is_none_or(|ip| !is_blocked_ip(ip))
}
