use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cashu_sdk::cashu::error::ErrorResponse;
use cashu_sdk::lightning_invoice::ParseOrSemanticError;

#[derive(Debug)]
pub enum Error {
    DecodeInvoice,
    StatusCode(StatusCode),
    _Ln(ln_rs::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecodeInvoice => write!(f, "Failed to decode LN Invoice"),
            Self::StatusCode(code) => write!(f, "{}", code),
            Self::_Ln(code) => write!(f, "{}", code),
        }
    }
}

impl From<StatusCode> for Error {
    fn from(code: StatusCode) -> Self {
        Self::StatusCode(code)
    }
}

impl From<ParseOrSemanticError> for Error {
    fn from(_err: ParseOrSemanticError) -> Self {
        Self::DecodeInvoice
    }
}

impl From<url::ParseError> for Error {
    fn from(_err: url::ParseError) -> Self {
        Self::DecodeInvoice
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::DecodeInvoice => (StatusCode::BAD_REQUEST, self.to_string()).into_response(),
            Error::StatusCode(code) => (code, "").into_response(),
            Error::_Ln(code) => {
                (StatusCode::INTERNAL_SERVER_ERROR, code.to_string()).into_response()
            }
        }
    }
}

pub fn into_response(error: cashu_sdk::mint::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json::<ErrorResponse>(error.into()),
    )
        .into_response()
}
