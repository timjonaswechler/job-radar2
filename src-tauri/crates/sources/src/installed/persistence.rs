use std::{
    io::{self, Write},
    path::Path,
};

pub(super) fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = atomic_write_file::AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.commit()
}
