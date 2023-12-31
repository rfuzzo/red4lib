/////////////////////////////////////////////////////////////////////////////////////////
/// ARCHIVE
/////////////////////////////////////////////////////////////////////////////////////////
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::io::{self, BufWriter, Read, Result, Seek, SeekFrom, Write};
use std::mem;
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use strum::IntoEnumIterator;
use walkdir::WalkDir;

use crate::cr2w::read_cr2w_header;
use crate::io::{read_null_terminated_string, write_null_terminated_string, FromReader};
use crate::kraken::{
    self, compress, decompress, get_compressed_buffer_size_needed, CompressionLevel,
};
use crate::{fnv1a64_hash_string, sha1_hash_file, ERedExtension};

#[derive(Debug, Clone, Default)]
pub struct Archive {
    pub header: Header,
    pub index: Index,

    // custom
    pub file_names: HashMap<u64, String>,
}

impl Archive {
    // Function to read a Header from a file
    pub fn from_file(file_path: &PathBuf) -> Result<Archive> {
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::with_capacity(mem::size_of::<Header>());

        file.read_to_end(&mut buffer)?;

        // Ensure that the buffer has enough bytes to represent a Header
        if buffer.len() < mem::size_of::<Header>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "File does not contain enough data to parse Header",
            ));
        }

        let mut cursor = io::Cursor::new(&buffer);
        let header = Header::from_reader(&mut cursor)?;

        // read custom data
        let mut file_names: HashMap<u64, String> = HashMap::default();
        if let Ok(custom_data_length) = cursor.read_u32::<LittleEndian>() {
            if custom_data_length > 0 {
                cursor.set_position(HEADER_EXTENDED_SIZE);
                if let Ok(footer) = LxrsFooter::from_reader(&mut cursor) {
                    // add files to hashmap
                    for f in footer.files {
                        let hash = fnv1a64_hash_string(&f);
                        file_names.insert(hash, f);
                    }
                }
            }
        }

        // move to offset Header.IndexPosition
        cursor.set_position(header.index_position);
        let index = Index::from_reader(&mut cursor)?;

        Ok(Archive {
            header,
            index,
            file_names,
        })
    }

    // get filehashes
    pub fn get_file_hashes(&self) -> Vec<u64> {
        self.index
            .file_entries
            .iter()
            .map(|f| f.1.name_hash_64)
            .collect::<Vec<_>>()
    }
}

//static HEADER_MAGIC: u32 = 1380009042;
//static HEADER_SIZE: i32 = 40;
static HEADER_EXTENDED_SIZE: u64 = 0xAC;

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub magic: u32,
    pub version: u32,
    pub index_position: u64,
    pub index_size: u32,
    pub debug_position: u64,
    pub debug_size: u32,
    pub filesize: u64,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            magic: 1380009042,
            version: 12,
            index_position: Default::default(),
            index_size: Default::default(),
            debug_position: Default::default(),
            debug_size: Default::default(),
            filesize: Default::default(),
        }
    }
}

impl FromReader for Header {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Header {
            magic: reader.read_u32::<LittleEndian>()?,
            version: reader.read_u32::<LittleEndian>()?,
            index_position: reader.read_u64::<LittleEndian>()?,
            index_size: reader.read_u32::<LittleEndian>()?,
            debug_position: reader.read_u64::<LittleEndian>()?,
            debug_size: reader.read_u32::<LittleEndian>()?,
            filesize: reader.read_u64::<LittleEndian>()?,
        })
    }
}
impl Header {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_u64::<LittleEndian>(self.index_position)?;
        writer.write_u32::<LittleEndian>(self.index_size)?;
        writer.write_u64::<LittleEndian>(self.debug_position)?;
        writer.write_u32::<LittleEndian>(self.debug_size)?;
        writer.write_u64::<LittleEndian>(self.filesize)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    pub file_table_offset: u32,
    pub file_table_size: u32,
    pub crc: u64,
    pub file_entry_count: u32,
    pub file_segment_count: u32,
    pub resource_dependency_count: u32,

    // not serialized
    pub file_entries: HashMap<u64, FileEntry>,
    pub file_segments: Vec<FileSegment>,
    pub dependencies: Vec<Dependency>,
}
impl Index {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.file_table_offset)?;
        writer.write_u32::<LittleEndian>(self.file_table_size)?;
        writer.write_u64::<LittleEndian>(self.crc)?;
        writer.write_u32::<LittleEndian>(self.file_entry_count)?;
        writer.write_u32::<LittleEndian>(self.file_segment_count)?;
        writer.write_u32::<LittleEndian>(self.resource_dependency_count)?;

        Ok(())
    }
}
impl FromReader for Index {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        let mut index = Index {
            file_table_offset: cursor.read_u32::<LittleEndian>()?,
            file_table_size: cursor.read_u32::<LittleEndian>()?,
            crc: cursor.read_u64::<LittleEndian>()?,
            file_entry_count: cursor.read_u32::<LittleEndian>()?,
            file_segment_count: cursor.read_u32::<LittleEndian>()?,
            resource_dependency_count: cursor.read_u32::<LittleEndian>()?,

