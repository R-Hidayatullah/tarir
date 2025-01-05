#![allow(dead_code)]

use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, Read, Seek, Write};
use std::os::windows::fs::MetadataExt;
use std::path::Path;

use crate::dat_decompress;

/// The length of the DAT file identifier, typically "AN(" in ASCII.
const DAT_MAGIC_NUMBER: usize = 3;
/// The length of the MFT file identifier, typically "Mft→" in ASCII.
const MFT_MAGIC_NUMBER: usize = 4;
/// Index in the MFT data where the base ID and file ID are stored.
const MFT_ENTRY_INDEX_NUM: usize = 1;

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
    /// Skipped when parsing data first time, becaus its take long time
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
        let file_path_str = file_path.as_ref().to_str().unwrap_or("").to_string();
        if !&file_path_str.to_lowercase().ends_with(".dat") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file extension. Expected '.dat'.",
            ));
        }

        let file_path_data = file_path_str.clone();
        // Open the file and create a buffered reader.
        let file = File::open(file_path)?;

        // Initialize the DatFile structure with default values.
        let mut data_dat_file = DatFile {
            filename: String::new(),
            file_size: 0,
            dat_header: Default::default(),
            mft_header: Default::default(),
            mft_data: Default::default(),
            mft_index_data: Default::default(),
            dat_file: BufReader::new(file),
        };

        data_dat_file.filename = file_path_data;
        data_dat_file.file_size = data_dat_file.dat_file.stream_len()?;

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
            .seek(std::io::SeekFrom::Start(self.dat_header.mft_offset))?;
        self.dat_file.read_exact(&mut self.mft_header.identifier)?;
        self.mft_header.unknown_field = self.dat_file.read_u64::<LittleEndian>()?;
        self.mft_header.mft_entry_size = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.unknown_field_2 = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.unknown_field_3 = self.dat_file.read_u32::<LittleEndian>()?;
        self.mft_header.mft_entry_size = self.mft_header.mft_entry_size - 1;

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
        let num_index_entries = self.mft_data.get(MFT_ENTRY_INDEX_NUM).unwrap().size
            / std::mem::size_of::<MftIndexData>() as u32;
        let mft_index_data_offset = self.mft_data.get(MFT_ENTRY_INDEX_NUM).unwrap().offset;

        self.dat_file
            .seek(std::io::SeekFrom::Start(mft_index_data_offset))?;

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
    ) -> std::io::Result<(Vec<u8>, String)> {
        let mut index_found: usize = 0;
        match archive_id {
            ArchiveId::FileId => {
                for i in 0..self.mft_index_data.len() {
                    if self.mft_index_data.get(i).unwrap().file_id as usize == number {
                        println!("Found : {:#?}", self.mft_index_data.get(i).unwrap());

                        index_found = self.mft_index_data.get(i).unwrap().base_id as usize - 1;
                    }
                }
            }
            ArchiveId::BaseId => {
                for i in 0..self.mft_index_data.len() {
                    if self.mft_index_data.get(i).unwrap().base_id as usize == number {
                        println!("Found : {:#?}", self.mft_index_data.get(i).unwrap());
                        index_found = self.mft_index_data.get(i).unwrap().base_id as usize - 1;
                    }
                }
            }
        }
        let mft_entry = self.mft_data.get(index_found).unwrap();
        println!("Inside : {:#?}", mft_entry);
        println!("MFT Chunk CRC 32C : {:08X?}", mft_entry.crc);
        let buffer_size = self.mft_data.get(index_found).unwrap().size;
        self.dat_file
            .seek(std::io::SeekFrom::Start(mft_entry.offset))?;

        let mut buffer_data = Vec::with_capacity(buffer_size as usize);

        let mut result_crc: Vec<u32> = Vec::new();

        #[allow(unused_assignments)]
        let mut name_file = String::new();

        let chunk_count = buffer_size / 0x10000;
        let last_chunk_size: usize = (buffer_size as usize % 0x10000) - 4;

        println!(
            "Chunk count : {} Chunk size : {}",
            chunk_count, last_chunk_size
        );

        if chunk_count >= 1 {
            for _ in 0..chunk_count {
                let mut chunk_buffer: Vec<u8> = Vec::with_capacity(0x10000 - 4);
                chunk_buffer.resize(0x10000 - 4, 0);
                self.dat_file.read_exact(&mut chunk_buffer)?;
                buffer_data.append(&mut chunk_buffer);
                result_crc.push(self.dat_file.read_u32::<LittleEndian>()?);
            }
        }

        let mut last_chunk_buffer: Vec<u8> = Vec::with_capacity(last_chunk_size);
        last_chunk_buffer.resize(last_chunk_size, 0);
        self.dat_file.read_exact(&mut last_chunk_buffer)?;
        buffer_data.append(&mut last_chunk_buffer);
        result_crc.push(self.dat_file.read_u32::<LittleEndian>()?);

        for crc_data in result_crc {
            println!("CRC 32C : {:08X?}", crc_data);
        }

        // let mut dump_data = File::create("buffer_19.bin")?;
        // dump_data.write_all(&buffer_data)?;

        println!("\nBuffer Length : {}", buffer_size);
        println!("\nActual Buffer Length : {}", buffer_data.len());

        self.hex_dump(&buffer_data);
        name_file = index_found.to_string();

        if mft_entry.compression_flag != 0 {
            println!("File data is compressed!");

            let mut output_data_size: u32 = 0;
            let mut output_data: Vec<u8> = Vec::new();
            dat_decompress::inflate_dat_file_buffer(
                buffer_data,
                &mut output_data_size,
                &mut output_data,
            )?;

            // let mut texture_output_data_size: u32 = 0;
            // let mut texture_output_data: Vec<u8> = Vec::new();

            // texture_decompress::inflate_texture_file_buffer(
            //     output_data.clone(),
            //     &mut texture_output_data_size,
            //     &mut texture_output_data,
            // )?;

            // println!("Texture output data size : {}", texture_output_data_size);
            println!("\nBuffer Length : {}", output_data.len());

            self.hex_dump(&output_data);
            return Ok((output_data, name_file));
        } else {
            println!("File data isn't compressed!");
            Ok((buffer_data, name_file))
        }
    }

    /// Print a hex dump of the given buffer.
    fn hex_dump(&self, buffer: &Vec<u8>) {
        const BYTES_PER_LINE: usize = 16;

        for (i, chunk) in buffer.chunks(BYTES_PER_LINE).enumerate() {
            if i == 8 {
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
}
