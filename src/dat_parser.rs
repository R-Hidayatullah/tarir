#![allow(dead_code)]
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::dat_decompress;

/// The length of the DAT file identifier, typically "AN(" in ASCII.
const DAT_MAGIC_NUMBER: usize = 3;
/// The length of the MFT file identifier, typically "Mft→" in ASCII.
const MFT_MAGIC_NUMBER: usize = 4;
/// Index in the MFT data where the base ID and file ID are stored.
const MFT_ENTRY_INDEX_NUM: usize = 1;

const CHUNK_SIZE: usize = 0x10000;

pub enum ArchiveId {
    FileId,
    BaseId,
}

#[derive(Debug, Default)]
pub struct DatHeader {
    /// The version of the DAT file format. Usually set to 151.
    pub version: u8,
    /// A 3-character ASCII identifier, typically "AN(".
    pub identifier: [u8; DAT_MAGIC_NUMBER],
    /// The size of the header in bytes, typically 40 bytes.
    pub header_size: u32,
    /// Purpose unknown; requires further analysis.
    pub unknown_field: u32,
    /// Size of data chunks, usually 512 bytes. This might define block sizes used in the self.dat_file.
    pub chunk_size: u32,
    /// CRC (Cyclic Redundancy Check) for verifying the integrity of the header or associated data.
    pub crc: u32,
    /// Another unknown field; its purpose is unclear.
    pub unknown_field_2: u32,
    /// Offset in the file where the MFT (Master File Table) starts.
    pub mft_offset: u64,
    /// Size of the MFT in bytes.
    pub mft_size: u32,
    /// A flag field; its purpose is currently unclear but may indicate file properties or settings.
    pub flag: u32,
}

#[derive(Debug, Default)]
pub struct MftHeader {
    /// A 4-character ASCII identifier, typically "Mft→".
    pub identifier: [u8; MFT_MAGIC_NUMBER],
    /// Purpose unknown; possibly metadata or reserved space.
    pub unknown_field: u64,
    /// The number of entries in the MFT. Can be large; for large files like Gw2.dat, be cautious when seeking offsets.
    pub mft_entry_size: u32,
    /// Another unknown field; its role is unclear.
    pub unknown_field_2: u32,
    /// Yet another unknown field; requires further investigation.
    pub unknown_field_3: u32,
}

#[derive(Debug, Default)]
pub struct MftData {
    /// The offset in the file where the data for this entry begins.
    pub offset: u64,
    /// The size of the data for this entry in bytes.
    pub size: u32,
    /// Indicates compression status: 8 means the file is compressed.
    pub compression_flag: u16,
    /// Flags related to the entry; exact meaning requires further analysis.
    pub entry_flag: u16,
    /// A counter or version number; its exact role is unclear.
    pub counter: u32,
    /// CRC (Cyclic Redundancy Check) for verifying the integrity of this entry.
    pub crc: u32,

    /// Customized data, is not part of the game real data
    /// Skipped when parsing data first time, because it takes a long time
    pub uncompressed_size: u32,
    /// u64 for position crc_32c data begin, the other one is the data itself 4 of u8 data in u32
    pub crc_32c_data: Vec<(u64, u32)>,
}

#[derive(Debug, Default)]
pub struct MftIndexData {
    /// A unique identifier for a specific self.dat_file. Multiple file IDs can reference the same base ID, indicating that these files are related or derived from the same source.
    pub file_id: u32,
    /// The index of the actual or "true" file number in the system. Acts as a reference to the primary file or data this file ID is associated with.
    pub base_id: u32,
}

#[derive(Debug)]
pub struct DatFile {
    pub filename: String,
    pub file_size: u64,
    pub dat_header: DatHeader,
    pub mft_header: MftHeader,
    pub mft_data: Vec<MftData>,
    pub mft_index_data: Vec<MftIndexData>,
    pub dat_file: BufReader<File>,
}

