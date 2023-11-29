#![warn(clippy::all, rust_2018_idioms)]

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::{self, File};
use std::hash::Hasher;
use std::io::{self, BufWriter, Read, Result, Write};
use std::mem;
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use sha1::{Digest, Sha1};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use walkdir::WalkDir;

#[link(name = "kraken_static")]
extern "C" {
    // EXPORT int Kraken_Decompress(const byte *src, size_t src_len, byte *dst, size_t dst_len)
    fn Kraken_Decompress(
        buffer: *const u8,
        bufferSize: i64,
        outputBuffer: *mut u8,
        outputBufferSize: i64,
    ) -> i32;

    // EXPORT int Kraken_Compress(uint8* src, size_t src_len, byte* dst, int level)
    fn Kraken_Compress(
        buffer: *const u8,
        bufferSize: i64,
        outputBuffer: *mut u8,
        level: i32,
    ) -> i32;
}

/////////////////////////////////////////////////////////////////////////////////////////
/// RED4 LIB
/////////////////////////////////////////////////////////////////////////////////////////

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

    // pack and write an archive from folder

    // Assuming you have a struct Archive and other necessary structs and enums

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
        // TODO
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
        let mut bw = BufWriter::new(&mut fs);

        // write temp header
        let archive = Archive::default();
        let header = Header::default();
        header.serialize(&mut bw)?;
        bw.write_all(&[0u8; 132]).unwrap(); // some weird padding

        // write custom data
        // TODO
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
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;

            let firstimportidx = imports_hash_set.len();
            let lastimportidx = imports_hash_set.len();
            let firstoffsetidx = archive.index.file_segments.len();
            let lastoffsetidx = 0;
            let flags = 0;
        }

        Ok(())
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

impl Header {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        let header = Header {
            magic: cursor.read_u32::<LittleEndian>()?,
            version: cursor.read_u32::<LittleEndian>()?,
            index_position: cursor.read_u64::<LittleEndian>()?,
            index_size: cursor.read_u32::<LittleEndian>()?,
            debug_position: cursor.read_u64::<LittleEndian>()?,
            debug_size: cursor.read_u32::<LittleEndian>()?,
            filesize: cursor.read_u64::<LittleEndian>()?,
        };

        Ok(header)
    }

    fn serialize<W: Write>(&self, cursor: &mut W) -> io::Result<()> {
        cursor.write_u32::<LittleEndian>(self.magic)?;
        cursor.write_u32::<LittleEndian>(self.version)?;
        cursor.write_u64::<LittleEndian>(self.index_position)?;
        cursor.write_u32::<LittleEndian>(self.index_size)?;
        cursor.write_u64::<LittleEndian>(self.debug_position)?;
        cursor.write_u32::<LittleEndian>(self.debug_size)?;
        cursor.write_u64::<LittleEndian>(self.filesize)?;

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

impl FileEntry {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        let mut entry = FileEntry {
            name_hash_64: cursor.read_u64::<LittleEndian>()?,
            timestamp: cursor.read_u64::<LittleEndian>()?,
            num_inline_buffer_segments: cursor.read_u32::<LittleEndian>()?,
            segments_start: cursor.read_u32::<LittleEndian>()?,
            segments_end: cursor.read_u32::<LittleEndian>()?,
            resource_dependencies_start: cursor.read_u32::<LittleEndian>()?,
            resource_dependencies_end: cursor.read_u32::<LittleEndian>()?,
            sha1_hash: [0; 20],
        };

        cursor.read_exact(&mut entry.sha1_hash[..])?;

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

    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        let magic = cursor.read_u32::<LittleEndian>()?;
        if magic != LxrsFooter::MAGIC {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid magic"));
        }
        let _version = cursor.read_u32::<LittleEndian>()?;
        let size = cursor.read_u32::<LittleEndian>()?;
        let zsize = cursor.read_u32::<LittleEndian>()?;
        let count = cursor.read_i32::<LittleEndian>()?;

        let mut files: Vec<String> = vec![];
        match size.cmp(&zsize) {
            Ordering::Greater => {
                // buffer is compressed
                let buffer_len = zsize as usize;
                let mut compressed_buffer = vec![0; buffer_len];
                cursor.read_exact(&mut compressed_buffer[..])?;

                let output_buffer_len = size as usize * 2;
                let mut output_buffer = vec![0; output_buffer_len];

                let _result = unsafe {
                    Kraken_Decompress(
                        compressed_buffer.as_ptr(),
                        compressed_buffer.len() as i64,
                        output_buffer.as_mut_ptr(),
                        output_buffer.len() as i64,
                    )
                };

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
                    if let Ok(string) = read_null_terminated_string(cursor) {
                        files.push(string);
                    }
                }
            }
        }

        let footer = LxrsFooter { files };

        Ok(footer)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, EnumIter, Display)]
