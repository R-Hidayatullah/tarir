#![feature(seek_stream_len)]

use std::env;
use std::io;

use dat_parser::ArchiveId;
use dat_parser::hex_dump;

mod dat_decompress;
mod dat_parser;
mod pf_parser;

fn main() -> io::Result<()> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();

    // Default values
    let default_file_path =
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Guild Wars 2\\Gw2.dat";
    // let default_file_path = "Local.dat";

    let default_index_number = 16;

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

    let (raw_data, decompressed_data, name_file) =
        dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize)?;

    println!("Filename : {}", name_file);
    println!("\nCompressed Size : {}", raw_data.len());
    hex_dump(&raw_data);
    println!("Decompressed Size : {}", decompressed_data.len());
    hex_dump(&decompressed_data);

    Ok(())
}
