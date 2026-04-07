//! Network agnostict error type to map network interface errors in the mail code.

#[derive(Debug, thiserror::Error)]
/// Proton Api Error
pub enum NetworkError {
    #[error("ApiError status={status} code={code} desc={description:?}")]
    Api {
        status: u16,
        code: u32,
        description: Option<String>,
    },

    /// General purpose http error if there is no proton api error
    #[error("Http status={status}")]
    Http {
        status: u16,
        response: Option<String>,
    },

    /// The request failed due to connection and/or transmission errors
    #[error(transparent)]
    Transport(anyhow::Error),
    /// Request timed out
    #[error("Timed out")]
    TimeOut,
    /// Something else happened
    #[error(transparent)]
    Other(anyhow::Error),
}