enum ERedExtension {
    unknown,

    acousticdata,
    actionanimdb,
    aiarch,
    animgraph,
    anims,
    app,
    archetypes,
    areas,
    audio_metadata,
    audiovehcurveset,
    behavior,
    bikecurveset,
    bk2,
    bnk,
    camcurveset,
    ccstate,
    cfoliage,
    charcustpreset,
    chromaset,
    cminimap,
    community,
    conversations,
    cooked_mlsetup,
    cookedanims,
    cookedapp,
    cookedprefab,
    credits,
    csv,
    cubemap,
    curveresset,
    curveset,
    dat,
    devices,
    dlc_manifest,
    dtex,
    effect,
    ent,
    env,
    envparam,
    envprobe,
    es,
    facialcustom,
    facialsetup,
    fb2tl,
    fnt,
    folbrush,
    foldest,
    fp,
    game,
    gamedef,
    garmentlayerparams,
    genericanimdb,
    geometry_cache,
    gidata,
    gradient,
    hitrepresentation,
    hp,
    ies,
    inkanim,
    inkatlas,
    inkcharcustomization,
    inkenginesettings,
    inkfontfamily,
    inkfullscreencomposition,
    inkgamesettings,
    inkhud,
    inklayers,
    inkmenu,
    inkshapecollection,
    inkstyle,
    inktypography,
    inkwidget,
    interaction,
    journal,
    journaldesc,
    json,
    lane_connections,
    lane_polygons,
    lane_spots,
    lights,
    lipmap,
    location,
    locopaths,
    loot,
    mappins,
    matlib,
    mesh,
    mi,
    mlmask,
    mlsetup,
    mltemplate,
    morphtarget,
    mt,
    null_areas,
    opusinfo,
    opuspak,
    particle,
    phys,
    physicalscene,
    physmatlib,
    poimappins,
    psrep,
    quest,
    questphase,
    redphysics,
    regionset,
    remt,
    reps,
    reslist,
    rig,
    scene,
    scenerid,
    scenesversions,
    smartobject,
    smartobjects,
    sp,
    spatial_representation,
    streamingblock,
    streamingquerydata,
    streamingsector,
    streamingsector_inplace,
    streamingworld,
    terrainsetup,
    texarray,
    traffic_collisions,
    traffic_persistent,
    vehcommoncurveset,
    vehcurveset,
    voicetags,
    w2mesh,
    w2mi,
    wem,
    workspot,
    worldlist,
    xbm,
    xcube,

    wdyn,
}
#[warn(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct CR2WFileInfo {
    pub header: CR2WFileHeader,
}

