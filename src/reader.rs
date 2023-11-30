/////////////////////////////////////////////////////////////////////////////////////////
/// READERS
/////////////////////////////////////////////////////////////////////////////////////////
use std::io::{self, Read};

// TODO make this into a macro
pub trait FromReader: Sized {
    fn from_reader<R: Read>(cursor: &mut R) -> io::Result<Self>;
}

/// Read a null_terminated_string from cursor
///
/// # Errors
///
/// This function will return an error if from_utf8_lossy fails
pub fn read_null_terminated_string<R: Read>(reader: &mut R) -> io::Result<String>
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
