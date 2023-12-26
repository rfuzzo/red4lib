use std::io::{Read, Result, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::io::FromReader;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FileSegment {
    offset: u64,
    z_size: u32,
    size: u32,
}

impl FileSegment {
    pub(crate) fn new(offset: u64, z_size: u32, size: u32) -> Self {
        Self {
            offset,
            z_size,
            size,
        }
    }

    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u64::<LittleEndian>(self.offset)?;
        writer.write_u32::<LittleEndian>(self.z_size)?;
        writer.write_u32::<LittleEndian>(self.size)?;
        Ok(())
    }

    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn z_size(&self) -> u32 {
        self.z_size
    }

    pub(crate) fn size(&self) -> u32 {
        self.size
    }
}

impl FromReader for FileSegment {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(FileSegment {
            offset: reader.read_u64::<LittleEndian>()?,
            z_size: reader.read_u32::<LittleEndian>()?,
            size: reader.read_u32::<LittleEndian>()?,
        })
    }
}