#[derive(Debug, Clone, Copy)]
pub struct CR2WFileHeader {
    pub version: u32,
    pub flags: u32,
    pub timeStamp: u64,
    pub buildVersion: u32,
    pub objectsEnd: u32,
    pub buffersEnd: u32,
    pub crc32: u32,
    pub numChunks: u32,
}
impl CR2WFileHeader {
    const MAGIC: u32 = 0x57325243;

    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WFileHeader {
            version: cursor.read_u32::<LittleEndian>()?,
            flags: cursor.read_u32::<LittleEndian>()?,
            timeStamp: cursor.read_u64::<LittleEndian>()?,
            buildVersion: cursor.read_u32::<LittleEndian>()?,
            objectsEnd: cursor.read_u32::<LittleEndian>()?,
            buffersEnd: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
            numChunks: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct CR2WTable {
    pub offset: u32,
    pub itemCount: u32,
    pub crc32: u32,
}

impl CR2WTable {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WTable {
            offset: cursor.read_u32::<LittleEndian>()?,
            itemCount: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

/////////////////////////////////////////////////////////////////////////////////////////
/// READERS
/////////////////////////////////////////////////////////////////////////////////////////

fn read_cr2w_header<R: Read>(cursor: &mut R) -> io::Result<CR2WFileInfo> {
    let magic = cursor.read_u32::<LittleEndian>()?;
    if magic != CR2WFileHeader::MAGIC {
        return Err(io::Error::new(io::ErrorKind::Other, "invalid magic"));
    }

    let header = CR2WFileHeader::from_reader(cursor)?;
    let mut headers: Vec<CR2WTable> = vec![];
    // Tables [7-9] are not used in cr2w so far.
    for i in 0..10 {
        headers[i] = CR2WTable::from_reader(cursor)?;
    }

    // read strings - block 1 (index 0)

    // read the other tables

    let info = CR2WFileInfo { header };
    Ok(info)
}

/// Read a null_terminated_string from cursor
///
/// # Errors
///
/// This function will return an error if from_utf8_lossy fails
fn read_null_terminated_string<R: Read>(reader: &mut R) -> io::Result<String>
where
    R: Read,
{
    let mut buffer = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        reader.read_exact(&mut byte)?;

        if byte[0] == 0 {
            break;
        }

        buffer.push(byte[0]);
    }

    Ok(String::from_utf8_lossy(&buffer).to_string())
}

/////////////////////////////////////////////////////////////////////////////////////////
/// HELPERS
/////////////////////////////////////////////////////////////////////////////////////////

/// Get top-level files of a folder with given extension
pub fn get_files(folder_path: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !folder_path.exists() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(ext) = entry.path().extension() {
                        if ext == extension {
                            files.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    files
}

/// Calculate FNV1a64 hash of a String
pub fn fnv1a64_hash_string(str: &String) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    hasher.write(str.as_bytes());
    hasher.finish()
}

/// Calculate FNV1a64 hash of a PathBuf
pub fn fnv1a64_hash_path(path: &Path) -> u64 {
    let path_string = path.to_string_lossy();
    let mut hasher = fnv::FnvHasher::default();
    hasher.write(path_string.as_bytes());
    hasher.finish()
}

/// Get vanilla resource path hashes https://www.cyberpunk.net/en/modding-support
pub fn get_red4_hashes() -> HashMap<u64, String> {
    let csv_data = include_bytes!("metadata-resources.csv");
    parse_csv_data(csv_data)
}

/// Reads the metadata-resources.csv (csv of hashes and strings) from https://www.cyberpunk.net/en/modding-support
fn parse_csv_data(csv_data: &[u8]) -> HashMap<u64, String> {
    let mut reader = csv::ReaderBuilder::new().from_reader(csv_data);
    let mut csv_map: HashMap<u64, String> = HashMap::new();

    for result in reader.records() {
        match result {
            Ok(record) => {
                // Assuming the CSV has two columns: String and u64
                if let (Some(path), Some(hash_str)) = (record.get(0), record.get(1)) {
                    if let Ok(hash) = hash_str.parse::<u64>() {
                        csv_map.insert(hash, path.to_string());
                    } else {
                        eprintln!("Error parsing u64 value: {}", hash_str);
                    }
                } else {
                    eprintln!("Malformed CSV record: {:?}", record);
                }
            }
            Err(err) => eprintln!("Error reading CSV record: {}", err),
        }
    }

    csv_map
}

/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    #[test]
    fn load_order() {
        let mut input = [
            "#.archive",
            "_.archive",
            "aa.archive",
            "zz.archive",
            "AA.archive",
            "ZZ.archive",
        ];
        let correct = [
            "#.archive",
            "AA.archive",
            "ZZ.archive",
            "_.archive",
            "aa.archive",
            "zz.archive",
        ];

        input.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        //input.sort();
        assert_eq!(correct, input);
    }
}
