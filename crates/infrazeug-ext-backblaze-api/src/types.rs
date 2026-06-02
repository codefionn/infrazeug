//! Shared JSON types for the B2 Native API.

use serde::Deserialize;

/// Standard B2 error response body.
#[derive(Debug, Clone, Deserialize)]
pub struct B2ErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
}
