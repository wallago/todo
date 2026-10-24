//! Helper functions.

/// Expands to `"<name> <version>"` for the crate that invokes it.
#[macro_export]
macro_rules! version {
    () => {
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_names_the_calling_crate() {
        assert!(crate::version!().starts_with("canary-core "));
    }
}
