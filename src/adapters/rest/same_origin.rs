use axum::http::{header, HeaderMap};

pub(super) fn request_has_same_origin(headers: &HeaderMap, trust_proxy_headers: bool) -> bool {
    let Some(host) = effective_host(headers, trust_proxy_headers) else {
        return false;
    };
    let Some(origin) = header_value(headers, header::ORIGIN) else {
        return false;
    };

    let Ok(origin_uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };

    if let Some(fetch_site) =
        header_value(headers, header::HeaderName::from_static("sec-fetch-site"))
    {
        if fetch_site != "same-origin" && fetch_site != "same-site" {
            return false;
        }
    }

    matches!(origin_uri.scheme_str(), Some("http" | "https"))
        && origin_uri.authority().map(|authority| authority.as_str()) == Some(host.as_str())
}

fn effective_host(headers: &HeaderMap, trust_proxy_headers: bool) -> Option<String> {
    if trust_proxy_headers {
        if let Some(host) =
            header_value(headers, header::HeaderName::from_static("x-forwarded-host"))
        {
            if let Some(first) = host.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        if let Some(forwarded) = header_value(headers, header::HeaderName::from_static("forwarded"))
        {
            if let Some(host) = forwarded_host(forwarded) {
                return Some(host);
            }
        }
    }

    header_value(headers, header::HOST).map(ToString::to_string)
}

fn forwarded_host(value: &str) -> Option<String> {
    value.split(',').find_map(|entry| {
        entry.split(';').find_map(|part| {
            let mut pieces = part.trim().splitn(2, '=');
            let key = pieces.next()?.trim();
            let raw = pieces.next()?.trim().trim_matches('"');
            key.eq_ignore_ascii_case("host")
                .then(|| (!raw.is_empty()).then(|| raw.to_string()))
                .flatten()
        })
    })
}

fn header_value(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}
