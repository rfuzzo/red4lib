/////////////////////////////////////////////////////////////////////////////////////////
/// READERS
/////////////////////////////////////////////////////////////////////////////////////////
use std::io::{self, Read, Write};

use byteorder::WriteBytesExt;

pub trait FromReader: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self>;
}

/// Read a null_terminated_string
///
/// # Errors
///
/// This function will return an error if from_utf8_lossy fails
pub fn read_null_terminated_string<R: Read>(reader: &mut R) -> io::Result<String> {
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

/// Writes a null_terminated_string
///
/// # Errors
///
/// This function will return an error if writing fails
pub fn write_null_terminated_string<W: Write>(writer: &mut W, str: String) -> io::Result<()> {
    writer.write_all(str.as_bytes())?;
    writer.write_u8(0x0)?;

    Ok(())
}
