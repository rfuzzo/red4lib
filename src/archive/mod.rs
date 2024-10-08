/////////////////////////////////////////////////////////////////////////////////////////
// ARCHIVE
/////////////////////////////////////////////////////////////////////////////////////////

use std::{
    borrow::BorrowMut,
    collections::HashMap,
    fs::{create_dir_all, File},
    io::{self, BufWriter, Cursor, Error, ErrorKind, Read, Result, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use strum::IntoEnumIterator;
use walkdir::WalkDir;

use crate::kraken::*;
use crate::{cr2w::*, *};
use crate::{fnv1a64_hash_string, io::FromReader};

use self::{dependency::*, file_entry::*, file_segment::*, header::*, index::*, lxrs::*};

mod dependency;
mod file_entry;
mod file_segment;
mod header;
mod index;
mod lxrs;

/////////////////////////////////////////////////////////////////////////////////////////
// ARCHIVE_FILE
// https://learn.microsoft.com/en-us/dotnet/api/system.io.compression.zipfile?view=net-8.0#methods
// ZipFile -> namespace
// Provides static methods for creating, extracting, and opening zip archives.
//
// ZipArchive -> Archive
// Represents a package of compressed files in the zip archive format.
//
// ZipArchiveEntry -> ArchiveEntry
// Represents a compressed file within a zip archive.
/////////////////////////////////////////////////////////////////////////////////////////

// public static void CreateFromDirectory (string sourceDirectoryName, System.IO.Stream destination);

/// Creates an archive in the specified stream that contains the files and directories from the specified directory.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn create_from_directory<P, W>(
    source_directory_name: &P,
    destination: W,
    hash_map: Option<HashMap<u64, String>>,
) -> Result<()>
where
    P: AsRef<Path>,
    W: Write + Seek,
{
    let map = if let Some(hash_map) = hash_map {
        hash_map
    } else {
        get_red4_hashes()
    };

    write_archive(source_directory_name, destination, map)
}

// public static void CreateFromDirectory (string sourceDirectoryName, string destinationArchiveFileName);

/// Creates an archive that contains the files and directories from the specified directory.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn create_from_directory_path<P>(
    source_directory_name: &P,
    destination: &P,
    hash_map: Option<HashMap<u64, String>>,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let map = if let Some(hash_map) = hash_map {
        hash_map
    } else {
        get_red4_hashes()
    };

    let fs: File = File::create(destination)?;
    write_archive(source_directory_name, fs, map)
}

// public static void ExtractToDirectory (System.IO.Stream source, string destinationDirectoryName, bool overwriteFiles);

/// Extracts all the files from the archive stored in the specified stream and places them in the specified destination directory on the file system, and optionally allows choosing if the files in the destination directory should be overwritten.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn extract_to_directory<R, P>(
    source: &mut R,
    destination_directory_name: &P,
    overwrite_files: bool,
    hash_map: Option<HashMap<u64, String>>,
) -> Result<()>
where
    P: AsRef<Path>,
    R: Read + Seek + 'static,
{
    let mut archive = ZipArchive::from_reader_consume(source, ArchiveMode::Read)?;
    archive.extract_to_directory(destination_directory_name, overwrite_files, hash_map)
}

// public static void ExtractToDirectory (string sourceArchiveFileName, string destinationDirectoryName, bool overwriteFiles);

/// Extracts all of the files in the specified archive to a directory on the file system.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn extract_to_directory_path<P, R>(
    source_archive_file_name: &P,
    destination_directory_name: &P,
    overwrite_files: bool,
    hash_map: Option<HashMap<u64, String>>,
) -> Result<()>
where
    P: AsRef<Path>,
    R: Read + Seek,
{
    let mut archive = open_read(source_archive_file_name)?;
    archive.extract_to_directory(destination_directory_name, overwrite_files, hash_map)
}

// public static System.IO.Compression.ZipArchive Open (string archiveFileName, System.IO.Compression.ZipArchiveMode mode);

/// Opens an archive at the specified path and in the specified mode.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn open<P>(archive_file_name: P, mode: ArchiveMode) -> Result<ZipArchive<File>>
where
    P: AsRef<Path>,
{
    match mode {
        ArchiveMode::Create => {
            let file = File::create(archive_file_name)?;
            ZipArchive::from_reader_consume(file, mode)
        }
        ArchiveMode::Read => open_read(archive_file_name),
        ArchiveMode::Update => {
            let file = File::open(archive_file_name)?;
            ZipArchive::from_reader_consume(file, mode)
        }
    }
}

