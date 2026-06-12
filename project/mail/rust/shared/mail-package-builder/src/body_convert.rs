use crate::error::PackageError;
use mail_html_transformer::sanitizer::StripStyleSheets;
use mail_html_transformer::{Html2TextOptions, Transformer};

/// Converts HTML to plain text via the sanitizing transformer pipeline.
pub fn html_to_text(input: &str) -> Result<String, PackageError> {
    let mut transformer = Transformer::new(input);

    transformer.transform_from_proton_schemes();
    transformer.add_noreferrer();
    transformer.strip_utm();
    transformer.strip_whitelist(StripStyleSheets::No);

    transformer
        .to_plain_text(Html2TextOptions {
            decorate_links: false,
            decorate_images: false,
        })
        .map_err(|e| PackageError::HtmlToTextConversion(e.to_string()))
}
