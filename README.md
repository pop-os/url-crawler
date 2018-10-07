# url-crawler

A parallel configurable web crawler, designed to crawl a website for content.

## Example

```rust
extern crate url_crawler;
use url_crawler::*;

fn apt_filter(url: &Url) -> bool {
    let url = url.as_str();
    url.ends_with("/") || url.ends_with(".deb")
}

pub fn main() {
    let crawler = Crawler::new("http://apt.pop-os.org/".into())
        .threads(4)
        .pre_fetch(apt_filter)
        .crawl();

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
    length: 87689398,
    modified: Some(
        2018-09-25T17:54:39+00:00
    )
}
File {
    url: "http://apt.pop-os.org/proprietary/pool/bionic/main/binary-amd64/a/atom/atom_1.31.1_amd64.deb",
    length: 90108020,
    modified: Some(
        2018-10-03T22:29:15+00:00
    )
}
...
```