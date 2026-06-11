use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum GraphWriterError {
    ShaclValidationError(String),
    // Other error variants can be added here
}

impl fmt::Display for GraphWriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphWriterError::ShaclValidationError(msg) => write!(f, "SHACL Validation Error: {}", msg),
        }
    }
}

impl std::error::Error for GraphWriterError {}