//! Configurable parallel web crawler
//! 
//! # Example
//! 
//! ```rust,no_run
//! extern crate url_crawler;
//! use url_crawler::*;
//! 
//! /// Function for filtering content in the crawler before a HEAD request.
//! /// 
//! /// Only allow directory entries, and files that have the `deb` extension.
//! fn apt_filter(url: &Url) -> bool {
//!     let url = url.as_str();
//!     url.ends_with("/") || url.ends_with(".deb")
//! }
//! 
//! pub fn main() {
//!     // Create a crawler designed to crawl the given website.
//!     let crawler = Crawler::new("http://apt.pop-os.org/".into())
//!         // Use four threads for fetching
//!         .threads(4)
//!         // Check if a URL matches this filter before performing a HEAD request on it.
//!         .pre_fetch(apt_filter)
//!         // Initialize the crawler and begin crawling. This returns immediately.
//!         .crawl();
//! 
//!     // Process url entries as they become available
//!     for file in crawler {
//!         println!("{:#?}", file);
//!     }
//! }
//! ```

#[macro_use]
extern crate bitflags;
extern crate chrono;
extern crate crossbeam_channel;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate reqwest;
extern crate url_scraper;

mod scraper;

pub use reqwest::{Url, header};
use chrono::{DateTime, FixedOffset};
use crossbeam_channel as channel;
use channel::Receiver;
use reqwest::Client;
use reqwest::header::*;
use scraper::Scraper;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use url_scraper::UrlScraper;

bitflags! {
    /// Flags for controlling the behavior of the crawler.
    pub struct Flags: u8 {
        /// Enable crawling across domains.
        const CROSS_DOMAIN = 1;
        /// Enable crawling outside of the specified directory.
        const CROSS_DIR = 2;
    }
}

/// A configurable parallel web crawler.
/// 
/// Crawling does not occur until this type is consumed by the `crawl` method.
pub struct Crawler {
    url: String,
    threads: usize,
    flags: Flags,
    errors: fn(Error) -> bool,
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
            errors: |_| true,
            pre_fetch: |_| true,
            post_fetch: |_, _| true,
        }
    }

    /// Set flags for configuring the crawler.
    pub fn flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// Specifies the number of fetcher threads to use.
    /// 
    /// # Notes
    /// - If the input is `0`, `1` thread will be used.
    /// - The default thread count is `4` when not using this method.
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = if threads == 0 { 1 } else { threads };
        self
    }

    /// Allow the caller to handle errors.
    /// 
    /// # Notes
    /// Returning `false` will stop the crawler.
    pub fn errors(mut self, errors: fn(error: Error) -> bool) -> Self {
        self.errors = errors;
        self
    }

    /// Enables filtering items based on their filename.
    /// 
    /// # Notes
    /// Returning `false` will prevent the item from being fetched.
    pub fn pre_fetch(mut self, pre_fetch: fn(url: &Url) -> bool) -> Self {
        self.pre_fetch = pre_fetch;
        self
    }

    /// Enables filtering items based on their filename and requested headers.
    /// 
    /// # Notes
    /// Returning `false` will prevent the item from being scraped / returned.
    pub fn post_fetch(mut self, post_fetch: fn(url: &Url, headers: &HeaderMap) -> bool) -> Self {
        self.post_fetch = post_fetch;
        self
    }

    /// Initializes the crawling, returning an iterator of discovered files.
    /// 
    /// The crawler will continue to crawl in background threads even while the iterator
    /// is not being pulled from.
    pub fn crawl(self) -> CrawlIter {
        let client_ = Arc::new(Client::new());

        let threads = self.threads;
        let pre_fetch = self.pre_fetch;
        let post_fetch = self.post_fetch;
        let errors = self.errors;
        let flags = self.flags;
        let (scraper_tx, scraper_rx) = channel::unbounded::<String>();
        let (fetcher_tx, fetcher_rx) = channel::bounded::<Url>(threads * 4);
        let (output_tx, output_rx) = channel::bounded::<UrlEntry>(threads * 4);
        let state = Arc::new(AtomicUsize::new(0));
        let kill = Arc::new(AtomicBool::new(false));
        scraper_tx.send(self.url);

        for _ in 0..threads {
            let fetcher = fetcher_rx.clone();
            let client = client_.clone();
            let scraper_tx = scraper_tx.clone();
            let output_tx = output_tx.clone();
            let status = state.clone();
            let kill = kill.clone();
            thread::spawn(move || {
                status.fetch_add(1, Ordering::SeqCst);
                for url in fetcher {
                    status.fetch_sub(1, Ordering::SeqCst);
                    if kill.load(Ordering::SeqCst) {
                        break
                    }

                    let head = match client.head(url.clone()).send() {
                        Ok(head) => head,
                        Err(why) => {
                            if ! errors(why.into()) {
                                kill.store(true, Ordering::SeqCst);
                            }
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
                            output_tx.send(UrlEntry::Html { url });
                        } else {
                            let length: u64 = headers.get(CONTENT_LENGTH)
                                .and_then(|c| c.to_str().ok())
                                .and_then(|c| c.parse().ok())
                                .unwrap_or(0);

                            let modified = headers.get(LAST_MODIFIED)
                                .and_then(|c| c.to_str().ok())
                                .and_then(|c| DateTime::parse_from_rfc2822(c).ok());

                            output_tx.send(UrlEntry::File { url, length, modified });
                        }
                    }

                    status.fetch_add(1, Ordering::SeqCst);
                }
            });
        }

        // Thread for scraping urls and avoiding repeats.
        let state_ = state.clone();
        let client = client_.clone();
        let kill_ = kill.clone();
        thread::spawn(move || {
            let mut visited = Vec::new();
            let jobs_complete = || {
                state_.load(Ordering::SeqCst) == threads
                    && scraper_rx.is_empty()
                    && fetcher_tx.is_empty()
            };

            while ! kill_.load(Ordering::SeqCst) {
                let url: String = match scraper_rx.try_recv() {
                    Some(url) => url,
                    None => {
                        if jobs_complete() { break }
                        thread::sleep(Duration::from_millis(1));
                        continue
                    }
                };

                match UrlScraper::new_with_client(&url, &client) {
                    Ok(scraper) => for url in Scraper::new(scraper.into_iter(), &url, &mut visited, flags) {
                        fetcher_tx.send(url);
                    }
                    Err(why) => if ! errors(why.into()) {
                        kill_.store(true, Ordering::SeqCst);
                    }
                }
            }
        });

        CrawlIter {
            recv: output_rx,
            kill
        }
    }
}

/// Iterator that returns fetched `UrlEntry` items.
/// 
/// On drop, the crawler's threads will be killed.
pub struct CrawlIter {
    recv: Receiver<UrlEntry>,
    kill: Arc<AtomicBool>,
}

impl Iterator for CrawlIter {
    type Item = UrlEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv.next()
    }
}

impl Drop for CrawlIter {
    fn drop(&mut self) {
        self.kill.store(true, Ordering::SeqCst);
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
