pub use zeroize::Zeroize;

/// A wrapper type for sensitive data that provides both debug redaction
/// and automatic zeroization on drop.
///
/// `Sensitive<T>` behaves like `T` in all ways except:
/// - `Debug` output will print `"redacted"` instead of the actual value
/// - The inner value is automatically zeroized when dropped
///
/// This type combines the functionality of redacting sensitive data from
/// debug output while ensuring the data is securely erased from memory.
///
/// Note: For FFI/facet compatibility, use the concrete type aliases like
/// `SensitiveString` or `SensitiveBytes` instead of `Sensitive<T>` directly
/// when the type needs to be exposed through facet.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Sensitive<T: Zeroize>(T);

impl<T: Zeroize> Sensitive<T> {
    // Creates a new `Sensitive` wrapper around the given value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Consumes the wrapper and returns the inner value.
    ///
    /// **Warning**: The returned value will NOT be automatically zeroized.
    /// Use with caution.
    pub fn into_inner(self) -> T {
        // Use ManuallyDrop to prevent the Drop impl from running
        let this = std::mem::ManuallyDrop::new(self);
        // Safety: we're taking ownership and won't use `this` again
        unsafe { std::ptr::read(&this.0) } // nosemgrep
    }
}

impl<T: Zeroize> Drop for Sensitive<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> Zeroize for Sensitive<T> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> std::fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "redacted")
    }
}

impl<T: Zeroize> std::ops::Deref for Sensitive<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Zeroize> std::ops::DerefMut for Sensitive<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Zeroize> From<T> for Sensitive<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Zeroize> AsRef<T> for Sensitive<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> AsMut<T> for Sensitive<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Zeroize> AsRef<Sensitive<T>> for Sensitive<T> {
    fn as_ref(&self) -> &Sensitive<T> {
        self
    }
}

// String-specific implementations
impl AsRef<str> for Sensitive<String> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<[u8]> for Sensitive<String> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<&str> for Sensitive<String> {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// Vec-specific implementations
impl<T: Zeroize> AsRef<[T]> for Sensitive<Vec<T>> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}
