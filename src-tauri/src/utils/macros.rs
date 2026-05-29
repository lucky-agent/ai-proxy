// A simple error type for formatted error messages
#[warn(dead_code)]
pub(crate) struct FormattedError(pub(crate) String);

impl std::fmt::Display for FormattedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Debug for FormattedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormattedError({})", self.0)
    }
}

impl std::error::Error for FormattedError {}

// Equivalent to anyhow::bail!
#[macro_export]
macro_rules! bail {
    ($msg:literal) => {
        return Err(::core::convert::Into::into(
            ::rama::error::extra::OpaqueError::from_static_str($msg),
        ))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(::core::convert::Into::into(
            $crate::utils::macros::FormattedError(format!($fmt, $($arg)*)),
        ))
    };
}

// Equivalent to anyhow::anyhow!
#[macro_export]
macro_rules! anyhow {
    ($msg:literal) => {
        ::rama::error::extra::OpaqueError::from_static_str($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::utils::macros::FormattedError(format!($fmt, $($arg)*))
    };
}
