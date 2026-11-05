//! Pure convert-idempotency rules (native-tested).

/// What to do when the lead already has `converted_deal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertMode {
    /// Return the existing deal (default).
    ReturnExisting,
    /// Create another deal anyway.
    Create,
    /// Fail unless `create_another` is set.
    Fail,
}

pub fn parse_convert_mode(if_converted: Option<&str>, create_another: bool) -> ConvertMode {
    if create_another {
        return ConvertMode::Create;
    }
    match if_converted.unwrap_or("return_existing") {
        "create" => ConvertMode::Create,
        "fail" => ConvertMode::Fail,
        _ => ConvertMode::ReturnExisting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_returns_existing() {
        assert_eq!(parse_convert_mode(None, false), ConvertMode::ReturnExisting);
        assert_eq!(parse_convert_mode(Some("return_existing"), false), ConvertMode::ReturnExisting);
    }

    #[test]
    fn create_and_fail_modes() {
        assert_eq!(parse_convert_mode(Some("create"), false), ConvertMode::Create);
        assert_eq!(parse_convert_mode(Some("fail"), false), ConvertMode::Fail);
    }

    #[test]
    fn create_another_overrides_fail() {
        assert_eq!(parse_convert_mode(Some("fail"), true), ConvertMode::Create);
    }
}
