use std::error::Error as StdError;

use axum::http::StatusCode;

pub fn format_error_chain(error: &dyn StdError) -> String {
    let mut chain = vec![error.to_string()];
    let mut source = error.source();

    while let Some(err) = source {
        chain.push(err.to_string());
        source = err.source();
    }

    chain.join(": ")
}

pub fn status_class(status: StatusCode) -> &'static str {
    if status.is_server_error() {
        "server_error"
    } else if status.is_client_error() {
        "client_error"
    } else if status.is_redirection() {
        "redirection"
    } else if status.is_success() {
        "success"
    } else {
        "informational"
    }
}
