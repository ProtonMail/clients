//! Shared [`Sensitive`] wrapper: zeroize-on-drop, redacted [`Debug`], optional [`serde`] and [`facet::Facet`].
//!
//! Use this crate from **lattice**, **account-crux**, and anywhere else that needs the same
//! [`Sensitive`] type with a consistent optional Facet surface. Enable the **`facet`** feature when
//! the dependency graph includes Facet-derived DTOs that store secrets in [`Sensitive`].
//!
//! For narrow FFI aliases, the `core-common-types` crate may still expose `SensitiveString` /
//! `SensitiveBytes`.

pub use zeroize::Zeroize;

/// A wrapper for sensitive data: redacted [`Debug`], zeroization on drop, optional serde and Facet.
///
/// Enable **`serde`** for `Serialize`/`Deserialize` (transparent). Enable **`facet`** so this type
/// implements [`facet::Facet`] for Crux/typegen (inner shape follows `T`'s Facet impl when present).
///
/// Note: For client typegen / FFI, concrete aliases in `core-common-types` (`SensitiveString`, etc.)
/// may still be preferable where you want a non-generic name in bindings.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
pub struct Sensitive<T: Zeroize>(T);

impl<T: Zeroize> Sensitive<T> {
    /// Wraps `value` in a [`Sensitive`] container.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Consumes the wrapper and returns the inner value.
    ///
    /// **Warning:** the returned value is not zeroized by this type on drop; use with care.
    pub fn into_inner(self) -> T {
        let this = std::mem::ManuallyDrop::new(self);
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

impl<T: Zeroize> AsRef<[T]> for Sensitive<Vec<T>> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}
