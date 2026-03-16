pub(crate) mod helpers;

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(all(feature = "muon", feature = "serde"))]
pub mod muon;

#[cfg(feature = "core")]
pub mod core;

#[cfg(feature = "quark")]
pub mod quark;

#[cfg(feature = "observability")]
pub mod observability;

mod api_definitions;
pub use api_definitions::*;

mod sensitive;

mod errors;
pub use errors::*;

use derive_more::Display;
pub use sensitive::*;

use std::{borrow::Cow, collections::HashMap};

/// An error type for Lattice operations.
///
/// This error type is used to wrap errors from the `serde_json` crate.
#[derive(derive_more::Debug, Display)]
pub enum LatticeError {
    #[cfg(feature = "serde")]
    #[display("SerdeJSON: {_0} {_1:?}")]
    SerdeJSON(serde_json::Error, Option<String>),

    #[display("UnexpectedResponse: {_0}")]
    UnexpectedResponse(String),

    #[cfg(feature = "muon")]
    Muon(::muon::Error),

    #[display("UnexpectedStatusCode({_0}: \"{}\")", String::from_utf8(_1.to_vec()).unwrap_or_else(|_| format!("Invalid UTF-8: {:?}", _1)))]
    #[debug("UnexpectedStatusCode({_0}: \"{}\")", String::from_utf8(_1.to_vec()).unwrap_or_else(|_| format!("Invalid UTF-8: {:?}", _1)))]
    UnexpectedStatusCode(u16, Vec<u8>),

    #[cfg(feature = "serde")]
    #[display("ApiError Status({_0}), {_1:?}")]
    ApiError(u16, Box<LtApiResponseError>),
}

impl LatticeError {
    pub fn as_api_error(&self) -> Option<&LtApiResponseError> {
        if let Self::ApiError(_, error) = self {
            Some(error)
        } else {
            None
        }
    }
}

/// A trait for all Lattice contracts.
///
/// This trait is used to define the path and method for a given request.
/// It also defines the response type and the body type for the contract.
///
/// The path is the HTTP url path for the request.
/// The body is linked to the method of the request.
///
/// Here are the reference implementation of the trait:
/// ## GET
/// ### Without path parameters
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::borrow::Cow;
///
/// struct GetRequest;
///
/// #[derive(serde::Deserialize)]
/// struct GetRequestRes {
///     some_json_field: String,
/// }
///
/// impl LtContract for GetRequest {
///     type Response = GetRequestRes;
///     type Body<'a> = ();
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Borrowed("/auth/v4/modulus"))
///     }
/// }
/// ```
/// Where `GetRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// ### With path parameters
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::borrow::Cow;
///
/// struct GetRequest {
///     some_path_param: String,
/// }
///
/// #[derive(serde::Deserialize)]
/// struct GetRequestRes {
///     some_json_field: String,
/// }
///
/// impl LtContract for GetRequest {
///     type Response = GetRequestRes;
///     type Body<'a> = ();
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Owned(format!("/some/path/{}", self.some_path_param)))
///     }
/// }
/// ```
/// Where `GetRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// ## DELETE
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::borrow::Cow;
///
/// #[derive(serde::Deserialize)]
/// struct PutRequestRes {
///     some_json_field: String,
/// }
///
/// struct PutRequest {
///     some_path_param: String,
/// }
///
/// impl LtContract for PutRequest {
///     type Response = PutRequestRes;
///     type Body<'a> = ();
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Owned(format!("/some/path/{}", self.some_path_param)))
///     }
///
///     fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
///         Ok(Method::Delete)
///     }
/// }
/// ```
/// Where `PutRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// ## POST / PUT
/// ### Without url parameters
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::borrow::Cow;
///
/// #[derive(serde::Deserialize)]
/// struct PostRequestRes {
///     some_json_field: String,
/// }
///
/// #[derive(serde::Serialize)]
/// struct PostRequest {
///     some_json_field: String,
/// }
///
/// impl LtContract for PostRequest {
///     type Response = PostRequestRes;
///     type Body<'a> = &'a Self;
///
///     fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
///         Ok(Method::Post(self)) // Or Method::Put(self)
///     }
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Borrowed("/some/path"))
///     }
/// }
/// ```
/// Where `PostRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// ### With url parameters
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::borrow::Cow;
///
/// #[derive(serde::Deserialize)]
/// struct PostRequestRes {
///     some_json_field: String,
/// }
///
/// struct PostRequest {
///     some_url_param: String,
///     body: PostRequestBody,
/// }
///
/// #[derive(serde::Serialize)]
/// struct PostRequestBody {
///     some_json_field: String,
/// }
///
/// impl LtContract for PostRequest {
///     type Response = PostRequestRes;
///     type Body<'a> = &'a PostRequestBody;
///
///     fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
///         Ok(Method::Post(&self.body)) // Or Method::Put(&self.body)
///     }
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Owned(format!("/some/path/{}", self.some_url_param)))
///     }
/// }
/// ```
/// Where `PostRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// ## Query parameters
/// ```rust
/// use lattice::{LtContract, LatticeError, Method};
/// use std::collections::HashMap;
/// use std::borrow::Cow;
///
/// struct GetRequest {
///     some_query_param: String,
/// }
///
/// #[derive(serde::Deserialize)]
/// struct GetRequestRes {
///     some_json_field: String,
/// }
///
/// impl LtContract for GetRequest {
///     type Response = GetRequestRes;
///     type Body<'a> = ();
///
///     fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
///         Ok(Some(HashMap::from([("some_query_param".to_string(), self.some_query_param.to_string())])))
///     }
///
///     fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
///         Ok(Cow::Borrowed("/some/path"))
///     }
/// }
/// ```
/// Where `GetRequestRes` is the response type for the request.
/// This type needs to implement `Deserialize` from `serde`.
///
/// The query parameters can be `None` if there are no query parameters.
///
/// The query parameters are a `HashMap<String, String>`.
/// The key is the query parameter name and the value is the query parameter value.
pub trait LtContract {
    /// The response type for the contract.
    ///
    /// This type needs to implement `Deserialize` from `serde`.
    #[cfg(feature = "serde")]
    type Response: serde::de::DeserializeOwned;
    #[cfg(not(feature = "serde"))]
    type Response;

