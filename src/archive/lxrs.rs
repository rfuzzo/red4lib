use std::{
    cmp::Ordering,
    io::{Cursor, Error, ErrorKind, Read, Result, Write},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{io::*, kraken::*};

#[derive(Debug, Clone)]
pub(crate) struct LxrsFooter {
    files: Vec<String>,
}

impl LxrsFooter {
    pub(crate) fn new(files: Vec<String>) -> Self {
        Self { files }
    }

    //const MINLEN: u32 = 20;
    const MAGIC: u32 = 0x4C585253;
    const VERSION: u32 = 1;

    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
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

    pub(crate) fn files(&self) -> &[String] {
        self.files.as_ref()
    }
}
impl FromReader for LxrsFooter {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic != LxrsFooter::MAGIC {
            return Err(Error::new(ErrorKind::Other, "invalid magic"));
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
                let mut inner_cursor = Cursor::new(&output_buffer);
                for _i in 0..count {
                    // read NullTerminatedString
                    if let Ok(string) = read_null_terminated_string(&mut inner_cursor) {
                        files.push(string);
                    }
                }
            }
            Ordering::Less => {
                // error
                return Err(Error::new(ErrorKind::Other, "invalid buffer"));
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
