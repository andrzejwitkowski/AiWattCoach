use axum::http::{header, HeaderMap};

pub fn read_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers.get_all(header::COOKIE).iter().find_map(|raw| {
        let cookie_header = raw.to_str().ok()?;

        cookie_header.split(';').find_map(|entry| {
            let trimmed = entry.trim();
            let (name, value) = trimmed.split_once('=')?;

            if name == cookie_name {
                Some(value.to_string())
            } else {
                None
            }
        })
    })
}
