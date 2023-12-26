use std::io::{Read, Result, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::io::FromReader;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Header {
    magic: u32,
    version: u32,
    index_position: u64,
    index_size: u32,
    debug_position: u64,
    debug_size: u32,
    filesize: u64,
}

impl Header {
    pub(crate) fn new(
        index_position: u64,
        index_size: u32,
        debug_position: u64,
        debug_size: u32,
        filesize: u64,
    ) -> Self {
        Self {
            magic: Header::HEADER_MAGIC,
            version: Header::HEADER_VERSION,
            index_position,
            index_size,
            debug_position,
            debug_size,
            filesize,
        }
    }

    pub(crate) const HEADER_MAGIC: u32 = 1380009042;
    pub(crate) const HEADER_VERSION: u32 = 12;
    pub(crate) const HEADER_SIZE: usize = 40;
    pub(crate) const HEADER_EXTENDED_SIZE: u64 = 0xAC;

    pub(crate) fn index_position(&self) -> u64 {
        self.index_position
    }
}

// impl Default for Header {
//     fn default() -> Self {
//         Self {
//             magic: 1380009042,
//             version: 12,
//             index_position: Default::default(),
//             index_size: Default::default(),
//             debug_position: Default::default(),
//             debug_size: Default::default(),
//             filesize: Default::default(),
//         }
//     }
// }

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
    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
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
