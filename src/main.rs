#![allow(dead_code)]
#![allow(unused_variables)]
#![feature(seek_stream_len)]

use std::env;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;

use dat_parser::ArchiveId;
use image::ImageFormat;
use image::load_from_memory;

mod dat_decompress;
mod dat_parser;
mod pf_parser;
mod texture_decompress;

fn main() -> io::Result<()> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();

    // Default values
    // let default_file_path =
    //     "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Guild Wars 2\\Gw2.dat";
    let default_file_path = "Local.dat";

    let default_index_number = 19;

    // Parse command line arguments
    let file_path = if args.len() > 1 {
        &args[1]
    } else {
        default_file_path
    };

    let index_number: u32 = if args.len() > 2 {
        args[2].parse::<u32>().unwrap_or(default_index_number)
    } else {
        default_index_number
    };

    // Load the DAT file
    let mut dat_file = dat_parser::DatFile::load(file_path)?;
    println!("{:#?}", dat_file.dat_header);
    println!("{:#?}", dat_file.mft_header);
    println!("Size mft index : {}", dat_file.mft_index_data.len());
    // Extract MFT data with the provided or default index number
    let (result, name_file) =
        dat_file.extract_mft_data(ArchiveId::FileId, index_number as usize)?;

    // let mut dump_data = File::create("buffer_19_v2.bin")?;
    // dump_data.write_all(&result)?;

    // save_image(result, name_file.as_str());
    // let file_path = "buffer_31_first_chunk.bin"; // Path to your binary file
    // match compute_crc32c_from_file(file_path) {
    //     //should be A9C0541F
    //     Ok(checksum) => {
    //         println!("CRC-32C checksum: 0x{:08X}", checksum);
    //     }
    //     Err(e) => {
    //         eprintln!("Error reading file: {}", e);
    //     }
    // }

    Ok(())
}

fn save_image(vec_data: Vec<u8>, custom_name: &str) {
    // Try to load the image from the byte vector
    if let Ok(img) = load_from_memory(&vec_data) {
        // Save the image as PNG
        if let Ok(file) = File::create(format!("{}.png", custom_name)) {
            let ref mut writer = BufWriter::new(file);

            let _ = img.write_to(writer, ImageFormat::Png);
            println!("Image saved as : {}.png", custom_name);
        }
    }
}

fn compute_crc32c_from_file(file_path: &str) -> io::Result<u32> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();

    // Read the entire file into the buffer
    reader.read_to_end(&mut buffer)?;

    // Optionally, you can adjust here to compute CRC only over a specific part of the file.
    // For example, you might want to compute CRC only for the header or data section.

    let result_data = crc32c::crc32c(&buffer);
    // Compute the CRC-32C checksum
    Ok(result_data)
}
