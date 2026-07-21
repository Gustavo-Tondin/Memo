use std::path::PathBuf;

/// Everything that can go wrong inside the core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0} is not a Memo notebook")]
    NotANotebook(PathBuf),

    /// The folder has no `.workspace.json`. A folder only becomes interface
    /// by carrying the marker — never on its own.
    #[error("{0} is not a workspace")]
    NotAWorkspace(PathBuf),

    /// A widget's `folder` tried to escape its workspace, or is malformed.
    /// The config file is user input, same as a list name.
    #[error("invalid widget folder {0:?}")]
    InvalidWidgetFolder(String),

    #[error("{0} already contains a Memo notebook")]
    AlreadyANotebook(PathBuf),

    #[error("no task with id {0}")]
    TaskNotFound(String),

    #[error("invalid list name {0:?}")]
    InvalidListName(String),

    /// The notebook was written by a newer version of the app. Opening it
    /// read-only is safer than rewriting a file whose fields we do not know.
    #[error("notebook uses schema version {found}, this build supports {supported}")]
    ReadOnlyNotebook { found: u64, supported: u64 },

    /// `Inbox` and `Completas` are recreated on every open, so renaming or
    /// deleting them would only confuse the user.
    #[error("{0} is a default list and cannot be renamed or deleted")]
    ProtectedList(String),

    /// The file watcher could not be started or kept running.
    #[error("could not watch the notebook: {0}")]
    Watch(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Attaches the offending path to an io error, so failures say *which* file
/// broke instead of just "No such file or directory".
pub(crate) trait IoContext<T> {
    fn ctx(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::result::Result<T, std::io::Error> {
    fn ctx(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
    }
}
