//! The Proton API service.
//!
//! This module provides a service that can be used to make requests to the
//! Proton API. Each method provided should match 1:1 with an API endpoint, and
//! follow the naming convention of the endpoint. For example, the endpoint
//! `GET /contacts` should have a method provided called `get_contacts()`.
//!
//! The purpose of the API service is to provide not only the means to make
//! requests, but also a formalisation of the data that is sent and received. To
//! this end, the data structures provided by this service should mirror the API
//! endpoint definitions, and NOT have any business logic or other
//! functionality.
//!
//! To be clear, they should only contain data, and not methods; should not be
//! saved in the database; and should not be used for anything except providing
//! an interface for data exchange.
//!
//! The goal is not to provide a semantic representation of actions, but a
//! strict and closely-coupled interface to the API.
//!
//! Everything in this service should be self-contained as much as possible, and
//! should be considered encapsulated and separate from the main application,
//! including the application's data. Types should be converted back and forth
//! as necessary, but generally not used in both places.
//!
//! # Example illustration
//!
//! Let's consider the case of a user. The application may have a `User` struct
//! that is used to represent a user in the application. From time to time it
//! will be necessary to interact with the API to sync data relevant to that
//! `User`. To do this, the necessary information should be used to prepare data
//! to send to the API, such as a `PostUserRequest` struct containing a child
//! data type of `User`. This latter `User` is not the same struct as that used
//! in the application, but rather, a data-only mirroring of the data the API
//! needs to receive.
//!
//! Now let's consider retrieving a user record. The API response might define
//! a `User` structure that is the same as the one accepted via `POST` — if so,
//! this could go into [`common`]. Otherwise, we will need two `User` structs,
//! one in [`request_data`] and one in [`response_data`]. Neither one of these
//! is the same as the one used inside the main application.
//!
//! Once the data is retrieved, the data required by the application can be
//! extracted from the response and converted into the application's `User`
//! struct. This struct would then be the one containing various methods and
//! other functionality, and would get saved to the database.
//!

mod auth;
mod core;
mod data;
pub mod growth;
mod payments;

pub mod common;
pub mod prelude;
pub mod traits;

pub use self::auth::*;
pub use self::core::*;
pub use self::data::*;
pub use self::growth::*;
pub use self::payments::*;
pub use mail_api_session::build::{BuildError, build};
pub use mail_muon;
