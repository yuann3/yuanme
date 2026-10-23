use std::path::PathBuf;

#[cfg(any(feature = "compare", feature = "foreign_document_test"))]
pub trait FileAccessInterface: Sync {
    fn read_file(&self, path: &str) -> Result<String, std::io::Error>;
}

#[cfg(not(any(feature = "compare", feature = "foreign_document_test")))]
pub trait FileAccessInterface {
    fn read_file(&self, path: &str) -> Result<String, std::io::Error>;
}

#[derive(Clone)]
pub struct LocalFileAccessInterface {
    pub working_directory: Option<PathBuf>,
}

impl FileAccessInterface for LocalFileAccessInterface {
    fn read_file(&self, path: &str) -> Result<String, std::io::Error> {
        let final_path = self
            .working_directory
            .as_deref()
            .map(|pb| pb.join(path))
            .unwrap_or_else(|| PathBuf::from(path));
        std::fs::read_to_string(final_path)
    }
}
