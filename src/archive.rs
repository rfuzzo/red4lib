/////////////////////////////////////////////////////////////////////////////////////////
/// ARCHIVE
/////////////////////////////////////////////////////////////////////////////////////////
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Result, Seek, SeekFrom, Write};
use std::mem;
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use sha1::{Digest, Sha1};
use strum::IntoEnumIterator;
use walkdir::WalkDir;

use crate::cr2w::read_cr2w_header;
use crate::kraken::{compress, decompress};
use crate::reader::{read_null_terminated_string, FromReader};
use crate::{fnv1a64_hash_string, ERedExtension};

#[derive(Debug, Clone, Default)]
pub struct Archive {
    pub header: Header,
    pub index: Index,

    // custom
    pub file_names: Vec<String>,
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
        let mut file_names: Vec<String> = vec![];
        if let Ok(custom_data_length) = cursor.read_u32::<LittleEndian>() {
            if custom_data_length > 0 {
                cursor.set_position(HEADER_EXTENDED_SIZE);
                if let Ok(footer) = LxrsFooter::from_reader(&mut cursor) {
                    // add files to hashmap
                    for f in footer.files {
                        file_names.push(f);
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
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let header = Header {
            magic: reader.read_u32::<LittleEndian>()?,
            version: reader.read_u32::<LittleEndian>()?,
            index_position: reader.read_u64::<LittleEndian>()?,
            index_size: reader.read_u32::<LittleEndian>()?,
            debug_position: reader.read_u64::<LittleEndian>()?,
            debug_size: reader.read_u32::<LittleEndian>()?,
            filesize: reader.read_u64::<LittleEndian>()?,
        };

        Ok(header)
    }
}
impl Header {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
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
    // pub dependencies: Vec<Dependency>,
    pub file_entries: HashMap<u64, FileEntry>,
    pub file_segments: Vec<FileSegment>,
}
impl Index {
    pub fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
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
        };

        // read files
        for _i in 0..index.file_entry_count {
            let entry = FileEntry::from_reader(cursor)?;
            index.file_entries.insert(entry.name_hash_64, entry);
        }

        // ignore the rest of the archive

        Ok(index)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileSegment {
    pub offset: u64,
    pub size: u32,
    pub z_size: u32,
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

#[derive(Debug, Clone)]
pub struct LxrsFooter {
    pub files: Vec<String>,
}

impl LxrsFooter {
    //const MINLEN: u32 = 20;
    const MAGIC: u32 = 0x4C585253;
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
                let buffer_len = zsize as usize;
                let mut compressed_buffer = vec![0; buffer_len];
                reader.read_exact(&mut compressed_buffer[..])?;

                let output_buffer_len = size as usize * 2;
                let mut output_buffer = vec![0; output_buffer_len];

                let _result = decompress(compressed_buffer, &mut output_buffer);

                // read bytes
                //if result as u32 == size {
                output_buffer.resize(size as usize, 0);
                let mut inner_cursor = io::Cursor::new(&output_buffer);
                for _i in 0..count {
                    // read NullTerminatedString
                    if let Ok(string) = read_null_terminated_string(&mut inner_cursor) {
                        files.push(string);
                    }
                }
                //}
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

pub fn write_archive(infolder: &Path, outpath: &Path, modname: Option<&str>) -> io::Result<()> {
    if !infolder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }

    if !outpath.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }

    let archive_name = if let Some(name) = modname {
        format!("{}.archive", name)
    } else {
        format!(
            "{}.archive",
            infolder.file_name().unwrap_or_default().to_string_lossy()
        )
    };

    // collect files
    let mut included_extensions = ERedExtension::iter()
        .map(|variant| variant.to_string())
        .collect::<Vec<_>>();
    included_extensions.push(String::from("bin"));

    // get only resource files
    let allfiles = WalkDir::new(infolder)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|p| {
            included_extensions
                .contains(&p.path().extension().unwrap().to_str().unwrap().to_owned())
        })
        .map(|f| f.into_path())
        .collect::<Vec<_>>();

    // sort by hash
    let mut resource_paths = allfiles
        .iter()
        .map(|f| {
            let relative_path = f.strip_prefix(infolder).unwrap_or(f);
            let hash = fnv1a64_hash_string(&relative_path.to_str().unwrap().to_owned());
            (f.clone(), hash)
        })
        .collect::<Vec<_>>();
    resource_paths.sort_by_key(|k| k.1);

    let outfile = outpath.join(archive_name);
    let mut fs = File::create(outfile).unwrap();
    let mut archive_writer = BufWriter::new(&mut fs);

    // write temp header
    let mut archive = Archive::default();
    let header = Header::default();
    header.serialize(&mut archive_writer)?;
    archive_writer.write_all(&[0u8; 132]).unwrap(); // some weird padding

    // TODO write custom data
    // let custom_data_length = if !custom_paths.is_empty() {
    //     let wfooter = LxrsFooter::new(&custom_paths);
    //     wfooter.write(&mut bw).unwrap();
    //     bw.seek(SeekFrom::Start(Header::EXTENDED_SIZE as u64))
    //         .unwrap();
    //     bw.stream_position().unwrap() as u32
    // } else {
    //     0
    // };

    // write files
    let mut imports_hash_set: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for (path, hash) in resource_paths {
        // TODO custom paths

        // read file
        let mut file = File::open(path)?;
        let mut file_buffer = Vec::new();
        file.read_to_end(&mut file_buffer)?;

        let firstimportidx = imports_hash_set.len();
        let mut lastimportidx = imports_hash_set.len();
        let firstoffsetidx = archive.index.file_segments.len();
        let mut lastoffsetidx = 0;
        let mut flags = 0;

        let mut file_cursor = io::Cursor::new(file_buffer.clone()); //TODO clone here?
        if let Ok(info) = read_cr2w_header(&mut file_cursor) {
            // get main file
            file_cursor.seek(SeekFrom::Start(0))?;
            let size = info.header.objects_end as usize;
            let mut resource_buffer = vec![0; size];
            file_cursor.read_exact(&mut resource_buffer[..])?;
            // get archive offset before writing
            let archive_offset = archive_writer.stream_position()?;

            // kark file
            let mut compressed_buffer = vec![0; size]; // TODO allocate?
            let zsize = compress(resource_buffer, &mut compressed_buffer, true);
            // write compressed main file archive
            archive_writer.write_all(&compressed_buffer)?;

            // add metadata to archive
            archive.index.file_segments.push(FileSegment {
                offset: archive_offset,
                size: size as u32,
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
            for import_info in info.imports_table.iter() {}

            lastimportidx = imports_hash_set.len();
            lastoffsetidx = archive.index.file_segments.len();
            flags = if info.buffers_table.len() > 0 {
                info.buffers_table.len() - 1
            } else {
                0
            };
        } else {
            // write non-cr2w file
            !todo!();
        }

        // update archive metadata
        let mut hasher = Sha1::new();
        hasher.update(file_buffer);
        let result = hasher.finalize();

        let entry = FileEntry {
            name_hash_64: hash,
            timestamp: 0, //TODO
            num_inline_buffer_segments: flags as u32,
            segments_start: firstoffsetidx as u32,
            segments_end: lastoffsetidx as u32,
            resource_dependencies_start: firstimportidx as u32,
            resource_dependencies_end: lastimportidx as u32,
            sha1_hash: result.into(),
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
    //archive_writer.write_u32::<LittleEndian>(custom_data_length);

    Ok(())
}

fn pad_until_page<W: Write + Seek>(writer: &mut W) -> io::Result<()> {
    let pos = writer.stream_position()?;
    let modulo = pos / 4096;
    let diff = ((modulo + 1) * 4096) - pos;
    let padding = vec![0xD9; diff as usize];
    writer.write_all(padding.as_slice())?;

    Ok(())
}
