use std::io;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use std::time::SystemTimeError;

use thiserror::Error;

/// The custom error type for this application.
#[derive(Error, Debug)]
pub enum AppError {
    /// Represents an I/O error with context (the path that caused it).
    #[error("I/O error for '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// A generic I/O error without a specific path.
    #[error("I/O error: {0}")]
    GenericIo(#[from] io::Error),

    /// Error while parsing a .trashinfo file.
    #[error("Failed to parse trash info file '{path}': {reason}")]
    TrashInfoParse { path: PathBuf, reason: String },

    /// Occurs when trying to restore a file to a location that already exists.
    #[error("Destination '{path}' already exists. Cannot restore.")]
    RestoreCollision { path: PathBuf },

    /// The file to be restored does not exist in the trash `files` directory.
    #[error("Trashed item '{path}' not found. The trash directory might be in an inconsistent state.")]
    TrashedItemNotFound { path: PathBuf },

    /// No trash directories (e.g., ~/.local/share/Trash) could be found.
    #[error("No trash directories found.")]
    NoTrashDirectories,

    /// Occurs when trying to trash an item that is already in a trash directory.
    #[error("Item '{path}' is already in the trash.")]
    AlreadyInTrash { path: PathBuf },

    /// Occurs when trying to trash an item that is already in a trash directory.
    #[error("Trash '{path}' is symbolic link.")]
    SymbolicLink { path: PathBuf },

    /// Occurs when trying to move a file across different filesystems (devices).
    #[error("Cross-device move not supported for '{path}'. The destination is on a different filesystem.")]
    CrossDeviceMove { path: PathBuf },

    /// Error originating from the `mountpoints` crate.
    #[error("Failed to read mount points: {0}")]
    Mountpoints(#[from] mountpoints::Error),

    /// Error when converting system time.
    #[error("System time error: {0}")]
    SystemTime(#[from] SystemTimeError),

    /// Error when converting a byte vector to a UTF-8 string.
    #[error("UTF-8 conversion error: {0}")]
    FromUtf8(#[from] FromUtf8Error),

    /// A generic, message-based error.
    #[error("{0}")]
    Message(String),

    /// No output error
    #[error("Ignorable Error")]
    Ignorable,
}

/// Allows converting from a string slice to our custom error type.
impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Message(s.to_string())
    }
}
