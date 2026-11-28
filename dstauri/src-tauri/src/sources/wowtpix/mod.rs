//! wowtpix.com — WordPress "anime" theme catalog source.
//!
//! The domain currently returns NXDOMAIN (verified 2026-11-28 with both
//! dns.google and cloudflare-dns.com), so the parsers are modelled on the
//! live sibling sites that use the same theme. No player URL is persisted:
//! episode pages are resolved at download/play time. Both browse and resolver
//! logic are pure functions over HTML, with synthetic fixtures standing in
//! until the domain resolves again.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "wowtpix";
pub const BASE: &str = "https://wowtpix.com";

/// Accept the apex host and its subdomains (e.g. `www.wowtpix.com`).
pub fn is_wowtpix_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host == "wowtpix.com" || host.ends_with(".wowtpix.com")
}

#[cfg(test)]
mod tests {
    use super::is_wowtpix_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_wowtpix_host("wowtpix.com"));
        assert!(is_wowtpix_host("www.wowtpix.com"));
        assert!(is_wowtpix_host("WWW.WOWTPIX.COM."));
        assert!(!is_wowtpix_host("notwowtpix.com"));
        assert!(!is_wowtpix_host("wowtpix.com.example.org"));
    }
}
