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

use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, File},
    io::{self, BufWriter, Read, Result, Seek, SeekFrom, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use strum::IntoEnumIterator;
use walkdir::WalkDir;

use crate::{
    archive::*,
    cr2w::*,
    fnv1a64_hash_string, get_red4_hashes,
    kraken::{self, *},
    sha1_hash_file, ERedExtension,
};

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
) -> io::Result<()>
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
) -> io::Result<()>
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
) -> io::Result<()>
where
    P: AsRef<Path>,
    R: Read + Seek,
{
    let map = if let Some(hash_map) = hash_map {
        hash_map
    } else {
        get_red4_hashes()
    };

    extract_archive(source, destination_directory_name, overwrite_files, &map)
}

// public static void ExtractToDirectory (string sourceArchiveFileName, string destinationDirectoryName, bool overwriteFiles);

/// Extracts all of the files in the specified archive to a directory on the file system.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn extract_to_directory_path<P>(
    source_archive_file_name: &P,
    destination_directory_name: &P,
    overwrite_files: bool,
    hash_map: Option<HashMap<u64, String>>,
) -> io::Result<()>
where
    P: AsRef<Path>,
{
    let map = if let Some(hash_map) = hash_map {
        hash_map
    } else {
        get_red4_hashes()
    };

    let archive_file = File::open(source_archive_file_name)?;
    let mut archive_reader = io::BufReader::new(archive_file);

    extract_archive(
        &mut archive_reader,
        destination_directory_name,
        overwrite_files,
        &map,
    )
}

pub enum ArchiveMode {
    Create,
    Read,
    Update,
}

/*
TODO We don't support different modes for now
needs a wrapper class for archives


// public static System.IO.Compression.ZipArchive Open (string archiveFileName, System.IO.Compression.ZipArchiveMode mode);

/// Opens an archive at the specified path and in the specified mode.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn open<P>(archive_file_name: P, mode: ArchiveMode) -> io::Result<Archive>
where
    P: AsRef<Path>,
{
    todo!()
}

 */

// public static System.IO.Compression.ZipArchive OpenRead (string archiveFileName);

/// Opens an archive for reading at the specified path.
///
/// # Errors
///
/// This function will return an error if any io fails.
pub fn open_read<P>(archive_file_name: P) -> io::Result<Archive>
where
    P: AsRef<Path>,
{
    Archive::from_file(archive_file_name)
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
fn extract_archive<P, R>(
    archive_reader: &mut R,
    out_dir: &P,
    overwrite_files: bool,
    hash_map: &HashMap<u64, String>,
) -> io::Result<()>
where
    P: AsRef<Path>,
    R: Read + Seek,
{
    // parse archive headers
    let archive = Archive::from_reader(archive_reader)?;

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
        let outfile = out_dir.as_ref().join(name_or_hash);
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

        //let mut fs = File::create(outfile)?;
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
                decompress_segment(archive_reader, segment, &mut file_writer)?;
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
fn write_archive<P, W>(
    in_folder: &P,
    out_stream: W,
    hash_map: HashMap<u64, String>,
) -> io::Result<()>
where
    P: AsRef<Path>,
    W: Write + Seek,
{
    /*if !in_folder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }

    if !out_folder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }
    // check extension
    if !out_folder.exists() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
    }*/

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

    let mut archive_writer = BufWriter::new(out_stream);

    // write temp header
    let mut archive = Archive::default();
    let header = Header::default();
    header.serialize(&mut archive_writer)?;
    archive_writer.write_all(&[0u8; 132])?; // some weird padding

    // write custom header
    assert_eq!(
        Header::HEADER_EXTENDED_SIZE,
        archive_writer.stream_position()?
    );
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
        custom_data_length = archive_writer.stream_position()? - Header::HEADER_EXTENDED_SIZE;
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

fn pad_until_page<W: Write + Seek>(writer: &mut W) -> io::Result<()> {
    let pos = writer.stream_position()?;
    let modulo = pos / 4096;
    let diff = ((modulo + 1) * 4096) - pos;
    let padding = vec![0xD9; diff as usize];
    writer.write_all(padding.as_slice())?;

    Ok(())
}
