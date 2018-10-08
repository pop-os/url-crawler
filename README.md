# url-crawler

A configurable parallel web crawler, designed to crawl a website for content.

- [Changelog](./CHANGELOG.md)
- [Docs.rs](https://docs.rs/url-crawler)

## Example

```rust
extern crate url_crawler;
use std::sync::Arc;
use url_crawler::*;

/// Function for filtering content in the crawler before a HEAD request.
///
/// Only allow directory entries, and files that have the `deb` extension.
fn apt_filter(url: &Url) -> bool {
    let url = url.as_str();
    url.ends_with("/") || url.ends_with(".deb")
}

pub fn main() {
    // Create a crawler designed to crawl the given website.
    let crawler = Crawler::new("http://apt.pop-os.org/".to_owned())
        // Use four threads for fetching
        .threads(4)
        // Check if a URL matches this filter before performing a HEAD request on it.
        .pre_fetch(Arc::new(apt_filter))
        // Initialize the crawler and begin crawling. This returns immediately.
        .crawl();

    // Process url entries as they become available
    for file in crawler {
        println!("{:#?}", file);
    }
}
```

### Output

The folowing includes two snippets from the combined output.

```
...
Html {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/source/s/system76-cudnn-9.2/"
}
Html {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/source/t/tensorflow-1.9-cuda-9.2/"
}
Html {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/source/t/tensorflow-1.9-cpu/"
}
...
File {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/binary-amd64/a/atom/atom_1.30.0_amd64.deb",
    content_type: "application/octet-stream",
    length: 87689398,
    modified: Some(
        2018-09-25T17:54:39+00:00
    )
}
File {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/binary-amd64/a/atom/atom_1.31.1_amd64.deb",
    content_type: "application/octet-stream",
    length: 90108020,
    modified: Some(
        2018-10-03T22:29:15+00:00
    )
}
...
```