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