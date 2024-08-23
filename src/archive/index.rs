use std::io::{Read, Result};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::io::FromReader;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct Index {
    /// Offset from the beginning of this struct, should be 8
    file_table_offset: u32,
    /// byte size of the table
    file_table_size: u32,
    crc: u64,
    file_entry_count: u32,
    file_segment_count: u32,
    resource_dependency_count: u32,
}

#[warn(dead_code)]

impl Index {
    pub(crate) fn file_entry_count(&self) -> u32 {
        self.file_entry_count
    }

    pub(crate) fn file_segment_count(&self) -> u32 {
        self.file_segment_count
    }

    pub(crate) fn resource_dependency_count(&self) -> u32 {
        self.resource_dependency_count
    }
}

impl FromReader for Index {
    fn from_reader<R: Read>(cursor: &mut R) -> Result<Self> {
        let index = Index {
            file_table_offset: cursor.read_u32::<LittleEndian>()?,
            file_table_size: cursor.read_u32::<LittleEndian>()?,
            crc: cursor.read_u64::<LittleEndian>()?,
            file_entry_count: cursor.read_u32::<LittleEndian>()?,
            file_segment_count: cursor.read_u32::<LittleEndian>()?,
            resource_dependency_count: cursor.read_u32::<LittleEndian>()?,
        };

        Ok(index)
    }
}
