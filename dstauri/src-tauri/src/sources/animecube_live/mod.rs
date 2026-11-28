//! animecube.live — the current, live AnimeCube site.
//!
//! The 1.1.2 `animecube` module ports the old `/anime/<code>?episode=&season=`
//! site and its AES-256-GCM source registry. The live site no longer serves
//! that registry (`/api/anime-sources-versions` answers HTTP 410) and uses
//! `/catalog`, `/api/search`, `/watch/<episodeCode>` and
//! `/api/watch/<episodeCode>/context` instead. This module is a fresh parser
//! for that layout, kept separate so the old port is not lost.
//!
//! All parsing is pure and regex/JSON based; all I/O goes through the shared
//! `fetch_text`. Playback sources are resolved at play/download time and are
//! never persisted.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "animecube_live";
pub const BASE: &str = "https://animecube.live";

/// Accept the apex host and any subdomain (e.g. `www.animecube.live`).
pub fn is_animecube_live_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == "animecube.live" || host.ends_with(".animecube.live")
}

#[cfg(test)]
mod tests {
    use super::is_animecube_live_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_animecube_live_host("animecube.live"));
        assert!(is_animecube_live_host("www.animecube.live"));
        assert!(is_animecube_live_host("cdn.animecube.live"));
        assert!(!is_animecube_live_host("notanimecube.live"));
        assert!(!is_animecube_live_host(" animekhor.org "));
    }
}
