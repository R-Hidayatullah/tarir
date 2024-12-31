#![allow(dead_code)]

const PF_MAGIC_NUMBER: usize = 2;
const CHUNK_HEADER_MAGIC_NUMBER: usize = 4;

#[derive(Debug, Default)]
struct PfHeader {
    identifier: [u8; PF_MAGIC_NUMBER],
    version: u16,
    zero: u16,
    header_size: u16,
    chunk_identifier: [u8; CHUNK_HEADER_MAGIC_NUMBER],
}

#[derive(Debug, Default)]
struct PfChunkHeader {
    identifier: [u8; CHUNK_HEADER_MAGIC_NUMBER],
    chunk_size: u32,
    version: u16,
    header_size: u16,
    offset_to_offset_table: u32,
}

#[derive(Debug, Default)]
struct PfChunkData {
    chunk_header: PfChunkHeader,
    chunk_data: Vec<u8>,
    offset_count: u32,
    offset_data: Vec<u32>,
    padding: Vec<u8>,
}
