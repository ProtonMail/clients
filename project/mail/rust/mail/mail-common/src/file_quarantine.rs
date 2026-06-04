use std::path::Path;

const DEFAULT_APP_NAME: &str = "Proton Mail";

pub struct FileQuarantineXattr {
    pub app_name: String,
}

impl Default for FileQuarantineXattr {
    fn default() -> Self {
        Self {
            app_name: String::from(DEFAULT_APP_NAME),
        }
    }
}

impl FileQuarantineXattr {
    pub fn new_or_fallback(name: Option<String>) -> Self {
        Self {
            app_name: name.unwrap_or_else(|| String::from(DEFAULT_APP_NAME)),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn get_quarantine_xattr(path: &Path) -> std::io::Result<String> {
        let requested_len = rustix::fs::getxattr(path, "com.apple.quarantine", &mut [0_u8; 0])?;
        let mut data = vec![0_u8; requested_len];
        let len = rustix::fs::getxattr(path, "com.apple.quarantine", &mut data)?;
        Ok(String::from_utf8_lossy(&data[..len]).into_owned())
    }
}

pub trait FileQuarantineXattrSetter {
    fn set_quarantine_xattr(&self, path: &Path) -> std::io::Result<()>;
}

impl FileQuarantineXattrSetter for FileQuarantineXattr {
    #[cfg(target_os = "macos")]
    fn set_quarantine_xattr(&self, path: &Path) -> std::io::Result<()> {
        let app_name = &self.app_name;
        // IDEA: This doesn't update the sqlite database at ~/Library/Preferences/com.apple.LaunchServices.QuarantineEventsV2
        // To do that we must use https://developer.apple.com/documentation/foundation/url/setresourcevalues(_:)
        // But this isn't required so we do the basic quarantine for now
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let data = format!("0083;{ts:x};{app_name};");
        rustix::fs::setxattr(
            path,
            "com.apple.quarantine",
            data.as_bytes(),
            rustix::fs::XattrFlags::empty(),
        )
        .map_err(std::io::Error::from)
    }

    #[cfg(not(target_os = "macos"))]
    fn set_quarantine_xattr(&self, _path: &Path) -> std::io::Result<()> {
        // Only supported on macos
        Ok(())
    }
}