// public static System.IO.Compression.ZipArchive OpenRead (string archiveFileName);

/// Opens an archive for reading at the specified path.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn open_read<P>(archive_file_name: P) -> Result<ZipArchive<File>>
where
    P: AsRef<Path>,
{
    let file = File::open(archive_file_name)?;
    ZipArchive::from_reader_consume(file, ArchiveMode::Read)
}

/// Packs redengine 4 resource file in a folder to an archive
///
/// # Panics
///
/// Panics if any path conversions fail
///
/// # Errors
///
/// This function will return an error if any parsing or IO fails
fn write_archive<P, W>(in_folder: &P, out_stream: W, hash_map: HashMap<u64, String>) -> Result<()>
where
    P: AsRef<Path>,
    W: Write + Seek,
{
    if !in_folder.as_ref().exists() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Input folder does not exist",
        ));
    }
    // get files
    let resources = collect_resource_files(in_folder);

    // get paths and sort by hash
    let mut file_info = resources
        .iter()
        .filter_map(|f| {
            if let Ok(relative_path) = f.strip_prefix(in_folder) {
                if let Some(path_str) = relative_path.to_str() {
                    let hash = fnv1a64_hash_string(&path_str.to_string());
                    return Some((f.clone(), hash));
                }
            }
            None
        })
        .collect::<Vec<_>>();
    file_info.sort_by_key(|k| k.1);

    let custom_paths = file_info
        .iter()
        .filter(|(_p, k)| hash_map.contains_key(k))
        .filter_map(|(f, _h)| {
            if let Ok(path) = f.strip_prefix(in_folder) {
                return Some(path.to_string_lossy().to_string());
            }
            None
        })
        .collect::<Vec<_>>();

    // start write

    let mut archive_writer = BufWriter::new(out_stream);

    // write empty header
    archive_writer.write_all(&[0u8; Header::HEADER_SIZE])?; //write empty header
    archive_writer.write_all(&[0u8; 132])?; // padding

    // write custom header
    let mut custom_data_length = 0;
    if !custom_paths.is_empty() {
        let wfooter = LxrsFooter::new(custom_paths);
        wfooter.write(&mut archive_writer)?;
        custom_data_length = archive_writer.stream_position()? - Header::HEADER_EXTENDED_SIZE;
    }

    // write files
    //let imports_hash_set: HashSet<String> = HashSet::new();
    let mut entries = HashMap::default();
    for (path, hash) in file_info {
        let wrapped_entry = make_entry(path, &mut archive_writer, hash)?;

        entries.insert(hash, wrapped_entry);
    }

    // run through entries again and enumerate the segments
    let mut file_segments_cnt = 0;
    for (_hash, entry) in entries.iter_mut() {
        let firstoffsetidx = file_segments_cnt;
        file_segments_cnt += entry.buffers.len() + 1;
        let lastoffsetidx = file_segments_cnt;
        entry.entry.set_segments_start(firstoffsetidx as u32);
        entry.entry.set_segments_end(lastoffsetidx as u32);
    }

    // write footers
    // let dependencies = imports_hash_set
    //     .iter()
    //     .map(|e| Dependency::new(fnv1a64_hash_string(e)))
    //     .collect::<Vec<_>>();

    // padding
    pad_until_page(&mut archive_writer)?;

    // write tables
    let tableoffset = archive_writer.stream_position()?;
    write_index(&mut archive_writer, &entries /*, &dependencies */)?;
    let tablesize = archive_writer.stream_position()? - tableoffset;

    // padding
    pad_until_page(&mut archive_writer)?;

    // write the header again
    let filesize = archive_writer.stream_position()?;
    let header = Header::new(tableoffset, tablesize as u32, 0, 0, filesize);
    archive_writer.seek(SeekFrom::Start(0))?;
    header.write(&mut archive_writer)?;
    archive_writer.write_u32::<LittleEndian>(custom_data_length as u32)?;

    Ok(())
}

