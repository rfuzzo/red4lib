/////////////////////////////////////////////////////////////////////////////////////////
// READ ONLY ARCHIVE
//
//https://learn.microsoft.com/en-us/dotnet/api/system.io.compression.ziparchivemode?view=net-8.0
//
// When you set the mode to Read, the underlying file or stream must support reading, but does not have to support seeking.
// If the underlying file or stream supports seeking, the files are read from the archive as they are requested.
// If the underlying file or stream does not support seeking, the entire archive is held in memory.
//
// We only implement Read + Seek and never hold anything in memory here.
//
/////////////////////////////////////////////////////////////////////////////////////////

use std::{
    borrow::BorrowMut,
    collections::HashMap,
    fs::{create_dir_all, File},
    io::{self, BufWriter, Read, Result, Seek, SeekFrom, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::*;
use crate::{fnv1a64_hash_string, io::FromReader};

use super::*;

#[derive(Debug)]
pub struct ZipArchiveReadonly<R>
where
    R: Read + Seek,
{
    /// wraps a read-only stream
    stream: R,
    /// The files inside an archive
    pub entries: HashMap<u64, ZipEntry>,
    pub dependencies: Vec<Dependency>,
}

/////////////////////////////////////////////////////////////////////////////////////////
// IMPL

impl<R> ZipArchiveReadonly<R>
where
    R: Read + Seek,
{
    /// Get an entry in the archive by resource path.
    pub fn get_entry(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.get(&fnv1a64_hash_string(&name.to_owned()))
    }

    /// Get an entry in the archive by hash (FNV1a64 of resource path).
    pub fn get_entry_by_hash(&self, hash: &u64) -> Option<&ZipEntry> {
        self.entries.get(hash)
    }

    /// Extracts a single entry to a directory path.
    ///
    /// # Errors
    ///
    /// This function will return an error if the entry cannot be found or any io fails.
    pub fn extract_entry<P: AsRef<Path>>(
        &mut self,
        entry: ZipEntry,
        destination_directory_name: &P,
        overwrite_files: bool,
        hash_map: &HashMap<u64, String>,
    ) -> Result<()> {
        let Some(info) = entry.get_resolved_name(&hash_map) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not get entry info from archive.",
            ));
        };

        // name or hash is a relative path
        let outfile = destination_directory_name.as_ref().join(info);
        create_dir_all(outfile.parent().expect("Could not create an out_dir"))?;

        // extract to stream
        let mut fs = if overwrite_files {
            File::create(outfile)?
        } else {
            File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(outfile)?
        };

        let writer = BufWriter::new(&mut fs);
        self.extract_segments(&entry, writer)?;

        Ok(())
    }

    /// Extracts a single entry by hash to a directory path.
    ///
    /// # Errors
    ///
    /// This function will return an error if the entry cannot be found or any io fails.
    pub fn extract_entry_by_hash<P: AsRef<Path>>(
        &mut self,
        hash: u64,
        destination_directory_name: &P,
        overwrite_files: bool,
        hash_map: &HashMap<u64, String>,
    ) -> Result<()> {
        if let Some(entry) = self.get_entry_by_hash(&hash) {
            self.extract_entry(
                entry.clone(),
                destination_directory_name,
                overwrite_files,
                hash_map,
            )
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find entry.",
            ));
        }
    }

    /// Extracts a single entry by resource path to a directory path.
    ///
    /// # Errors
    ///
    /// This function will return an error if the entry cannot be found or any io fails.
    pub fn extract_entry_by_name<P: AsRef<Path>>(
        &mut self,
        name: String,
        destination_directory_name: &P,
        overwrite_files: bool,
        hash_map: &HashMap<u64, String>,
    ) -> Result<()> {
        if let Some(entry) = self.get_entry(&name) {
            self.extract_entry(
                entry.clone(),
                destination_directory_name,
                overwrite_files,
                hash_map,
            )
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find entry.",
            ));
        }
    }

    /// Returns an open read stream to an entry of this [`ZipArchive<R>`].
    pub fn open_entry<W: Write>(&mut self, entry: ZipEntry, writer: W) -> Result<()> {
        self.extract_segments(&entry, writer)?;

        Ok(())
    }

    /// Extracts all entries to the given directory.
    ///
    /// # Errors
    ///
    /// This function will return an error if io fails.
    pub fn extract_to_directory<P: AsRef<Path>>(
        &mut self,
        destination_directory_name: &P,
        overwrite_files: bool,
        hash_map: Option<HashMap<u64, String>>,
    ) -> Result<()> {
        let hash_map = if let Some(hash_map) = hash_map {
            hash_map
        } else {
            get_red4_hashes()
        };

        // collect info
        let mut entries: Vec<ZipEntry> = vec![];
        for (_hash, entry) in &self.entries {
            entries.push(entry.clone());
        }

        for entry in entries {
            self.extract_entry(
                entry,
                destination_directory_name,
                overwrite_files,
                &hash_map,
            )?;
        }

        Ok(())
    }

    // getters

    fn reader_mut(&mut self) -> &mut R {
        self.stream.borrow_mut()
    }

    // methods

    /// Extracts segments to a writer, expects correct offset info.
    ///
    /// # Errors
    ///
    /// This function will return an error if io fails
    fn extract_segments<W: Write>(&mut self, entry: &ZipEntry, mut writer: W) -> Result<()> {
        let segment = entry.segment;
        let buffers = entry.buffers.clone();

        if segment.size() == segment.z_size() {
            // just copy
            self.reader_mut().seek(SeekFrom::Start(segment.offset()))?;
            let mut buffer = vec![0; segment.z_size() as usize];
            self.reader_mut().read_exact(&mut buffer[..])?;
            writer.write_all(&buffer)?;
        } else {
            decompress_segment(self.reader_mut(), &segment, &mut writer)?;
        }
        for segment in buffers {
            self.reader_mut().seek(SeekFrom::Start(segment.offset()))?;
            let mut buffer = vec![0; segment.z_size() as usize];
            self.reader_mut().read_exact(&mut buffer[..])?;
            writer.write_all(&buffer)?;
        }

        Ok(())
    }

    /// Opens an archive, needs to be read-only
    pub(crate) fn from_reader_consume(mut reader: R) -> Result<ZipArchiveReadonly<R>> {
        // read header
        let header = Header::from_reader(&mut reader)?;

        // read custom data
        let mut file_names: HashMap<u64, String> = HashMap::default();
        if let Ok(custom_data_length) = reader.read_u32::<LittleEndian>() {
            if custom_data_length > 0 {
                reader.seek(io::SeekFrom::Start(Header::HEADER_EXTENDED_SIZE))?;
                if let Ok(footer) = LxrsFooter::from_reader(&mut reader) {
                    // add files to hashmap
                    for f in footer.files() {
                        let hash = fnv1a64_hash_string(f);
                        file_names.insert(hash, f.to_owned());
                    }
                }
            }
        }

        // read index
        // move to offset Header.IndexPosition
        reader.seek(io::SeekFrom::Start(header.index_position()))?;
        let index = Index::from_reader(&mut reader)?;

        // read tables
        let mut file_entries: HashMap<u64, FileEntry> = HashMap::default();
        for _i in 0..index.file_entry_count() {
            let entry = FileEntry::from_reader(&mut reader)?;
            file_entries.insert(entry.name_hash_64(), entry);
        }

        let mut file_segments = Vec::default();
        for _i in 0..index.file_segment_count() {
            file_segments.push(FileSegment::from_reader(&mut reader)?);
        }

        // dependencies can't be connected to individual files anymore
        let mut dependencies = Vec::default();
        for _i in 0..index.resource_dependency_count() {
            dependencies.push(Dependency::from_reader(&mut reader)?);
        }

        // construct wrapper
        let mut entries = HashMap::default();
        for (hash, entry) in file_entries.iter() {
            let resolved = if let Some(name) = file_names.get(hash) {
                Some(name.to_owned())
            } else {
                None
            };

            let start_index = entry.segments_start();
            let next_index = entry.segments_end();
            if let Some(segment) = file_segments.get(start_index as usize) {
                let mut buffers: Vec<FileSegment> = vec![];
                for i in start_index + 1..next_index {
                    if let Some(buffer) = file_segments.get(i as usize) {
                        buffers.push(*buffer);
                    }
                }

                let zip_entry = ZipEntry {
                    hash: *hash,
                    name: resolved,
                    entry: *entry,
                    segment: *segment,
                    buffers,
                };
                entries.insert(*hash, zip_entry);
            }
        }

        let archive = ZipArchiveReadonly::<R> {
            stream: reader,
            entries,
            dependencies,
        };
        Ok(archive)
    }
}
