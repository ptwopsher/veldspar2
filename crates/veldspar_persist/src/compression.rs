use std::io;

pub fn compress_zstd(data: &[u8], level: i32) -> io::Result<Vec<u8>> {
    zstd::stream::encode_all(data, level)
}

pub fn decompress_zstd(data: &[u8]) -> io::Result<Vec<u8>> {
    zstd::stream::decode_all(data)
}

pub fn compress_lz4(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

pub fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, lz4_flex::block::DecompressError> {
    lz4_flex::decompress_size_prepended(data)
}