fn make_entry<W: Write + Seek>(
    path: PathBuf,
    archive_writer: &mut BufWriter<W>,
    hash: u64,
) -> Result<ZipEntry> {
    let mut file = File::open(&path)?;
    let mut file_buffer = Vec::new();
    file.read_to_end(&mut file_buffer)?;
    let mut file_cursor = Cursor::new(&file_buffer);

    let mut flags = 0;
    let segment: FileSegment;
    let mut buffers = vec![];

    if let Ok(info) = read_cr2w_header(&mut file_cursor) {
        // get main file
        file_cursor.seek(SeekFrom::Start(0))?;
        let size = info.header.objects_end;
        let mut resource_buffer = vec![0; size as usize];
        file_cursor.read_exact(&mut resource_buffer[..])?;
        // get archive offset before writing
        let archive_offset = archive_writer.stream_position()?;

        // kark file
        let compressed_size_needed = get_compressed_buffer_size_needed(size as u64);
        let mut compressed_buffer = vec![0; compressed_size_needed as usize];
        let zsize = compress(
            &resource_buffer,
            &mut compressed_buffer,
            CompressionLevel::Normal,
        );
        assert!((zsize as u32) <= size);
        compressed_buffer.resize(zsize as usize, 0);

        // write compressed main file archive
        // KARK header
        archive_writer.write_u32::<LittleEndian>(kraken::MAGIC)?; //magic
        archive_writer.write_u32::<LittleEndian>(size)?; //uncompressed buffer length
        archive_writer.write_all(&compressed_buffer)?;

        // add metadata to archive
        segment = FileSegment::new(archive_offset, zsize as u32, size);

        // write buffers (bytes after the main file)
        for buffer_info in info.buffers_table.iter() {
            let mut buffer = vec![0; buffer_info.disk_size as usize];
            file_cursor.read_exact(&mut buffer[..])?;

            let bsize = buffer_info.mem_size;
            let bzsize = buffer_info.disk_size;
            let boffset = archive_writer.stream_position()?;
            archive_writer.write_all(buffer.as_slice())?;

            // add metadata to archive
            buffers.push(FileSegment::new(boffset, bzsize, bsize));
        }

        //register imports
        // NOTE don't use a dependency list for mods
        //for import in info.imports.iter() {
        // if (cr2WImportWrapper.Flags is not InternalEnums.EImportFlags.Soft and not InternalEnums.EImportFlags.Embedded)
        //imports_hash_set.insert(import.depot_path.to_owned());
        //}

        //lastimportidx = imports_hash_set.len();

        flags = if !info.buffers_table.is_empty() {
            info.buffers_table.len() - 1
        } else {
            0
        };
    } else {
        // write non-cr2w file
        file_cursor.seek(SeekFrom::Start(0))?;
        let os_ext = path.extension().unwrap();
        let ext = os_ext.to_ascii_lowercase().to_string_lossy().to_string();
        if get_aligned_file_extensions().contains(&ext) {
            pad_until_page(archive_writer)?;
        }

        let offset = archive_writer.stream_position()?;
        let size = file_buffer.len() as u32;
        let final_zsize;
        if get_uncompressed_file_extensions().contains(&ext) {
            // direct copy
            archive_writer.write_all(&file_buffer)?;
            final_zsize = size;
        } else {
            // kark file
            let compressed_size_needed = get_compressed_buffer_size_needed(size as u64);
            let mut compressed_buffer = vec![0; compressed_size_needed as usize];
            let zsize = compress(
                &file_buffer,
                &mut compressed_buffer,
                CompressionLevel::Normal,
            );
            assert!((zsize as u32) <= size);
            compressed_buffer.resize(zsize as usize, 0);
            final_zsize = zsize as u32;
            // write
            archive_writer.write_all(&compressed_buffer)?;
        }

        // add metadata to archive
        segment = FileSegment::new(offset, final_zsize, size);
    }
    let sha1_hash = sha1_hash_file(&file_buffer);
    let entry = FileEntry::new(
        hash,
        0,
        flags as u32,
        0, //firstoffsetidx as u32,
        0, //lastoffsetidx as u32,
        0, //firstimportidx as u32,
        0, //lastimportidx as u32,
        sha1_hash,
    );
    let wrapped_entry = ZipEntry {
        hash,
        name: None,
        entry,
        segment,
        buffers,
    };
    Ok(wrapped_entry)
}

fn collect_resource_files<P: AsRef<Path>>(in_folder: &P) -> Vec<PathBuf> {
    // collect files
    let mut included_extensions = ERedExtension::iter()
        .map(|variant| variant.to_string())
        .collect::<Vec<_>>();
    included_extensions.push(String::from("bin"));

    // get only resource files
    let allfiles = WalkDir::new(in_folder)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|f| f.into_path())
        .filter(|p| {
            if let Some(ext) = p.extension() {
                if let Some(ext) = ext.to_str() {
                    return included_extensions.contains(&ext.to_owned());
                }
            }
            false
        })
        .collect::<Vec<_>>();
    allfiles
}

