//! animexin.dev — WordPress "anime" theme catalog source.
//!
//! Series pages live at `/<slug>/` (though `/anime/<slug>/` also resolves),
//! episode pages embed Dailymotion or base64-encoded mirror options. Parsing
//! is regex-based and pure; the resolver runs only at play/download time.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "animexin";
pub const BASE: &str = "https://animexin.dev";

/// Accept the apex host and subdomains (e.g. `www.animexin.dev`).
pub fn is_animexin_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    host == "animexin.dev" || host.ends_with(".animexin.dev")
}

#[cfg(test)]
mod tests {
    use super::is_animexin_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_animexin_host("animexin.dev"));
        assert!(is_animexin_host("www.animexin.dev"));
        assert!(!is_animexin_host("animexin.com"));
        assert!(!is_animexin_host("animexin.dev.example.org"));
    }
}
