use derive_more::Display;

/// Quark CLI output format flag (`-f`). Use [`Json`](Self::Json) when the response type is
/// [`super::LtQuarkJSONRes`]; [`Text`](Self::Text) is the default for line-oriented stdout.
#[derive(Debug, Display, Clone, Copy)]
pub enum LtQuarkFormat {
    #[display("text")]
    Text,
    #[display("json")]
    Json,
}
