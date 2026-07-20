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

    #[error("{0} already contains a Memo notebook")]
    AlreadyANotebook(PathBuf),

    #[error("no task with id {0}")]
    TaskNotFound(String),

    #[error("invalid list name {0:?}")]
    InvalidListName(String),
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
