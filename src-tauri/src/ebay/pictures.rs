//! A listing's picture, fetched for an AI agent to hold against the card's scan (`next_cards_to_price`,
//! `listing_pictures`).
//!
//! The address is one eBay's search answered with and Cardloader stored, but what is stored isn't trusted to stay
//! that: only an https address on eBay's picture host is fetched, without following redirects, within a time and
//! a size. So an address that got into the database another way can't make the app call something on the user's
//! network for an agent to read.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Url;

/// eBay's picture host; pictures are on its subdomains (`i.ebayimg.com`).
const HOST: &str = "ebayimg.com";
/// The long edges eBay serves a picture at, as `s-l<edge>` in its address.
const SIZES: [u32; 8] = [225, 300, 400, 500, 640, 960, 1200, 1600];
const TIMEOUT: Duration = Duration::from_secs(8);
/// A picture larger than this isn't a listing's (eBay's largest are a few hundred kB).
const MAX_BYTES: usize = 3 * 1024 * 1024;

/// An https address on eBay's picture host.
pub fn is_ebay_picture(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str()
                .is_some_and(|h| h == HOST || h.ends_with(&format!(".{HOST}")))
    })
}

/// Tests serve pictures from a mock on this computer; a release fetches from eBay alone.
fn allowed(url: &str) -> bool {
    let local =
        || Url::parse(url).is_ok_and(|u| u.scheme() == "http" && u.host_str() == Some("127.0.0.1"));
    is_ebay_picture(url) || (cfg!(test) && local())
}

/// The picture's address at the smallest size eBay serves that is at least `edge` (its addresses end in
/// `s-l225.jpg`, `s-l1600.webp`, …); `None` for an address not written that way. That eBay serves every size of
/// every picture isn't documented: a caller falls back to the address it has.
pub fn sized(url: &str, edge: u32) -> Option<String> {
    let at = url.rfind("/s-l")? + "/s-l".len();
    let digits = url[at..].chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || !url[at + digits..].starts_with('.') {
        return None;
    }
    let size = SIZES
        .iter()
        .find(|s| **s >= edge)
        .unwrap_or(&SIZES[SIZES.len() - 1]);
    Some(format!("{}{size}{}", &url[..at], &url[at + digits..]))
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(crate::ximilar::http::USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(TIMEOUT)
            .timeout(TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

async fn get(url: &str) -> Result<Vec<u8>, String> {
    if !allowed(url) {
        return Err("not a picture on eBay".into());
    }
    let mut response = http()
        .get(url)
        .send()
        .await
        .map_err(|_| "eBay's picture couldn't be loaded".to_string())?;
    if !response.status().is_success() {
        return Err(format!("eBay answered {}", response.status().as_u16()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "eBay's picture couldn't be loaded".to_string())?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("the picture is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// The listing's picture at about `edge` pixels: the size asked of eBay first, else the picture as stored.
pub async fn fetch(url: &str, edge: u32) -> Result<Vec<u8>, String> {
    if let Some(sized) = sized(url, edge).filter(|s| s != url) {
        if let Ok(bytes) = get(&sized).await {
            return Ok(bytes);
        }
    }
    get(url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ebays_pictures_are_fetched() {
        assert!(is_ebay_picture(
            "https://i.ebayimg.com/images/g/abc/s-l225.jpg"
        ));
        assert!(is_ebay_picture("https://ebayimg.com/1.jpg"));
        for other in [
            "http://i.ebayimg.com/images/g/abc/s-l225.jpg",
            "https://i.ebayimg.com.evil.example/1.jpg",
            "https://evilebayimg.com/1.jpg",
            "https://192.168.1.1/admin",
            "file:///etc/passwd",
            "not an address",
        ] {
            assert!(!is_ebay_picture(other), "{other}");
        }
    }

    #[test]
    fn asks_ebay_for_the_size_wanted() {
        let stored = "https://i.ebayimg.com/images/g/abc/s-l225.jpg";
        assert_eq!(
            sized(stored, 800).as_deref(),
            Some("https://i.ebayimg.com/images/g/abc/s-l960.jpg")
        );
        assert_eq!(sized(stored, 200).as_deref(), Some(stored));
        assert_eq!(
            sized("https://i.ebayimg.com/images/g/abc/s-l1600.webp", 5000).as_deref(),
            Some("https://i.ebayimg.com/images/g/abc/s-l1600.webp")
        );
        assert_eq!(sized("https://i.ebayimg.com/1.jpg", 800), None);
        assert_eq!(sized("https://i.ebayimg.com/s-large.jpg", 800), None);
    }
}