/// Decompresses and writes a kraken-compressed segment from an archive to a stream
///
/// # Errors
///
/// This function will return an error if .
fn decompress_segment<R: Read + Seek, W: Write>(
    archive_reader: &mut R,
    segment: &FileSegment,
    file_writer: &mut W,
) -> Result<()> {
    archive_reader.seek(SeekFrom::Start(segment.offset()))?;

    let magic = archive_reader.read_u32::<LittleEndian>()?;
    if magic == kraken::MAGIC {
        // read metadata
        let mut size = segment.size();
        let size_in_header = archive_reader.read_u32::<LittleEndian>()?;
        if size_in_header != size {
            size = size_in_header;
        }
        let mut compressed_buffer = vec![0; segment.z_size() as usize - 8];
        archive_reader.read_exact(&mut compressed_buffer[..])?;
        let mut output_buffer = vec![];
        let result = decompress(compressed_buffer, &mut output_buffer, size as usize);
        assert_eq!(result as u32, size);

        // write
        file_writer.write_all(&output_buffer)?;
    } else {
        // incorrect data, fall back to direct copy
        archive_reader.seek(SeekFrom::Start(segment.offset()))?;
        let mut buffer = vec![0; segment.z_size() as usize];
        archive_reader.read_exact(&mut buffer[..])?;
        file_writer.write_all(&buffer)?;
    };

    Ok(())
}

/// .
fn get_aligned_file_extensions() -> Vec<String> {
    let files = vec![".bk2", ".bnk", ".opusinfo", ".wem", ".bin"];
    files.into_iter().map(|f| f.to_owned()).collect::<Vec<_>>()
}

/// .
fn get_uncompressed_file_extensions() -> Vec<String> {
    let files = vec![
        ".bk2",
        ".bnk",
        ".opusinfo",
        ".wem",
        ".bin",
        ".dat",
        ".opuspak",
    ];
    files.into_iter().map(|f| f.to_owned()).collect::<Vec<_>>()
}

fn pad_until_page<W: Write + Seek>(writer: &mut W) -> Result<()> {
    let pos = writer.stream_position()?;
    let modulo = pos / 4096;
    let diff = ((modulo + 1) * 4096) - pos;
    let padding = vec![0xD9; diff as usize];
    writer.write_all(padding.as_slice())?;

    Ok(())
}

/////////////////////////////////////////////////////////////////////////////////////////
// API
/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ArchiveMode {
    #[default]
    Create,
    Read,
    Update,
}

#[derive(Debug)]
pub struct ZipArchive<S> {
    /// wraps a stream
    stream: S,

    /// The read-write mode of the archive
    mode: ArchiveMode,
    dirty: bool,
    /// The files inside an archive
    entries: HashMap<u64, ZipEntry>,
    pub dependencies: Vec<Dependency>,
}

impl<S> ZipArchive<S> {
    /// Get an entry in the archive by resource path.
    pub fn get_entry(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.get(&fnv1a64_hash_string(&name.to_owned()))
    }

    /// Get an entry in the archive by hash (FNV1a64 of resource path).
    pub fn get_entry_by_hash(&self, hash: &u64) -> Option<&ZipEntry> {
        self.entries.get(hash)
    }

    pub fn get_entries(&self) -> &HashMap<u64, ZipEntry> {
        &self.entries
    }
}

impl<R> ZipArchive<R>
where
    R: Read + Seek,
{
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
        let Some(info) = entry.get_resolved_name(hash_map) else {
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
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find entry.",
            ))
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
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not find entry.",
            ))
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
        for entry in self.entries.values() {
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
    fn from_reader_consume(mut reader: R, mode: ArchiveMode) -> Result<ZipArchive<R>> {
        // checks
        if mode == ArchiveMode::Create {
            return Ok(ZipArchive::<R> {
                stream: reader,
                mode,
                dirty: true,
                entries: HashMap::default(),
                dependencies: Vec::default(),
            });
        }

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
            let resolved = file_names.get(hash).map(|name| name.to_owned());

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

        let archive = ZipArchive::<R> {
            stream: reader,
            mode,
            entries,
            dependencies,
            dirty: false,
        };
        Ok(archive)
    }
}

