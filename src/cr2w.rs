use std::collections::HashMap;
use std::io::{self, Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::reader::{read_null_terminated_string, FromReader};

// DTOs

#[derive(Debug, Clone)]
pub struct Import {
    pub class_name: String,
    pub depot_path: String,
    pub flags: u16,
}

#[derive(Debug, Clone)]
pub struct CR2WFileInfo {
    pub header: CR2WFileHeader,
    pub names_table: Vec<CR2WNameInfo>,
    pub imports_table: Vec<CR2WImportInfo>,
    pub properties_table: Vec<CR2WPropertyInfo>,
    pub exports_table: Vec<CR2WExportInfo>,
    pub buffers_table: Vec<CR2WBufferInfo>,
    pub embeds_table: Vec<CR2WEmbeddedInfo>,
    // not-serialized
    pub strings: HashMap<u32, String>,
    pub names: Vec<String>,
    pub imports: Vec<Import>,
}

// Real red4 data

#[derive(Debug, Clone, Copy)]
pub struct CR2WFileHeader {
    pub version: u32,
    pub flags: u32,
    pub time_stamp: u64,
    pub build_version: u32,
    pub objects_end: u32,
    pub buffers_end: u32,
    pub crc32: u32,
    pub num_chunks: u32,
}
impl CR2WFileHeader {
    const MAGIC: u32 = 0x57325243;
}
impl FromReader for CR2WFileHeader {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WFileHeader {
            version: cursor.read_u32::<LittleEndian>()?,
            flags: cursor.read_u32::<LittleEndian>()?,
            time_stamp: cursor.read_u64::<LittleEndian>()?,
            build_version: cursor.read_u32::<LittleEndian>()?,
            objects_end: cursor.read_u32::<LittleEndian>()?,
            buffers_end: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
            num_chunks: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct CR2WTable {
    pub offset: u32,
    pub item_count: u32,
    pub crc32: u32,
}

impl FromReader for CR2WTable {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WTable {
            offset: cursor.read_u32::<LittleEndian>()?,
            item_count: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WNameInfo {
    pub offset: u32,
    pub hash: u32,
}
impl FromReader for CR2WNameInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WNameInfo {
            offset: cursor.read_u32::<LittleEndian>()?,
            hash: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WImportInfo {
    pub offset: u32,
    pub class_name: u16,
    pub flags: u16,
}
impl FromReader for CR2WImportInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WImportInfo {
            offset: cursor.read_u32::<LittleEndian>()?,
            class_name: cursor.read_u16::<LittleEndian>()?,
            flags: cursor.read_u16::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WPropertyInfo {
    pub class_name: u16,
    pub class_flags: u16,
    pub property_name: u16,
    pub property_flags: u16,
    pub hash: u64,
}
impl FromReader for CR2WPropertyInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WPropertyInfo {
            class_name: cursor.read_u16::<LittleEndian>()?,
            class_flags: cursor.read_u16::<LittleEndian>()?,
            property_name: cursor.read_u16::<LittleEndian>()?,
            property_flags: cursor.read_u16::<LittleEndian>()?,
            hash: cursor.read_u64::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WExportInfo {
    pub class_name: u16,
    pub object_flags: u16,
    pub parent_id: u32,
    pub data_size: u32,
    pub data_offset: u32,
    pub template: u32,
    pub crc32: u32,
}
impl FromReader for CR2WExportInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WExportInfo {
            class_name: cursor.read_u16::<LittleEndian>()?,
            object_flags: cursor.read_u16::<LittleEndian>()?,
            parent_id: cursor.read_u32::<LittleEndian>()?,
            data_size: cursor.read_u32::<LittleEndian>()?,
            data_offset: cursor.read_u32::<LittleEndian>()?,
            template: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WBufferInfo {
    pub flags: u32,
    pub index: u32,
    pub offset: u32,
    pub disk_size: u32,
    pub mem_size: u32,
    pub crc32: u32,
}
impl FromReader for CR2WBufferInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WBufferInfo {
            flags: cursor.read_u32::<LittleEndian>()?,
            index: cursor.read_u32::<LittleEndian>()?,
            offset: cursor.read_u32::<LittleEndian>()?,
            disk_size: cursor.read_u32::<LittleEndian>()?,
            mem_size: cursor.read_u32::<LittleEndian>()?,
            crc32: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CR2WEmbeddedInfo {
    pub import_index: u32,
    pub chunk_index: u32,
    pub path_hash: u64,
}
impl FromReader for CR2WEmbeddedInfo {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self> {
        Ok(CR2WEmbeddedInfo {
            import_index: cursor.read_u32::<LittleEndian>()?,
            chunk_index: cursor.read_u32::<LittleEndian>()?,
            path_hash: cursor.read_u64::<LittleEndian>()?,
        })
    }
}

/////////////////////////////////////////////////////////////////////////////////////////
/// HELPERS
/////////////////////////////////////////////////////////////////////////////////////////

/// Reads the string table of a CR2W file
///
/// # Panics
///
/// Panics if the reading fails
fn read_strings<R: Read + Seek>(reader: &mut R, table: CR2WTable) -> HashMap<u32, String> {
    let mut stringtable: HashMap<u32, String> = HashMap::default();

    let mut offset = reader
        .stream_position()
        .expect("Failed to get offset from reader") as u32;

    while offset < (table.offset + table.item_count) {
        offset = reader
            .stream_position()
            .expect("Failed to get offset from reader") as u32;
        let mut str = read_null_terminated_string(reader)
            .expect("Failed to read a string from the string table");
        if str.is_empty() {
            str = "None".to_owned();
        }
        let position_in_chunk = offset - table.offset;
        stringtable.insert(position_in_chunk, str);
    }

    stringtable
}

fn read_table<R: Read + Seek, T: FromReader>(
    reader: &mut R,
    table: CR2WTable,
) -> io::Result<Vec<T>> {
    let mut result_table: Vec<T> = vec![];
    for _i in 0..table.item_count {
        result_table.push(T::from_reader(reader)?);
    }
    Ok(result_table)
}

/// Reads the header info from a cr2w file
///
/// # Errors
///
/// This function will return an error if any parsing failed downstream.
pub fn read_cr2w_header<R: Read + Seek>(cursor: &mut R) -> io::Result<CR2WFileInfo> {
    let magic = cursor.read_u32::<LittleEndian>()?;
    if magic != CR2WFileHeader::MAGIC {
        return Err(io::Error::new(io::ErrorKind::Other, "invalid magic"));
    }

    let header = CR2WFileHeader::from_reader(cursor)?;
    let mut tables: Vec<CR2WTable> = vec![];
    // Tables [7-9] are not used in cr2w so far.
    for _i in 0..10 {
        tables.push(CR2WTable::from_reader(cursor)?);
    }

    // read strings - block 1 (index 0)
    let strings = read_strings(cursor, tables[0]);

    // read the other tables
    let names_table = read_table::<R, CR2WNameInfo>(cursor, tables[1])?;
    let imports_table = read_table::<R, CR2WImportInfo>(cursor, tables[2])?;
    let properties_table = read_table::<R, CR2WPropertyInfo>(cursor, tables[3])?;
    let exports_table = read_table::<R, CR2WExportInfo>(cursor, tables[3])?;
    let buffers_table = read_table::<R, CR2WBufferInfo>(cursor, tables[3])?;
    let embeds_table = read_table::<R, CR2WEmbeddedInfo>(cursor, tables[3])?;

    // hacks: parse specific
    // parse names
    let names = names_table
        .iter()
        .map(|f| strings.get(&f.offset).unwrap().to_owned())
        .collect::<Vec<_>>();

    // parse imports
    let mut imports: Vec<Import> = vec![];
    for info in imports_table.iter() {
        let class_name = names.get(info.class_name as usize).unwrap().to_owned();
        let depot_path = strings.get(&info.offset).unwrap().to_owned();
        let flags = info.flags;
        imports.push(Import {
            class_name,
            depot_path,
            flags,
        });
    }

    let info = CR2WFileInfo {
        header,
        names_table,
        imports_table,
        properties_table,
        exports_table,
        buffers_table,
        embeds_table,
        strings,
        imports,
        names,
    };
    Ok(info)
}
