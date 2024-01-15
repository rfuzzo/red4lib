/////////////////////////////////////////////////////////////////////////////////////////
// IN-MEMORY ARCHIVE
//
//https://learn.microsoft.com/en-us/dotnet/api/system.io.compression.ziparchivemode?view=net-8.0
//
// When you set the mode to Update, the underlying file or stream must support reading, writing, and seeking.
// The content of the entire archive is held in memory,
// and no data is written to the underlying file or stream until the archive is disposed.
//
// We don't implement a wrapped stream here. Archive needs to be written manually for now.
//
/////////////////////////////////////////////////////////////////////////////////////////

use std::{collections::HashMap, io::Result, path::Path};

use super::*;

#[derive(Debug)]
pub struct ZipArchiveMemory {
    /// The files inside an archive
    pub entries: HashMap<u64, ZipEntry>,
    pub dependencies: Vec<Dependency>,
}

/////////////////////////////////////////////////////////////////////////////////////////
// IMPL

impl ZipArchiveMemory {
    fn write(&mut self) {
        todo!()
    }

    /// Compresses and adds a file to the archive.
    ///
    /// # Errors
    ///
    /// This function will return an error if compression or io fails, or if the mode is Read.
    pub fn create_entry<P: AsRef<Path>>(
        &mut self,
        _file_path: P,
        _compression_level: CompressionLevel,
    ) -> Result<ZipEntry> {
        // can only add entries in update mode

        // write?

        todo!()
    }

    /// Deletes an entry from the archive
    pub fn delete_entry(&mut self, hash: &u64) -> Option<ZipEntry> {
        // can only delete entries in update mode

        // Set dirty

        self.entries.remove(hash)
    }
}
