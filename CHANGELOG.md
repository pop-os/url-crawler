# 0.3.0

- Allow scraping from multiple URLs in the same crawler.

```rust
// This
fn new(url: String) -> Self {}

// Now becomes
fn new(urls: impl Into<CrawlerSource>) -> Self {}

// Which enables
Crawler::new(vec!["url1".into(), "url2".into()]);
Crawler::new("url".to_owned());
```

# 0.2.1

- The `PreFetchCallback` was not being called before fetching HEAD requests.

# 0.2.0

- Switch to using `Arc<Fn()>` callbacks instead of function pointers.

# 0.1.1

- Remove dependency on the failure crates
- Add the `content_type` field to `UrlEntry::File`

# 0.1.0

- Initial release