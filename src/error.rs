use std::fmt;

/// Error type returned from constructing a shell
///
/// The `ShellInitError` enum represents the various errors that may occur when
/// attempting to initialize a shell. This includes errors related to directory
/// access permissions and existence.
#[derive(Debug)]
pub enum ShellInitError {
    /// This variant indicates that an error occurred related to a directory.
    /// It can occur when trying to construct an `IShell` inside a directory that does not exist.
    ///
    /// The associated `String` contains a message that provides more details about the error,
    /// such as the directory (or variations of the directory) that could not be found.
    ///
    /// Display trait included.
    DirectoryError(String),
}

impl fmt::Display for ShellInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellInitError::DirectoryError(msg) => write!(f, "IShell directory error: {}", msg),
        }
    }
}