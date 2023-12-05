pub const MAGIC: u32 = 0x4B52414B;

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

pub enum CompressionLevel {
    None = 0,
    SuperFast = 1,
    VeryFast = 2,
    Fast = 3,
    Normal = 4,
    Optimal1 = 5,
    Optimal2 = 6,
    Optimal3 = 7,
    Optimal4 = 8,
    Optimal5 = 9,
}

/// Decompresses a compressed buffer into another
pub fn decompress(compressed_buffer: Vec<u8>, output_buffer: &mut Vec<u8>) -> i32 {
    unsafe {
        Kraken_Decompress(
            compressed_buffer.as_ptr(),
            compressed_buffer.len() as i64,
            output_buffer.as_mut_ptr(),
            output_buffer.len() as i64,
        )
    }
}

/// Compresses a buffer into another
pub fn compress(
    uncompressed_buffer: &Vec<u8>,
    compressed_buffer: &mut Vec<u8>,
    compression_level: CompressionLevel,
) -> i32 {
    if uncompressed_buffer.len() < 256 {
        *compressed_buffer = uncompressed_buffer.clone();
        compressed_buffer.len() as i32
    } else {
        unsafe {
            Kraken_Compress(
                uncompressed_buffer.as_ptr(),
                uncompressed_buffer.len() as i64,
                compressed_buffer.as_mut_ptr(),
                compression_level as i32,
            )
        }
    }
}

pub fn get_compressed_buffer_size_needed(count: u64) -> i32 {
    let n = (((count + 0x3ffff + (((count + 0x3ffff) >> 0x3f) & 0x3ffff)) >> 0x12) * 0x112) + count;
    n as i32
}

pub fn get_compressed_buffer_size_needed_kraken(size: i32) -> i32 {
    size + 274 * ((size + 0x3FFFF) / 0x40000)
}

/////////////////////////////////////////////////////////////////////////////////////////
/// TESTS
/////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::kraken::{
        get_compressed_buffer_size_needed, get_compressed_buffer_size_needed_kraken,
    };

    #[test]
    fn compressed_buffer_size() {
        let sizes = vec![
            10, 100, 1000, 10000, 100000, 1000000, 20, 200, 2000, 20000, 200000, 2000000,
        ];
        for s in sizes {
            let a = get_compressed_buffer_size_needed(s);
            let b = get_compressed_buffer_size_needed_kraken(s as i32);

            assert_eq!(a, b);
        }
    }
}
