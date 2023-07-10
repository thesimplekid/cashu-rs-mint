use std::str::FromStr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use jwt_compact::{
    alg::{Hs256, Hs256Key},
    prelude::*,
    AlgorithmExt,
};
use node_manager_types::TokenClaims;
use nostr::key::XOnlyPublicKey;
use serde::Serialize;
use tracing::debug;

use super::node_manager::NodeMangerState;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub status: &'static str,
    pub message: String,
}

pub async fn auth<B>(
    cookie_jar: CookieJar,
    State(data): State<Arc<NodeMangerState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    debug!("{:?}", req.headers());
    let token = cookie_jar
        .get("token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get("authorization")
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_value| {
                    if auth_value.starts_with("Bearer ") {
                        Some(auth_value[7..].to_owned())
                    } else {
                        None
                    }
                })
        });
    debug!(" token {:?}", token);
    let token = token.ok_or_else(|| {
        let json_error = ErrorResponse {
            status: "fail",
            message: "You are not logged in, please provide token".to_string(),
        };
        (StatusCode::UNAUTHORIZED, Json(json_error))
    })?;

    let token = UntrustedToken::new(&token).unwrap();

    let key = Hs256Key::new(data.settings.ln.jwt_secret.clone());
    let token: Token<TokenClaims> = Hs256.validator(&key).validate(&token).map_err(|_| {
        let json_error = ErrorResponse {
            status: "fail",
            message: "Could not verify token".to_string(),
        };
        (StatusCode::UNAUTHORIZED, Json(json_error))
    })?;

    let time_options = TimeOptions::default();

    token
        .claims()
        .validate_expiration(&time_options)
        .map_err(|_| {
            let json_error = ErrorResponse {
                status: "fail",
                message: "You are not logged in, please provide token".to_string(),
            };
            (StatusCode::UNAUTHORIZED, Json(json_error))
        })?;

    let user_pubkey = XOnlyPublicKey::from_str(&token.claims().custom.sub).map_err(|_| {
        let json_error = ErrorResponse {
            status: "fail",
            message: "Invalid token".to_string(),
        };
        (StatusCode::UNAUTHORIZED, Json(json_error))
    })?;

    let authorized_users = &data.settings.ln.authorized_users;

    if !authorized_users.contains(&user_pubkey) {
        let json_error = ErrorResponse {
            status: "fail",
            message: "The user belonging to this token no longer exists".to_string(),
        };
        return Err((StatusCode::UNAUTHORIZED, Json(json_error)));
    }

    Ok(next.run(req).await)
}