    /// The body type for the contract.
    ///
    /// This type needs to implement `Serialize` from `serde` if the method is `POST` or `PUT`.
    /// This can be `()` if the method is `GET` or `DELETE`.
    #[cfg(feature = "serde")]
    type Body<'b>: serde::Serialize + Sized
    where
        Self: 'b;
    #[cfg(not(feature = "serde"))]
    type Body<'b>: Sized
    where
        Self: 'b;

    /// The path for the contract.
    ///
    /// This method returns the path for the contract.
    /// The path is the HTTP url path for the request.
    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError>;

    /// The method for the contract.
    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Get)
    }

    /// The query parameters for the contract.
    ///
    /// This method returns the query parameters for the contract.
    /// The query parameters are the HTTP url query parameters for the request.
    ///
    /// The query parameters are a `HashMap<String, String>`.
    /// The key is the query parameter name and the value is the query parameter value.
    ///
    /// The query parameters can be `None` if there are no query parameters.
    fn query(&self) -> Result<Option<HashMap<String, String>>, LatticeError> {
        Ok(None)
    }

    /// The headers for the contract.
    ///
    /// This method returns the headers for the contract.
    /// The headers are the HTTP headers for the request.
    /// The headers are a `HashMap<String, String>`.
    /// The key is the header name and the value is the header value.
    fn headers(&self) -> Result<HashMap<String, String>, LatticeError> {
        Ok(HashMap::new())
    }
}

/// A trait for Lattice contracts that are not authenticated.
///
/// This trait is implemented by all Lattice contracts that don't require authentication.
pub trait UnauthReq {}

/// A trait for Lattice contracts that are authenticated.
///
/// This trait is implemented by all Lattice contracts that require authentication.
pub trait AuthReq {}

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
/// This type needs to implement `Serialize` from `serde` if the method is `POST` or `PUT`.
/// This can be `()` if the method is `GET` or `DELETE`.
#[derive(Clone)]
#[cfg(feature = "serde")]
pub enum Method<T: serde::Serialize> {
    Get,
    Delete,
    Post(T),
    Put(T),
}
#[cfg(feature = "serde")]
impl<T: serde::Serialize> Method<T> {
    pub fn into_body(self) -> Option<T> {
        match self {
            Self::Delete | Self::Get => None,
            Self::Post(body) | Self::Put(body) => Some(body),
        }
    }
}

#[cfg(not(feature = "serde"))]
pub enum Method<T> {
    Get,
    Delete,
    Post(T),
    Put(T),
}

#[cfg(not(feature = "serde"))]
impl<T> Method<T> {
    pub fn into_body(self) -> Option<T> {
        match self {
            Self::Delete | Self::Get => None,
            Self::Post(body) | Self::Put(body) => Some(body),
        }
    }
}
