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

enum CompressionLevel {
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
    uncompressed_buffer: Vec<u8>,
    compressed_buffer: &mut Vec<u8>,
    with_header: bool,
) -> usize {
    !todo!("KARK Header");

    if uncompressed_buffer.len() < 256 {
        *compressed_buffer = uncompressed_buffer.clone();
        compressed_buffer.len()
    } else {
        let result = unsafe {
            Kraken_Compress(
                uncompressed_buffer.as_ptr(),
                uncompressed_buffer.len() as i64,
                compressed_buffer.as_mut_ptr(),
                CompressionLevel::Normal as i32,
            )
        };

        result as usize
    }
}