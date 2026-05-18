use crate::errors::UserSessionError;
use crate::mail::MailUserSession;
use crate::uniffi_async;
use mail_common::{ProtonMailError as RealProtonMailError, Unexpected};
use mail_core_common::datatypes::{LightOrDarkMode, SenderImageSize as CoreSenderImageSize};

/// Image size for sender images.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum SenderImageSize {
    S16,
    S32,
    S64,
    S128,
}

impl From<SenderImageSize> for CoreSenderImageSize {
    fn from(value: SenderImageSize) -> Self {
        match value {
            SenderImageSize::S16 => Self::S16,
            SenderImageSize::S32 => Self::S32,
            SenderImageSize::S64 => Self::S64,
            SenderImageSize::S128 => Self::S128,
        }
    }
}

#[uniffi_export]
impl MailUserSession {
    /// Get a path to the sender image.
    ///
    /// # Parameters
    /// * `address`: Email address of the sender.
    /// * `bimi_selector`: BIMI protocol selector.
    /// * `display_sender_image`: Whether this sender would has sender image enabled.
    /// * `size`: Image size — also determines the maximum scale-up factor sent to the API.
    /// * `mode`: Can be used to select if the "light" or "dark" mode of the image is desired (default is light).
    /// * `format`: Desired image format, if none is specified the default format of the image will be used.
    ///
    /// Returns a path toward the image file or `None` if no image needs to be displayed.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(skip_all)]
    pub async fn image_for_sender(
        &self,
        address: String,
        bimi_selector: Option<String>,
        display_sender_image: bool,
        size: Option<SenderImageSize>,
        mode: Option<String>,
        format: Option<String>,
    ) -> Result<Option<String>, UserSessionError> {
        let ctx = self.ctx()?;
        uniffi_async(async move {
            let mode = light_or_dark_mode_from_string(mode)?;
            let size = size.map(CoreSenderImageSize::from);
            Ok::<_, RealProtonMailError>(
                ctx.image_for_sender(
                    address.into(),
                    bimi_selector,
                    display_sender_image,
                    size,
                    mode,
                    format,
                )
                .await?,
            )
        })
        .await
        .map_err(UserSessionError::from)
    }
}

fn light_or_dark_mode_from_string(
    mode: Option<String>,
) -> Result<Option<LightOrDarkMode>, Unexpected> {
    let mode = match mode {
        Some(m) => match m.as_str() {
            "light" | "Light" => Some(LightOrDarkMode::Light),
            "dark" | "Dark" => Some(LightOrDarkMode::Dark),
            _ => return Err(Unexpected::InvalidArgument),
        },
        None => None,
    };
    Ok(mode)
}