impl DatFile {
    /// Load a `.dat` file and parse its contents into a `DatFile` structure.
    pub fn load<P: AsRef<Path>>(file_path: P) -> std::io::Result<DatFile> {
        // Check if the file extension is '.dat'
        let file_path_str = file_path.as_ref().to_str().unwrap_or_default().to_string();
        if !file_path_str.to_lowercase().ends_with(".dat") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file extension. Expected '.dat'.",
            ));
        }

        // Open the file and create a buffered reader.
        let file = File::open(file_path)?;
        let mut dat_file = BufReader::new(file);

        // Initialize the DatFile structure with default values.
        let mut data_dat_file = DatFile {
            filename: file_path_str,
            file_size: dat_file.stream_len()?,
            dat_header: Default::default(),
            mft_header: Default::default(),
            mft_data: Default::default(),
            mft_index_data: Default::default(),
            dat_file,
        };

        // Read and parse the headers and data.
        data_dat_file.read_dat_header()?;
        data_dat_file.read_mft_header()?;
        data_dat_file.read_mft_data()?;
        data_dat_file.read_mft_index_data()?;

        Ok(data_dat_file)
    }

    /// Read and parse the DAT file header.
    fn read_dat_header(&mut self) -> std::io::Result<()> {
        self.dat_header.version = self.dat_file.read_u8()?;
        self.dat_file.read_exact(&mut self.dat_header.identifier)?;
        self.dat_header.header_size = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.unknown_field = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.chunk_size = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.crc = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.unknown_field_2 = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.mft_offset = self.dat_file.read_u64::<LittleEndian>()?;
        self.dat_header.mft_size = self.dat_file.read_u32::<LittleEndian>()?;
        self.dat_header.flag = self.dat_file.read_u32::<LittleEndian>()?;
        Ok(())
    }

    /// Read and parse the MFT file header.
    fn read_mft_header(&mut self) -> std::io::Result<()> {
        self.dat_file
            .seek(SeekFrom::Start(self.dat_header.mft_offset))?;
        self.dat_file.read_exact(&mut self.mft_header.identifier)?;
        self.mft_header.unknown_field = self.dat_file.read_u64::<LittleEndian>()?;
        self.mft_header.mft_entry_size = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.unknown_field_2 = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.unknown_field_3 = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.mft_entry_size -= 1; // Adjust size based on data format
        Ok(())
    }

    /// Read and parse the MFT data entries.
    fn read_mft_data(&mut self) -> std::io::Result<()> {
        for _ in 0..self.mft_header.mft_entry_size {
            let offset = self.dat_file.read_u64::<LittleEndian>()?;
            let size = self.dat_file.read_u32::<LittleEndian>()?;
            let compression_flag = self.dat_file.read_u16::<LittleEndian>()?;
            let entry_flag = self.dat_file.read_u16::<LittleEndian>()?;
            let counter = self.dat_file.read_u32::<LittleEndian>()?;
            let crc = self.dat_file.read_u32::<LittleEndian>()?;
            self.mft_data.push(MftData {
                offset,
                size,
                compression_flag,
                entry_flag,
                counter,
                crc,
                uncompressed_size: Default::default(),
                crc_32c_data: Default::default(),
            });
        }
        Ok(())
    }

    /// Read and parse the MFT index data.
    fn read_mft_index_data(&mut self) -> std::io::Result<()> {
        let num_index_entries = self.mft_data.get(MFT_ENTRY_INDEX_NUM).map_or(0, |entry| {
            entry.size / std::mem::size_of::<MftIndexData>() as u32
        });
        let mft_index_data_offset = self
            .mft_data
            .get(MFT_ENTRY_INDEX_NUM)
            .map_or(0, |entry| entry.offset);

        self.dat_file.seek(SeekFrom::Start(mft_index_data_offset))?;

        for _ in 0..num_index_entries {
            let file_id = self.dat_file.read_u32::<LittleEndian>()?;
            let base_id = self.dat_file.read_u32::<LittleEndian>()?;
            self.mft_index_data.push(MftIndexData { file_id, base_id });
        }
        Ok(())
    }

    pub fn extract_mft_data(
        &mut self,
        archive_id: ArchiveId,
        number: usize,
    ) -> std::io::Result<(Vec<u8>, Vec<u8>, String)> {
        let mut index_found: usize = 0;
        match archive_id {
            ArchiveId::FileId => {
                for i in 0..self.mft_index_data.len() {
                    if self.mft_index_data.get(i).unwrap().file_id as usize == number {
                        index_found = self.mft_index_data.get(i).unwrap().base_id as usize - 1;
                    }
                }
            }
            ArchiveId::BaseId => {
                for i in 0..self.mft_index_data.len() {
                    if self.mft_index_data.get(i).unwrap().base_id as usize == number {
                        index_found = self.mft_index_data.get(i).unwrap().base_id as usize - 1;
                    }
                }
            }
        }
        let mft_entry = self.mft_data.get(index_found).unwrap();
        #[allow(unused_mut)]
        let raw_data_size = self.mft_data.get(index_found).unwrap().size;
        self.dat_file
            .seek(std::io::SeekFrom::Start(mft_entry.offset))?;

        let mut raw_data = Vec::with_capacity(raw_data_size as usize);
        raw_data.resize(raw_data_size as usize, 0);
        self.dat_file.read_exact(&mut raw_data)?;
        let mut raw_data_cleaned = raw_data.clone();

        // CRC-32C (Cyclic Redundancy Check 32-bit Castagnoli) is a variant of the CRC-32 algorithm that uses the Castagnoli polynomial.
        // Define the range to remove 4 bytes from each cycle
        let start_index = CHUNK_SIZE - 4; // Start of the range to remove
        let end_index = CHUNK_SIZE; // End of the range to remove

        // Check the size of the raw data
        if raw_data_size > CHUNK_SIZE as u32 {
            // If data is larger than 0x10000, remove 4 bytes in each cycle
            while raw_data_cleaned.len() > raw_data_size as usize - 4 {
                // Remove 4 bytes from the specified range
                raw_data_cleaned.drain(start_index..end_index);
            }
            if raw_data_cleaned.len() > 4 {
                raw_data_cleaned.truncate(raw_data_cleaned.len() - 4);
            }
        } else if raw_data_size == CHUNK_SIZE as u32 {
            // If data is exactly 0x10000, remove 4 bytes from the specified range
            raw_data_cleaned.drain(start_index..end_index);
        } else if raw_data_size < CHUNK_SIZE as u32 {
            // If data is smaller than 0x10000, no removal, just truncate the last 4 bytes
            if raw_data_cleaned.len() > 4 {
                raw_data_cleaned.truncate(raw_data_cleaned.len() - 4);
            }
        }

        let name_file = index_found.to_string();

        if mft_entry.compression_flag != 0 {
            let mut decompressed_data_size: u32 = 0;
            let mut decompressed_data: Vec<u8> = Vec::new();
            dat_decompress::inflate_dat_file_buffer(
                raw_data_cleaned,
                &mut decompressed_data_size,
                &mut decompressed_data,
            )?;

            return Ok((raw_data, decompressed_data, name_file));
        } else {
            Ok((raw_data, raw_data_cleaned, name_file))
        }
    }
}

/// Print a hex dump of the given buffer.
pub fn hex_dump(buffer: &Vec<u8>) {
    const BYTES_PER_LINE: usize = 16;

    for (i, chunk) in buffer.chunks(BYTES_PER_LINE).enumerate() {
        if i == 16 {
            break;
        }
        // Print the offset
        print!("{:08X}: ", i * BYTES_PER_LINE);

        // Print the hexadecimal representation
        for byte in chunk {
            print!("{:02X} ", byte);
        }

        // Pad the last line with spaces if necessary
        for _ in 0..(BYTES_PER_LINE - chunk.len()) {
            print!("   ");
        }

        // Print the ASCII representation
        print!("|");
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!("|");
    }
    println!()
}
