/////////////////////////////////////////////////////////////////////////////////////////
/// ARCHIVE
/////////////////////////////////////////////////////////////////////////////////////////
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Result, Write};
use std::mem;
use std::path::{Path, PathBuf};

use crate::fnv1a64_hash_string;
use crate::io::*;
use crate::kraken::*;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

#[derive(Debug, Clone, Default)]
pub(crate) struct Archive {
    pub(crate) header: Header,
    pub(crate) index: Index,

    // custom
    pub(crate) file_names: HashMap<u64, String>,
}

impl Archive {
    // Function to read a Header from a file
    pub fn from_file<P>(file_path: &P) -> Result<Archive>
    where
        P: AsRef<Path>,
    {
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
                cursor.set_position(Header::HEADER_EXTENDED_SIZE);
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
    pub(crate) fn get_file_hashes(&self) -> Vec<u64> {
        self.index
            .file_entries
            .iter()
            .map(|f| f.1.name_hash_64)
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Header {
    pub magic: u32,
    pub version: u32,
    pub index_position: u64,
    pub index_size: u32,
    pub debug_position: u64,
    pub debug_size: u32,
    pub filesize: u64,
}

impl Header {
    //static HEADER_MAGIC: u32 = 1380009042;
    //static HEADER_SIZE: i32 = 40;
    pub const HEADER_EXTENDED_SIZE: u64 = 0xAC;
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

/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod integration_tests {
    use std::{
        fs,
        io::{self, Read},
        path::PathBuf,
    };

    use super::LxrsFooter;
    use super::{Archive, FromReader};

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
        let result = Archive::from_file(&archive_path);
        assert!(result.is_ok());
    }

    #[test]
    fn read_archive2() {
        let archive_path = PathBuf::from("tests").join("nci.archive");
        let result = Archive::from_file(&archive_path);
        assert!(result.is_ok());
    }

    #[test]
    fn read_custom_data() {
        let archive_path = PathBuf::from("tests").join("test1.archive");
        let archive = Archive::from_file(&archive_path).expect("Could not parse archive");
        let mut file_names = archive
            .file_names
            .values()
            .map(|f| f.to_owned())
            .collect::<Vec<_>>();
        file_names.sort();

        let expected: Vec<String> = vec!["base\\cycleweapons\\localization\\en-us.json".to_owned()];
        assert_eq!(expected, file_names);
    }
}
