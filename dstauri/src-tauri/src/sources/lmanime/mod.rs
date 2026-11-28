//! lmanime.com — WordPress donghua catalog with Dailymotion embeds.
//!
//! The theme is the same one animekhor.org uses: `<article class="bs">`
//! poster cards, `<div class="eplister">` episode lists and `<div class="spe">`
//! metadata. Playback is resolved from the episode page's `<iframe>` (or
//! mirror `<option>`) at play/download time.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "lmanime";
pub const BASE: &str = "https://lmanime.com";

/// Accept the apex host and any subdomain (e.g. `www.lmanime.com`).
pub fn is_lmanime_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == "lmanime.com" || host.ends_with(".lmanime.com")
}

#[cfg(test)]
mod tests {
    use super::is_lmanime_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_lmanime_host("lmanime.com"));
        assert!(is_lmanime_host("www.lmanime.com"));
        assert!(!is_lmanime_host("notlmanime.com"));
        assert!(!is_lmanime_host("animekhor.org"));
    }
}
