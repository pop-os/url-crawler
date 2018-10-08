# 0.2.1

- The `PreFetchCallback` was not being called before fetching HEAD requests.

# 0.2.0

- Switch to using `Arc<Fn()>` callbacks instead of function pointers.

# 0.1.1

- Remove dependency on the failure crates
- Add the `content_type` field to `UrlEntry::File`

# 0.1.0

- Initial release