            file_entries: HashMap::default(),
            file_segments: vec![],
            dependencies: vec![],
        };

        // read tables
        for _i in 0..index.file_entry_count {
            let entry = FileEntry::from_reader(cursor)?;
            index.file_entries.insert(entry.name_hash_64, entry);
        }

        for _i in 0..index.file_segment_count {
            index.file_segments.push(FileSegment::from_reader(cursor)?);
        }

        for _i in 0..index.resource_dependency_count {
            index.dependencies.push(Dependency::from_reader(cursor)?);
        }

        // ignore the rest of the archive

        Ok(index)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileSegment {
    pub offset: u64,
    pub z_size: u32,
    pub size: u32,
}

impl FromReader for FileSegment {
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        Ok(FileSegment {
            offset: reader.read_u64::<LittleEndian>()?,
            z_size: reader.read_u32::<LittleEndian>()?,
            size: reader.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileEntry {
    pub name_hash_64: u64,
    pub timestamp: u64, //SystemTime,
    pub num_inline_buffer_segments: u32,
    pub segments_start: u32,
    pub segments_end: u32,
    pub resource_dependencies_start: u32,
    pub resource_dependencies_end: u32,
    pub sha1_hash: [u8; 20],
}

impl FromReader for FileEntry {
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut entry = FileEntry {
            name_hash_64: reader.read_u64::<LittleEndian>()?,
            timestamp: reader.read_u64::<LittleEndian>()?,
            num_inline_buffer_segments: reader.read_u32::<LittleEndian>()?,
            segments_start: reader.read_u32::<LittleEndian>()?,
            segments_end: reader.read_u32::<LittleEndian>()?,
            resource_dependencies_start: reader.read_u32::<LittleEndian>()?,
            resource_dependencies_end: reader.read_u32::<LittleEndian>()?,
            sha1_hash: [0; 20],
        };

        reader.read_exact(&mut entry.sha1_hash[..])?;

        Ok(entry)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dependency {
    pub hash: u64,
}

impl FromReader for Dependency {
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        Ok(Dependency {
            hash: reader.read_u64::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LxrsFooter {
    pub files: Vec<String>,
}

impl LxrsFooter {
    //const MINLEN: u32 = 20;
    const MAGIC: u32 = 0x4C585253;
    const VERSION: u32 = 1;

    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.files.len() as u32)?;
        writer.write_u32::<LittleEndian>(LxrsFooter::VERSION)?;

        // write strings to buffer
        let mut buffer: Vec<u8> = Vec::new();
        for f in &self.files {
            write_null_terminated_string(&mut buffer, f.to_owned())?;
        }

        // compress
        let size = buffer.len();
        let compressed_size_needed = get_compressed_buffer_size_needed(size as u64);
        let mut compressed_buffer = vec![0; compressed_size_needed as usize];
        let zsize = compress(&buffer, &mut compressed_buffer, CompressionLevel::Normal);
        assert!((zsize as u32) <= size as u32);
        compressed_buffer.resize(zsize as usize, 0);

        // write to writer
        writer.write_all(&compressed_buffer)?;

        Ok(())
    }
}
impl FromReader for LxrsFooter {
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic != LxrsFooter::MAGIC {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid magic"));
        }
        let _version = reader.read_u32::<LittleEndian>()?;
        let size = reader.read_u32::<LittleEndian>()?;
        let zsize = reader.read_u32::<LittleEndian>()?;
        let count = reader.read_i32::<LittleEndian>()?;

        let mut files: Vec<String> = vec![];
        match size.cmp(&zsize) {
            Ordering::Greater => {
                // buffer is compressed
                let mut compressed_buffer = vec![0; zsize as usize];
                reader.read_exact(&mut compressed_buffer[..])?;
                let mut output_buffer = vec![];
                let result = decompress(compressed_buffer, &mut output_buffer, size as usize);
                assert_eq!(result as u32, size);

                // read from buffer
                let mut inner_cursor = io::Cursor::new(&output_buffer);
                for _i in 0..count {
                    // read NullTerminatedString
                    if let Ok(string) = read_null_terminated_string(&mut inner_cursor) {
                        files.push(string);
                    }
                }
            }
            Ordering::Less => {
                // error
                return Err(io::Error::new(io::ErrorKind::Other, "invalid buffer"));
            }
            Ordering::Equal => {
                // no compression
                for _i in 0..count {
                    // read NullTerminatedString
                    if let Ok(string) = read_null_terminated_string(reader) {
                        files.push(string);
                    }
                }
            }
        }

        let footer = LxrsFooter { files };

        Ok(footer)
    }
}

fn pad_until_page<W: Write + Seek>(writer: &mut W) -> io::Result<()> {
    let pos = writer.stream_position()?;
    let modulo = pos / 4096;
    let diff = ((modulo + 1) * 4096) - pos;
    let padding = vec![0xD9; diff as usize];
    writer.write_all(padding.as_slice())?;

    Ok(())
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
    archive_reader.seek(SeekFrom::Start(segment.offset))?;

    let magic = archive_reader.read_u32::<LittleEndian>()?;
    if magic == kraken::MAGIC {
        // read metadata
        let mut size = segment.size;
        let size_in_header = archive_reader.read_u32::<LittleEndian>()?;
        if size_in_header != size {
            size = size_in_header;
        }
        let mut compressed_buffer = vec![0; segment.z_size as usize - 8];
        archive_reader.read_exact(&mut compressed_buffer[..])?;
        let mut output_buffer = vec![];
        let result = decompress(compressed_buffer, &mut output_buffer, size as usize);
        assert_eq!(result as u32, size);

        // write
        file_writer.write_all(&output_buffer)?;
    } else {
        // incorrect data, fall back to direct copy
        archive_reader.seek(SeekFrom::Start(segment.offset))?;
        let mut buffer = vec![0; segment.z_size as usize];
        archive_reader.read_exact(&mut buffer[..])?;
        file_writer.write_all(&buffer)?;
    };

    Ok(())
}

/////////////////////////////////////////////////////////////////////////////////////////
/// Lib
/////////////////////////////////////////////////////////////////////////////////////////

/// Extracts all files from an archive and writes them to a folder
///
/// # Panics
///
/// Panics if file path operations fail
///
/// # Errors
///
/// This function will return an error if any parsing fails
pub fn extract_archive(
    in_file: &PathBuf,
    out_dir: &Path,
    hash_map: &HashMap<u64, String>,
) -> io::Result<()> {
    // parse archive headers
    let archive = Archive::from_file(in_file)?;

    let archive_file = File::open(in_file)?;
    let mut archive_reader = io::BufReader::new(archive_file);

    for (hash, file_entry) in archive.index.file_entries.iter() {
        // get filename
        let mut name_or_hash: String = format!("{}.bin", hash);
        if let Some(name) = hash_map.get(hash) {
            name_or_hash = name.to_owned();
        }
        if let Some(name) = archive.file_names.get(hash) {
            name_or_hash = name.to_owned();
        }

        // name or hash is a relative path
        let outfile = out_dir.join(name_or_hash);
        create_dir_all(outfile.parent().expect("Could not create an out_dir"))?;

        // extract to stream
        let mut fs = File::create(outfile)?;
        let mut file_writer = BufWriter::new(&mut fs);
        // decompress main file
        let start_index = file_entry.segments_start;
        let next_index = file_entry.segments_end;
        if let Some(segment) = archive.index.file_segments.get(start_index as usize) {
            // read and decompress from main archive stream

            // kraken decompress
            if segment.size == segment.z_size {
                // just copy over
                archive_reader.seek(SeekFrom::Start(segment.offset))?;
                let mut buffer = vec![0; segment.z_size as usize];
                archive_reader.read_exact(&mut buffer[..])?;
                file_writer.write_all(&buffer)?;
            } else {
                decompress_segment(&mut archive_reader, segment, &mut file_writer)?;
            }
        }

        // extract additional buffers
        for i in start_index + 1..next_index {
            if let Some(segment) = archive.index.file_segments.get(i as usize) {
                // do not decompress with oodle
                archive_reader.seek(SeekFrom::Start(segment.offset))?;
                let mut buffer = vec![0; segment.z_size as usize];
                archive_reader.read_exact(&mut buffer[..])?;
                file_writer.write_all(&buffer)?;
            }
        }
    }

    Ok(())
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
pub fn write_archive(
    in_folder: &Path,
    out_folder: &Path,
    modname: Option<&str>,
    hash_map: HashMap<u64, String>,
) -> io::Result<()> {
    if !in_folder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }

    if !out_folder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }

    let archive_name = if let Some(name) = modname {
        format!("{}.archive", name)
    } else {
        let folder_name = in_folder.file_name().expect("Could not get in_dir name");
        format!("{}.archive", folder_name.to_string_lossy())
    };

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

    // sort by hash
    let mut hashed_paths = allfiles
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
    hashed_paths.sort_by_key(|k| k.1);

    let outfile = out_folder.join(archive_name);
    let mut fs = File::create(outfile)?;
    let mut archive_writer = BufWriter::new(&mut fs);

    // write temp header
    let mut archive = Archive::default();
    let header = Header::default();
    header.serialize(&mut archive_writer)?;
    archive_writer.write_all(&[0u8; 132])?; // some weird padding

    // write custom header
    assert_eq!(HEADER_EXTENDED_SIZE, archive_writer.stream_position()?);
    let custom_paths = hashed_paths
        .iter()
        .filter(|(_p, k)| hash_map.contains_key(k))
        .filter_map(|(f, _h)| {
            if let Ok(path) = f.strip_prefix(in_folder) {
                return Some(path.to_string_lossy().to_string());
            }
            None
        })
        .collect::<Vec<_>>();

    let mut custom_data_length = 0;
    if !custom_paths.is_empty() {
        let wfooter = LxrsFooter {
            files: custom_paths,
        };
        wfooter.serialize(&mut archive_writer)?;
        custom_data_length = archive_writer.stream_position()? - HEADER_EXTENDED_SIZE;
    }

    // write files
    let mut imports_hash_set: HashSet<String> = HashSet::new();
    for (path, hash) in hashed_paths {
        // read file
        let mut file = File::open(&path)?;
        let mut file_buffer = Vec::new();
        file.read_to_end(&mut file_buffer)?;

        let firstimportidx = imports_hash_set.len();
        let mut lastimportidx = imports_hash_set.len();
        let firstoffsetidx = archive.index.file_segments.len();
        let mut lastoffsetidx = 0;
        let mut flags = 0;

        let mut file_cursor = io::Cursor::new(&file_buffer);
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
            archive.index.file_segments.push(FileSegment {
                offset: archive_offset,
                size,
                z_size: zsize as u32,
            });

            // write buffers (bytes after the main file)
            for buffer_info in info.buffers_table.iter() {
                let mut buffer = vec![0; buffer_info.disk_size as usize];
                file_cursor.read_exact(&mut buffer[..])?;

                let bsize = buffer_info.mem_size;
                let bzsize = buffer_info.disk_size;
                let boffset = archive_writer.stream_position()?;
                archive_writer.write_all(buffer.as_slice())?;

                // add metadata to archive
                archive.index.file_segments.push(FileSegment {
                    offset: boffset,
                    size: bsize,
                    z_size: bzsize,
                });
            }

            //register imports
            for import in info.imports.iter() {
                // TODO fix flags
                // if (cr2WImportWrapper.Flags is not InternalEnums.EImportFlags.Soft and not InternalEnums.EImportFlags.Embedded)
                imports_hash_set.insert(import.depot_path.to_owned());
            }

            lastimportidx = imports_hash_set.len();
            lastoffsetidx = archive.index.file_segments.len();
            flags = if !info.buffers_table.is_empty() {
                info.buffers_table.len() - 1
            } else {
                0
            };
        } else {
            // write non-cr2w file
            file_cursor.seek(SeekFrom::Start(0))?;
            if let Some(os_ext) = path.extension() {
                let ext = os_ext.to_ascii_lowercase().to_string_lossy().to_string();
                if get_aligned_file_extensions().contains(&ext) {
                    pad_until_page(&mut archive_writer)?;
                }

                let offset = archive_writer.stream_position()?;
                let size = file_buffer.len() as u32;
                let mut final_zsize = file_buffer.len() as u32;
                if get_uncompressed_file_extensions().contains(&ext) {
                    // direct copy
                    archive_writer.write_all(&file_buffer)?;
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
                archive.index.file_segments.push(FileSegment {
                    offset,
                    size,
                    z_size: final_zsize,
                });
            }
        }

        // update archive metadata
        let sha1_hash = sha1_hash_file(&file_buffer);

        let entry = FileEntry {
            name_hash_64: hash,
            timestamp: 0, // TODO proper timestamps
            num_inline_buffer_segments: flags as u32,
            segments_start: firstoffsetidx as u32,
            segments_end: lastoffsetidx as u32,
            resource_dependencies_start: firstimportidx as u32,
            resource_dependencies_end: lastimportidx as u32,
            sha1_hash,
        };
        archive.index.file_entries.insert(hash, entry);
    }

    // write footers
    // padding
    pad_until_page(&mut archive_writer)?;

    // write tables
    let tableoffset = archive_writer.stream_position()?;
    archive.index.serialize(&mut archive_writer)?;
    let tablesize = archive_writer.stream_position()? - tableoffset;

    // padding
    pad_until_page(&mut archive_writer)?;
    let filesize = archive_writer.stream_position()?;

    // write the header again
    archive.header.index_position = tableoffset;
    archive.header.index_size = tablesize as u32;
    archive.header.filesize = filesize;
    archive_writer.seek(SeekFrom::Start(0))?;
    archive.header.serialize(&mut archive_writer)?;
    archive_writer.write_u32::<LittleEndian>(custom_data_length as u32)?;

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
