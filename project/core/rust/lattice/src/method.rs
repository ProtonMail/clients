#[cfg(feature = "serde")]
use crate::LtRequestBody;

/// A enum for the HTTP methods.
///
/// This enum is used to define the HTTP method for a given request.
///
/// The `Get` variant is used for GET requests.
/// The `Delete` variant is used for DELETE requests.
/// The `Post` variant is used for POST requests.
/// The `Put` variant is used for PUT requests.
///
/// The `T` type is the body type for the request.
/// This type needs to implement `LtRequestBody` if the method is `POST` or `PUT`.
/// This can be `()` if the method is `GET` or `DELETE`.
#[derive(Clone)]
pub enum Method<T: LtRequestBody> {
    Get,
    Delete,
    Post(T),
    Put(T),
}

impl<T: LtRequestBody> Method<T> {
    pub fn into_body(self) -> Option<T> {
        match self {
            Self::Delete | Self::Get => None,
            Self::Post(body) | Self::Put(body) => Some(body),
        }
    }
}
