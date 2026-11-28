//! dongsub.net — Blogger-based donghua catalog with Dailymotion embeds.
//!
//! Series live at `/YYYY/MM/<slug>.html` and list their episodes through a
//! Blogger label (`var label_episode = "…"` in the post body). Episode pages
//! put the actual player URL in `<option value="https://geo.dailymotion.com/…">`
//! inside `<select id="selectServ">`; the resolver picks that at play time.

pub mod browse;
pub mod resolver;

pub use browse::{catalog, filters, series};
pub use resolver::resolve;

pub const SOURCE_KEY: &str = "dongsub";
pub const BASE: &str = "https://www.dongsub.net";

/// Accept the apex host and any subdomain (e.g. `dongsub.net`, `www.dongsub.net`).
pub fn is_dongsub_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == "dongsub.net" || host.ends_with(".dongsub.net")
}

#[cfg(test)]
mod tests {
    use super::is_dongsub_host;

    #[test]
    fn apex_and_www_hosts_are_recognised() {
        assert!(is_dongsub_host("dongsub.net"));
        assert!(is_dongsub_host("www.dongsub.net"));
        assert!(is_dongsub_host("blog.dongsub.net"));
        assert!(!is_dongsub_host("notdongsub.net"));
        assert!(!is_dongsub_host("donghuastream.org"));
    }
}