impl<W: Write + Seek> ZipArchive<W> {
    #[allow(dead_code)] // TODO: Implement
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
        if self.mode != ArchiveMode::Update {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Archive is in read-only mode.",
            ));
        }

        // write?

        // set dirty
        self.dirty = true;

        todo!()
    }

    /// Deletes an entry from the archive
    pub fn delete_entry(&mut self, hash: &u64) -> Option<ZipEntry> {
        // can only delete entries in update mode
        if self.mode != ArchiveMode::Update {
            return None;
        }

        // Set dirty
        self.dirty = true;

        self.entries.remove(hash)
    }
}

#[derive(Debug, Clone)]
pub struct ZipEntry {
    /// FNV1a64 hash of the entry name
    pub hash: u64,
    /// Resolved resource path of that entry, this may not be available
    name: Option<String>,

    /// wrapped internal struct
    pub entry: FileEntry,
    segment: FileSegment,
    buffers: Vec<FileSegment>,
}

impl ZipEntry {
    fn get_resolved_name(&self, hash_map: &HashMap<u64, String>) -> Option<String> {
        // get filename
        let resolved = if let Some(name) = &self.name {
            name.to_owned()
        } else {
            let mut name_or_hash: String = format!("{}.bin", self.hash);
            // check vanilla hashes
            if let Some(name) = hash_map.get(&self.hash) {
                name_or_hash = name.to_owned();
            }
            name_or_hash
        };

        Some(resolved)
    }
}

/////////////////////////////////////////////////////////////////////////////////////////
// INTERNAL
/////////////////////////////////////////////////////////////////////////////////////////

fn write_index<W: Write>(
    writer: &mut W,
    entries: &HashMap<u64, ZipEntry>,
    //dependencies: &[Dependency],
) -> Result<()> {
    let file_entry_count = entries.len() as u32;
    let buffer_counts = entries.iter().map(|e| e.1.buffers.len() + 1);
    let file_segment_count = buffer_counts.sum::<usize>() as u32;
    let resource_dependency_count = 0; //dependencies.len() as u32;

    // write table to buffer
    let mut buffer: Vec<u8> = Vec::new();
    buffer.write_u32::<LittleEndian>(file_entry_count)?;
    buffer.write_u32::<LittleEndian>(file_segment_count)?;
    buffer.write_u32::<LittleEndian>(resource_dependency_count)?;
    let mut entries = entries.values().collect::<Vec<_>>();
    entries.sort_by_key(|e| e.hash);
    // write entries
    let mut segments = Vec::default();
    for entry in entries {
        entry.entry.write(&mut buffer)?;
        // collect offsets
        segments.push(entry.segment);
        for buffer in &entry.buffers {
            segments.push(*buffer);
        }
    }
    // write segments
    for segment in segments {
        segment.write(&mut buffer)?;
    }

    // write dependencies
    // for dependency in dependencies {
    //     dependency.write(&mut buffer)?;
    // }

    // write to out stream
    let crc = crc64::crc64(0, buffer.as_slice());
    writer.write_u32::<LittleEndian>(8)?;
    writer.write_u32::<LittleEndian>(buffer.len() as u32 + 8)?;
    writer.write_u64::<LittleEndian>(crc)?;
    writer.write_all(buffer.as_slice())?;

    Ok(())
}

/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod integration_tests {
    use std::{
        fs::{self},
        io::{self, Read},
        path::PathBuf,
    };

    use crate::archive::open_read;

    use super::FromReader;
    use super::LxrsFooter;

    #[test]
    fn read_srxl() {
        let file_path = PathBuf::from("tests").join("srxl.bin");
        let mut file = fs::File::open(file_path).expect("Could not open file");
        let mut buffer: Vec<u8> = vec![];
        file.read_to_end(&mut buffer).expect("Could not read file");

        let mut cursor = io::Cursor::new(&buffer);

        let _srxl = LxrsFooter::from_reader(&mut cursor).unwrap();
    }

    #[test]
    fn read_archive() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let result = open_read(archive_path);
        assert!(result.is_ok());
    }

    #[test]
    fn read_archive2() {
        let file = PathBuf::from("tests").join("nci.archive");
        let result = open_read(file);
        assert!(result.is_ok());
    }

    #[test]
    fn read_custom_data() {
        let file = PathBuf::from("tests").join("test1.archive");
        let archive = open_read(file).expect("Could not parse archive");
        let mut file_names = archive
            .entries
            .values()
            .filter_map(|f| f.name.to_owned())
            .collect::<Vec<_>>();
        file_names.sort();

        let expected: Vec<String> = vec!["base\\cycleweapons\\localization\\en-us.json".to_owned()];
        assert_eq!(expected, file_names);
    }
}
