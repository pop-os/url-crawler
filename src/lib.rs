#[macro_use]
extern crate bitflags;
extern crate chrono;
extern crate crossbeam_channel;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate reqwest;
extern crate url_scraper;

pub use reqwest::{Url, header};
use reqwest::header::*;
use chrono::{DateTime, FixedOffset};
use crossbeam_channel as channel;
use reqwest::Client;
use self::scraper::Scraper;
use std::thread;
use url_scraper::UrlScraper;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

bitflags! {
    /// Flags for controlling the behavior of the crawler.
    pub struct Flags: u8 {
        /// Enable crawling across domains.
        const CROSS_DOMAIN = 1;
        /// Enable crawling outside of the specified directory.
        const CROSS_DIR = 2;
    }
}

/// A configurable web crawler.
pub struct Crawler {
    url: String,
    threads: usize,
    flags: Flags,
    pre_fetch: fn(&Url) -> bool,
    post_fetch: fn(&Url, &HeaderMap) -> bool,
}

impl Crawler {
    /// Initializes a new crawler with a default thread count of `4`.
    pub fn new(url: String) -> Self {
        Crawler {
            url,
            threads: 4,
            flags: Flags::empty(),
            pre_fetch: |_| true,
            post_fetch: |_, _| true,
        }
    }

    /// Disables cross-domain crawling.
    pub fn flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// Specifies the number of fetcher threads to use.
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = if threads == 0 { 1 } else { threads };
        self
    }

    /// Enables filtering items based on their filename.
    pub fn pre_fetch(mut self, pre_fetch: fn(url: &Url) -> bool) -> Self {
        self.pre_fetch = pre_fetch;
        self
    }

    /// Enables filtering items based on their filename and requested headers.
    pub fn post_fetch(mut self, post_fetch: fn(url: &Url, headers: &HeaderMap) -> bool) -> Self {
        self.post_fetch = post_fetch;
        self
    }

    /// Initializes the crawling, returning an iterator of discovered files.
    pub fn crawl(self) -> impl Iterator<Item = UrlEntry> {
        let client_ = Arc::new(Client::new());

        let threads = self.threads;
        let pre_fetch = self.pre_fetch;
        let post_fetch = self.post_fetch;
        let flags = self.flags;
        let (scraper_tx, scraper_rx) = channel::unbounded::<String>();
        let (fetcher_tx, fetcher_rx) = channel::bounded::<Url>(threads * 4);
        let (compare_tx, compare_rx) = channel::bounded::<UrlEntry>(threads * 4);
        let state = Arc::new(AtomicUsize::new(4));
        scraper_tx.send(self.url);

        // Thread for scraping urls and avoiding repeats.
        let status = state.clone();
        thread::spawn(move || {
            let mut visited = Vec::new();
            loop {
                let url: String = match scraper_rx.try_recv() {
                    Some(url) => url,
                    None => {
                        if status.load(Ordering::SeqCst) == threads && scraper_rx.is_empty() {
                            break
                        }

                        thread::sleep(Duration::from_millis(1));
                        continue
                    }
                };

                if let Ok(scraper) = UrlScraper::new(&url) {
                    for url in Scraper::new(scraper.into_iter(), &url, &mut visited, flags) {
                        fetcher_tx.send(url);
                    }
                }
            }
        });

        for _ in 0..threads {
            let fetcher = fetcher_rx.clone();
            let client = client_.clone();
            let scraper_tx = scraper_tx.clone();
            let compare_tx = compare_tx.clone();
            let status = state.clone();
            thread::spawn(move || {
                for url in fetcher {
                    status.fetch_sub(1, Ordering::SeqCst);
                    let head = match client.head(url.clone()).send() {
                        Ok(head) => head,
                        _ => {
                            status.fetch_add(1, Ordering::SeqCst);
                            continue
                        }
                    };

                    if ! (pre_fetch)(&url) {
                        status.fetch_add(1, Ordering::SeqCst);
                        continue
                    }

                    let headers = head.headers();

                    if ! (post_fetch)(&url, head.headers()) {
                        status.fetch_add(1, Ordering::SeqCst);
                        continue
                    }

                    if let Some(content_type) = head.headers().get(CONTENT_TYPE).and_then(|c| c.to_str().ok()) {
                        if content_type.starts_with("text/html") {
                            scraper_tx.send(url.to_string());
                            compare_tx.send(UrlEntry::Html { url });
                        } else {
                            let length: u64 = headers.get(CONTENT_LENGTH)
                                .and_then(|c| c.to_str().ok())
                                .and_then(|c| c.parse().ok())
                                .unwrap_or(0);

                            let modified = headers.get(LAST_MODIFIED)
                                .and_then(|c| c.to_str().ok())
                                .and_then(|c| DateTime::parse_from_rfc2822(c).ok());

                            compare_tx.send(UrlEntry::File { url, length, modified });
                        }
                    }

                    status.fetch_add(1, Ordering::SeqCst);
                }
            });
        }

        compare_rx.into_iter()
    }
}

#[derive(Debug)]
/// URLs discovered found by the web crawler
pub enum UrlEntry {
    /// A URL with the "text/html" content type
    Html { url: Url },
    /// All other detected content.
    File { url: Url, length: u64, modified: Option<DateTime<FixedOffset>> }
}

/// Convenience function for getting the filename from a URL.
pub fn filename_from_url(url: &str) -> &str {
    if url.len() < 2 {
        url
    } else {
        &url[url.rfind('/').unwrap_or(0)+1..]
    }
}

mod scraper {
    use reqwest::Url;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use url_scraper::UrlIter;
    use super::Flags;

    pub struct Scraper<'a> {
        iter: UrlIter<'a, 'a>,
        url: Url,
        visited: &'a mut Vec<u64>,
        flags: Flags
    }

    impl<'a> Scraper<'a> {
        pub fn new(iter: UrlIter<'a, 'a>, url: &'a str, visited: &'a mut Vec<u64>, flags: Flags) -> Self {
            Self { iter, url: Url::parse(url).unwrap(), visited, flags }
        }
    }

    impl<'a> Iterator for Scraper<'a> {
        type Item = Url;

        fn next(&mut self) -> Option<Self::Item> {
            for (_, url) in &mut self.iter {
                if ! self.flags.contains(Flags::CROSS_DOMAIN) {
                    if url.domain() != self.url.domain() {
                        continue
                    }
                }

                if ! self.flags.contains(Flags::CROSS_DIR) {
                    if ! url.path().starts_with(self.url.path()) {
                        continue
                    }
                }

                let mut hasher = DefaultHasher::new();
                url.as_str().hash(&mut hasher);
                let hash = hasher.finish();

                if self.visited.contains(&hash) {
                    continue
                }
                self.visited.push(hash);

                return Some(url);
            }

            None
        }
    }
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "error while scraping a page: {}", why)]
    Scraper { why: url_scraper::Error },
    #[fail(display = "error while requesting content: {}", why)]
    Request { why: reqwest::Error }
}

impl From<url_scraper::Error> for Error {
    fn from(why: url_scraper::Error) -> Error {
        Error::Scraper { why }
    }
}

impl From<reqwest::Error> for Error {
    fn from(why: reqwest::Error) -> Error {
        Error::Request { why }
    }
}
