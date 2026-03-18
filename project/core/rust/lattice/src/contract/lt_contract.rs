use std::{borrow::Cow, collections::HashMap};

#[cfg(feature = "serde")]
use crate::LtRequestBody;
use crate::{LatticeError, LtResponseBody, Method};

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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<GetRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<()>;
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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<GetRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<()>;
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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<PutRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<()>;
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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<PostRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<&'a Self>;
///
///     fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
///         Ok(Method::Post(LtSlimAPIJSON(self)))
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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<PostRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<&'a PostRequestBody>;
///
///     fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
///         Ok(Method::Post(LtSlimAPIJSON(&self.body))) // Or Method::Put(LtSlimAPIJSON(&self.body))
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
/// use lattice::{LtContract, LatticeError, Method, LtSlimAPIJSON};
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
///     type Response = LtSlimAPIJSON<GetRequestRes>;
///     type Body<'a> = LtSlimAPIJSON<()>;
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
    type Response: LtResponseBody;

    /// The body type for the contract.
    ///
    /// This type needs to implement `Serialize` from `serde` if the method is `POST` or `PUT`.
    /// This can be `LtRawBody` if the method is `GET` or `DELETE`.
    #[cfg(feature = "serde")]
    type Body<'b>: LtRequestBody
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
