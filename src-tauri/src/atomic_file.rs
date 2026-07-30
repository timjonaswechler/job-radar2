use std::io::{self, Write};
use std::path::Path;

pub(crate) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = atomic_write_file::AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_file_without_exposing_partial_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        std::fs::write(&path, b"old").unwrap();

        replace(&path, b"new").unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"new");
    }
}
