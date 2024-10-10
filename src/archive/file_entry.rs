use std::io::{Read, Result, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::io::FromReader;

#[derive(Debug, Clone, Copy)]
pub struct FileEntry {
    name_hash_64: u64,
    timestamp: u64, //SystemTime,
    num_inline_buffer_segments: u32,
    segments_start: u32,
    segments_end: u32,
    resource_dependencies_start: u32,
    resource_dependencies_end: u32,
    sha1_hash: [u8; 20],
}

impl FileEntry {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        name_hash_64: u64,
        timestamp: u64,
        num_inline_buffer_segments: u32,
        segments_start: u32,
        segments_end: u32,
        resource_dependencies_start: u32,
        resource_dependencies_end: u32,
        sha1_hash: [u8; 20],
    ) -> Self {
        Self {
            name_hash_64,
            timestamp,
            num_inline_buffer_segments,
            segments_start,
            segments_end,
            resource_dependencies_start,
            resource_dependencies_end,
            sha1_hash,
        }
    }

    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u64::<LittleEndian>(self.name_hash_64)?;
        writer.write_u64::<LittleEndian>(self.timestamp)?;
        writer.write_u32::<LittleEndian>(self.num_inline_buffer_segments)?;
        writer.write_u32::<LittleEndian>(self.segments_start)?;
        writer.write_u32::<LittleEndian>(self.segments_end)?;
        writer.write_u32::<LittleEndian>(self.resource_dependencies_start)?;
        writer.write_u32::<LittleEndian>(self.resource_dependencies_end)?;
        writer.write_all(self.sha1_hash.as_slice())?;
        Ok(())
    }

    pub(crate) fn name_hash_64(&self) -> u64 {
        self.name_hash_64
    }

    pub fn sha1_hash(&self) -> [u8; 20] {
        self.sha1_hash
    }

    pub(crate) fn segments_start(&self) -> u32 {
        self.segments_start
    }

    pub(crate) fn segments_end(&self) -> u32 {
        self.segments_end
    }

    pub(crate) fn set_segments_start(&mut self, segments_start: u32) {
        self.segments_start = segments_start;
    }

    pub(crate) fn set_segments_end(&mut self, segments_end: u32) {
        self.segments_end = segments_end;
    }
}

impl FromReader for FileEntry {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
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
