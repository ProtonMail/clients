//! Quark response body parsing.
//!
//! On success, Quark returns command stdout as the HTTP body (UTF-8). Pick a [`LtQuarkRes`]
//! adapter that matches what the command prints:
//!
//! | Adapter | Body shape | Typical request |
//! |---------|------------|-----------------|
//! | [`LtQuarkJSONRes`] | Single JSON object | `-f json` ([`LtQuarkFormat::Json`]) |
//! | [`LtQuarkResTryFrom`] | One line of plain text | default text output |
//! | [`LtQuarkResString`] | Arbitrary UTF-8 text | any |
//! | Custom [`LtQuarkRes`] | Multi-line prose (regex) | text (see `domain_create`, `subscribed_user_seed`) |

mod lt_quark_contract;
mod lt_quark_format;
mod lt_quark_json_res;
mod lt_quark_res;
mod lt_quark_res_string;
mod lt_quark_res_try_from;
mod quark_command;

pub use lt_quark_contract::LtQuarkContract;
pub use lt_quark_format::LtQuarkFormat;
pub use lt_quark_json_res::LtQuarkJSONRes;
pub use lt_quark_res::LtQuarkRes;
pub use lt_quark_res_string::LtQuarkResString;
pub use lt_quark_res_try_from::LtQuarkResTryFrom;
pub use quark_command::QuarkCommand;
