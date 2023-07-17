use std::{fmt, string::FromUtf8Error};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use cashu_crab::lightning_invoice::ParseOrSemanticError;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Error {
    StatusCode(StatusCode),
    Serde(serde_json::Error),
    WrongClnResponse,
    Custom(String),
    TonicError(String),
    InvalidLnUrl,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatusCode(code) => write!(f, "{}", code.as_str()),
            Self::Serde(code) => write!(f, "{}", code),
            Self::WrongClnResponse => write!(f, "Cln returned the wrong response"),
            Self::Custom(code) => write!(f, "{}", code),
            Self::TonicError(code) => write!(f, "{}", code),
            Self::InvalidLnUrl => write!(f, "LnUrl is invalid"),
        }
    }
}

impl From<StatusCode> for Error {
    fn from(code: StatusCode) -> Self {
        Self::StatusCode(code)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

impl From<cln_rpc::RpcError> for Error {
    fn from(err: cln_rpc::RpcError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<bitcoin_hashes::hex::Error> for Error {
    fn from(err: bitcoin_hashes::hex::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<ParseOrSemanticError> for Error {
    fn from(err: ParseOrSemanticError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<ldk_node::NodeError> for Error {
    fn from(err: ldk_node::NodeError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(err: std::net::AddrParseError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<bitcoin::address::Error> for Error {
    fn from(err: bitcoin::address::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<gl_client::bitcoin::secp256k1::Error> for Error {
    fn from(err: gl_client::bitcoin::secp256k1::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<bitcoin::secp256k1::Error> for Error {
    fn from(err: bitcoin::secp256k1::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<bip39::Error> for Error {
    fn from(err: bip39::Error) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<url::ParseError> for Error {
    fn from(_err: url::ParseError) -> Self {
        Self::Custom("Could not parse url".to_string())
    }
}

impl From<hex::FromHexError> for Error {
    fn from(err: hex::FromHexError) -> Self {
        Self::Custom(err.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::Custom(err.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    code: u16,
    error: String,
}

impl ErrorResponse {
    pub fn _new(code: u16, error: &str) -> Self {
        Self {
            code,
            error: error.to_string(),
        }
    }

    pub fn _as_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::StatusCode(code) => (code, "").into_response(),
            Self::Serde(code) => {
                (StatusCode::INTERNAL_SERVER_ERROR, code.to_string()).into_response()
            }
            Self::WrongClnResponse => (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
            // REVIEW: Most of these error codes shouldnt be returned on response
            // Keeping it for now to make debugging easier
            Self::Custom(code) => (StatusCode::INTERNAL_SERVER_ERROR, code).into_response(),
            Self::TonicError(_code) => (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
            Self::InvalidLnUrl => (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
        }
    }
}
