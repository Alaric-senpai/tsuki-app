//! Lucifer Donghua — one logical source with two hosts:
//! `luciferdonghua.in` (canonical) and `misterdonghua.com` (alias).
//!
//! Both sites run the same WordPress "anime" theme and publish the same
//! catalogs (often with host-specific slugs), so they share one `SOURCE_KEY`
//! and one parser. Episode pages are resolved whenever playback/download
//! starts; no resolved URL is ever written to the library.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "luciferdonghua";
pub const BASE: &str = "https://luciferdonghua.in";
pub const MISTER_BASE: &str = "https://misterdonghua.com";

/// Both apex domains, their `www` subdomains and any further subdomains.
pub fn is_luciferdonghua_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host == "luciferdonghua.in"
        || host.ends_with(".luciferdonghua.in")
        || host == "misterdonghua.com"
        || host.ends_with(".misterdonghua.com")
}

#[cfg(test)]
mod tests {
    use super::is_luciferdonghua_host;

    #[test]
    fn both_host_aliases_are_recognised() {
        assert!(is_luciferdonghua_host("luciferdonghua.in"));
        assert!(is_luciferdonghua_host("www.luciferdonghua.in"));
        assert!(is_luciferdonghua_host("misterdonghua.com"));
        assert!(is_luciferdonghua_host("WWW.MISTERDONGHUA.COM."));
        assert!(!is_luciferdonghua_host("luciferdonghua.com"));
        assert!(!is_luciferdonghua_host("misterdonghua.com.example.org"));
    }
}
