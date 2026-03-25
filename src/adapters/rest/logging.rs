use axum::http::StatusCode;

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
