//! animekhor.org — the live catalog source that replaced AnimeCube.
//!
//! WordPress pages are parsed with regexes (matching the legacy parser style),
//! and playback is resolved to Dailymotion at download/play time.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "animekhor";
pub const BASE: &str = "https://animekhor.org";

/// Accept the apex host and any subdomain (e.g. `www.animekhor.org`).
pub fn is_animekhor_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == "animekhor.org" || host.ends_with(".animekhor.org")
}

#[cfg(test)]
mod tests {
    use super::is_animekhor_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_animekhor_host("animekhor.org"));
        assert!(is_animekhor_host("www.animekhor.org"));
        assert!(!is_animekhor_host("notanimekhor.org"));
    }
}
