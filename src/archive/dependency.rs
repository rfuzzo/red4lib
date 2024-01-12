use std::io::{Read, Result};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::io::FromReader;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Dependency {
    hash: u64,
}

// impl Dependency {
//     pub(crate) fn new(hash: u64) -> Self {
//         Self { hash }
//     }

//     pub(crate) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
//         writer.write_u64::<LittleEndian>(self.hash)?;
//         Ok(())
//     }
// }
#[warn(dead_code)]

impl FromReader for Dependency {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Dependency {
            hash: reader.read_u64::<LittleEndian>()?,
        })
    }
}
