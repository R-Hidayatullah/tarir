#![allow(dead_code)]
#![allow(unused_variables)]

use std::env;
use std::fs::File;
use std::io;
use std::io::BufWriter;

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
    let default_file_path =
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Guild Wars 2\\Gw2.dat";
    let default_index_number = 17;

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

    // Extract MFT data with the provided or default index number
    let (result, name_file) =
        dat_file.extract_mft_data(ArchiveId::BaseId, index_number as usize)?;

    // save_image(result, name_file.as_str());

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
