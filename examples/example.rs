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
