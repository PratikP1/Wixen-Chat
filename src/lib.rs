//! Wixen Chat: a fully accessible, cross-platform chat client based on the
//! Matrix specification.

/// The application's display name and version, shown at startup and in
/// diagnostics.
#[must_use]
pub fn identity() -> String {
    format!("Wixen Chat {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_names_the_app_and_its_cargo_version() {
        assert_eq!(
            identity(),
            format!("Wixen Chat {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
