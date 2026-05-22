use derive_more::Display;

/// Quark CLI output format flag (`-f`). Use [`Json`](LtQuarkFormat::Json) when the response type is
/// [`LtQuarkJSONRes`]; [`Text`](LtQuarkFormat::Text) is the default for line-oriented stdout.
#[derive(Debug, Display, Clone, Copy)]
pub enum LtQuarkFormat {
    #[display("text")]
    Text,
    #[display("json")]
    Json,
}
