use std::fmt::Display;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use tower_cookies::Cookies;

pub const THEME_COOKIE_NAME: &str = "theme";

// --- Theme Enum for Type Safety ---
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

impl Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Light => write!(f, "light"),
            Theme::Dark => write!(f, "dark"),
        }
    }
}

/// Returns Some(Theme) with Theme representing the value of the cookie with `THEME_COOKIE_NAME`.
/// Returns None if cookie was not set.
pub struct ExplicitTheme(pub Theme);

impl<S> FromRequestParts<S> for ExplicitTheme
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let theme = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get cookies"))?
            .get(THEME_COOKIE_NAME)
            .map(|cookie| match cookie.value() {
                "light" => Theme::Light,
                _ => Theme::Dark,
            })
            .unwrap_or(Theme::Dark);

        Ok(ExplicitTheme(theme))
    }
}
