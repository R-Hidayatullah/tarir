#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Seek};

const MAX_BITS_HASH: usize = 8;
const MAX_CODE_BITS_LENGTH: usize = 32;
const MAX_SYMBOL_VALUE: usize = 285;

const SKIPPED_BYTES_PER_CHUNK: usize = 16384; // 0x4000
const BYTES_TO_REMOVE: usize = 4; // sizeof(u32)

#[derive(Debug, Default)]
struct StateData {
    input_buffer: Cursor<Vec<u8>>,
    buffer_position: u64,
    bytes_available: u32,
    skipped_bytes: u32,
    head_data: u32,
    buffer_data: u32,
    bytes_available_data: u8,
}

#[derive(Debug)]
struct HuffmanTree {
    code_comparison: [u32; MAX_CODE_BITS_LENGTH],
    symbol_value_offset: [u16; MAX_CODE_BITS_LENGTH],
    code_bits: [u8; MAX_CODE_BITS_LENGTH],
    symbol_value: [u16; MAX_SYMBOL_VALUE],
    symbol_value_hash_exist: [bool; 1 << MAX_BITS_HASH],
    symbol_value_hash: [u16; 1 << MAX_BITS_HASH],
    code_bits_hash: [u8; 1 << MAX_BITS_HASH],
}

impl Default for HuffmanTree {
    fn default() -> Self {
        HuffmanTree {
            code_comparison: [0; MAX_CODE_BITS_LENGTH],
            symbol_value_offset: [0; MAX_CODE_BITS_LENGTH],
            code_bits: [0; MAX_CODE_BITS_LENGTH],
            symbol_value: [0; MAX_SYMBOL_VALUE],
            symbol_value_hash_exist: [false; 1 << MAX_BITS_HASH],
            symbol_value_hash: [0; 1 << MAX_BITS_HASH],
            code_bits_hash: [0; 1 << MAX_BITS_HASH],
        }
    }
}

#[derive(Debug)]
struct HuffmanTreeBuilder {
    bits_head_exist: [bool; MAX_CODE_BITS_LENGTH],
    bits_head: [u16; MAX_CODE_BITS_LENGTH],
    bits_body_exist: [bool; MAX_SYMBOL_VALUE],
    bits_body: [u16; MAX_SYMBOL_VALUE],
}

impl Default for HuffmanTreeBuilder {
    fn default() -> Self {
        HuffmanTreeBuilder {
            bits_head_exist: [false; MAX_CODE_BITS_LENGTH],
            bits_head: [0; MAX_CODE_BITS_LENGTH],
            bits_body_exist: [false; MAX_SYMBOL_VALUE],
            bits_body: [0; MAX_SYMBOL_VALUE],
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Format {
    flag_data: u16,
    pixel_size_bits: u16,
}

#[derive(Debug, Default)]
struct FullFormat {
    format: Format,
    pixel_blocks: u32,
    bytes_pixel_blocks: u32,
    bytes_component: u32,
    two_component: bool,
    width: u16,
    height: u16,
}

enum FormatFlags {
    FfColor = 0x10,
    FfAlpha = 0x20,
    FfDeducedalphacomp = 0x40,
    FfPlaincomp = 0x80,
    FfBicolorcomp = 0x200,
}

enum CompressionFlags {
    CfDecodeWhiteColor = 0x01,
    CfDecodeConstantAlphaFrom4bits = 0x02,
    CfDecodeConstantAlphaFrom8bits = 0x04,
    CfDecodePlainColor = 0x08,
}

fn pull_byte(
    state_data: &mut StateData,
    head_data: &mut u32,
    bytes_available_data: &mut u8,
) -> std::io::Result<()> {
    if state_data.bytes_available >= std::mem::size_of::<u32>() as u32 {
        if state_data.skipped_bytes != 0 {
            if ((state_data.buffer_position / std::mem::size_of::<u32>() as u64) + 1)
                % state_data.skipped_bytes as u64
                == 0
            {
                state_data.bytes_available -= std::mem::size_of::<u32>() as u32;
                state_data.input_buffer.read_u32::<LittleEndian>()?; // Skipping 4 bytes, for CRC probably
                state_data.buffer_position = state_data.input_buffer.position();
            }
        }
        *head_data = state_data.input_buffer.read_u32::<LittleEndian>()?;
        state_data.bytes_available -= std::mem::size_of::<u32>() as u32;
        state_data.buffer_position = state_data.input_buffer.position();
        *bytes_available_data = (std::mem::size_of::<u32>() as u32 * 8) as u8;
    } else {
        *head_data = 0;
        *bytes_available_data = 0;
    }
    Ok(())
}

fn read_bits(state_data: &mut StateData, bits_number: u8) -> std::io::Result<u32> {
    if state_data.bytes_available_data < bits_number {
        println!(
            "Not enough bits available to read the value. in position : {}",
            state_data.input_buffer.position()
        );
    }
    Ok(state_data.head_data >> (std::mem::size_of::<u32>() as u8 * 8) - bits_number)
}

fn drop_bits(state_data: &mut StateData, bits_number: u8) -> std::io::Result<()> {
    if state_data.bytes_available_data < bits_number {
        println!("Too much bits were asked to be dropped.");
    }
    #[allow(unused_assignments)]
    let mut new_bits_available: u8 = 0;
    new_bits_available = state_data.bytes_available_data.wrapping_sub(bits_number);
    if new_bits_available >= std::mem::size_of::<u32>() as u8 * 8 {
        if bits_number == std::mem::size_of::<u32>() as u8 * 8 {
            state_data.head_data = state_data.buffer_data;
            state_data.buffer_data = 0;
        } else {
            state_data.head_data = (state_data.head_data << bits_number)
                | (state_data.buffer_data >> (std::mem::size_of::<u32>() as u8 * 8) - bits_number);
            state_data.buffer_data = state_data.buffer_data << bits_number;
        }
        state_data.bytes_available_data = new_bits_available;
    } else {
        let mut new_value: u32 = 0;
        let mut pulled_bits: u8 = 0;
        pull_byte(state_data, &mut new_value, &mut pulled_bits)?;

        if bits_number == std::mem::size_of::<u32>() as u8 * 8 {
            state_data.head_data = 0;
        } else {
            state_data.head_data = state_data.head_data << bits_number;
        }
        state_data.head_data |= (state_data.buffer_data
            >> ((std::mem::size_of::<u32>() as u8 * 8) - bits_number))
            | (new_value >> (new_bits_available));
        if new_bits_available > 0 {
            state_data.buffer_data =
                new_value << (std::mem::size_of::<u32>() as u8 * 8) - new_bits_available;
        }
        state_data.bytes_available_data = new_bits_available + pulled_bits;
    }
    Ok(())
}

fn read_code(
    huffmantree_data: &mut HuffmanTree,
    state_data: &mut StateData,
    symbol_data: &mut u16,
) -> std::io::Result<()> {
    let index_num = read_bits(state_data, MAX_BITS_HASH as u8)? as usize;

    let exist = huffmantree_data.symbol_value_hash_exist[index_num];

    if exist {
        *symbol_data = huffmantree_data.symbol_value_hash
            [read_bits(state_data, MAX_BITS_HASH as u8)? as usize];

        let code_bits_hash =
            huffmantree_data.code_bits_hash[read_bits(state_data, MAX_BITS_HASH as u8)? as usize];

        drop_bits(state_data, code_bits_hash)?;
    } else {
        let mut index_data: u16 = 0;
        while read_bits(state_data, 32)? < huffmantree_data.code_comparison[index_data as usize] {
            index_data = index_data.wrapping_add(1);
        }

        let temp_bits: u8 = huffmantree_data.code_bits[index_data as usize];

        // Step 1: Read 32 bits from state_data
        let read_bits_value = read_bits(state_data, 32)?;

        // Step 2: Subtract code_comparison from read_bits_value (with wrapping)
        let adjusted_bits = read_bits_value
            .wrapping_sub(huffmantree_data.code_comparison[index_data as usize] as u32);

        // Step 3: Perform the right shift operation (with wrapping)
        let shifted_bits = adjusted_bits.wrapping_shr((32 - temp_bits as u16) as u32);

        // Step 4: Subtract the shifted value from the symbol_value_offset (with wrapping)
        let symbol_index = huffmantree_data.symbol_value_offset[index_data as usize]
            .wrapping_sub(shifted_bits as u16) as usize;

        // Step 5: Retrieve the symbol_data using the calculated index
        *symbol_data = huffmantree_data.symbol_value[symbol_index];

        drop_bits(state_data, temp_bits)?;
    }
    Ok(())
}

pub fn inflate_texture_file_buffer(
    input_data: Vec<u8>,
    output_data_size: &mut u32,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut texture_huffmantree_dict = HuffmanTree::default();
    let mut format_data: Vec<Format> = Vec::new();

    initialize_static_values(&mut texture_huffmantree_dict, &mut format_data)?;

    let mut state_data = StateData::default();
    state_data.bytes_available = input_data.len() as u32;
    state_data.input_buffer = Cursor::new(input_data);
    state_data.skipped_bytes = 0 as u32;
    let mut head_data: u32 = 0;
    let mut bytes_available_data: u8 = 0;

    pull_byte(&mut state_data, &mut head_data, &mut bytes_available_data)?;

    state_data.head_data = head_data;
    state_data.bytes_available_data = bytes_available_data;

    drop_bits(&mut state_data, 32)?;

    let mut fourcc_format: u32 = 0;
    fourcc_format = read_bits(&mut state_data, 32)?;
    drop_bits(&mut state_data, 32)?;

    let mut full_format_data = FullFormat::default();
    full_format_data.format = deduce_format(fourcc_format, format_data)?;

    full_format_data.width = read_bits(&mut state_data, 16)? as u16;
    drop_bits(&mut state_data, 16)?;
    full_format_data.height = read_bits(&mut state_data, 16)? as u16;
    drop_bits(&mut state_data, 16)?;

    full_format_data.pixel_blocks =
        ((full_format_data.width as u32 + 3) / 4) * ((full_format_data.height as u32 + 3) / 4);
    full_format_data.bytes_pixel_blocks =
        (full_format_data.format.pixel_size_bits as u32 * 4 * 4) / 8;
    full_format_data.bytes_component =
        full_format_data.bytes_pixel_blocks / if full_format_data.two_component { 2 } else { 1 };

    let mut texture_output_size: u32 = 0;
    texture_output_size = full_format_data.bytes_pixel_blocks * full_format_data.pixel_blocks;

    if (*output_data_size != 0 && *output_data_size < texture_output_size) {
        println!("Output buffer is too small.");
    }
    *output_data_size = texture_output_size;

    output_data.resize(*output_data_size as usize, 0);

    inflate_texture_data(
        &mut state_data,
        &full_format_data,
        &mut texture_output_size,
        output_data,
        &mut texture_huffmantree_dict,
    )?;

    Ok(())
}

fn inflate_texture_data(
    state_data: &mut StateData,
    fullformat_data: &FullFormat,
    texture_output_data_size: &mut u32,
    output_data: &mut Vec<u8>,
    texture_huffmantree_dict: &mut HuffmanTree,
) -> std::io::Result<()> {
    let mut color_bitmap_data: Vec<bool> = Vec::new();
    let mut alpha_bitmap_data: Vec<bool> = Vec::new();
    color_bitmap_data.reserve(fullformat_data.pixel_blocks as usize);
    alpha_bitmap_data.reserve(fullformat_data.pixel_blocks as usize);

    let mut data_size: u32 = 0;
    data_size = read_bits(state_data, 32)?;
    drop_bits(state_data, 32)?;
    println!("Data size : {}", data_size);
    let mut compression_flag_data: u32 = 0;
    compression_flag_data = read_bits(state_data, 32)?;
    drop_bits(state_data, 32)?;
    println!("Compression flags : {}", compression_flag_data);

    println!(
        "full_format_data.pixel_blocks : {}",
        fullformat_data.pixel_blocks
    );
    color_bitmap_data.resize(fullformat_data.pixel_blocks as usize, false);
    alpha_bitmap_data.resize(fullformat_data.pixel_blocks as usize, false);

    if (compression_flag_data & CompressionFlags::CfDecodeWhiteColor as u32) != 0 {
        println!(
            "Checking CfDecodeWhiteColor: {}",
            12 & CompressionFlags::CfDecodeWhiteColor as i32
        );
        decode_white_color(
            state_data,
            texture_huffmantree_dict,
            &mut alpha_bitmap_data,
            &mut color_bitmap_data,
            fullformat_data,
            output_data,
        )?;
    }

    if (compression_flag_data & CompressionFlags::CfDecodeConstantAlphaFrom4bits as u32) != 0 {
        println!(
            "Checking CfDecodeConstantAlphaFrom4bits: {}",
            12 & CompressionFlags::CfDecodeConstantAlphaFrom4bits as i32
        );
        decode_constant_alpha_from_4_bits(
            state_data,
            texture_huffmantree_dict,
            &mut alpha_bitmap_data,
            fullformat_data,
            output_data,
        )?;
    }

    if (compression_flag_data & CompressionFlags::CfDecodeConstantAlphaFrom8bits as u32) != 0 {
        println!(
            "Checking CfDecodeConstantAlphaFrom8bits: {}",
            12 & CompressionFlags::CfDecodeConstantAlphaFrom8bits as i32
        );
        decode_constant_alpha_from_8_bits(
            state_data,
            texture_huffmantree_dict,
            &mut alpha_bitmap_data,
            fullformat_data,
            output_data,
        )?;
    }

    if (compression_flag_data & CompressionFlags::CfDecodePlainColor as u32) != 0 {
        println!(
            "Checking CfDecodePlainColor: {}",
            12 & CompressionFlags::CfDecodePlainColor as i32
        );
        decode_plain_color(
            state_data,
            texture_huffmantree_dict,
            &mut color_bitmap_data,
            fullformat_data,
            output_data,
        )?;
    }

    let mut loop_index_data: u32 = 0;
    if state_data.bytes_available_data >= 32 {
        state_data
            .input_buffer
            .seek(std::io::SeekFrom::Current(-1))?;
        state_data.buffer_position = state_data.input_buffer.position();
    }

    Ok(())
}
pub fn inflate_texture_block_buffer(
    input_data: Vec<u8>,
    output_data_size: &mut u32,
    output_data: &mut Vec<u8>,
    width: u16,
    height: u16,
    fourcc_format: u32,
) -> std::io::Result<()> {
    Ok(())
}

fn initialize_static_values(
    texture_huffmantree_dict: &mut HuffmanTree,
    format_data: &mut Vec<Format>,
) -> std::io::Result<()> {
    // Number 1 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16
            | FormatFlags::FfAlpha as u16
            | FormatFlags::FfDeducedalphacomp as u16,
        pixel_size_bits: 4,
    });
    // Number 2 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16
            | FormatFlags::FfAlpha as u16
            | FormatFlags::FfPlaincomp as u16,
        pixel_size_bits: 8,
    });

    // Number 3 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16
            | FormatFlags::FfAlpha as u16
            | FormatFlags::FfPlaincomp as u16,
        pixel_size_bits: 8,
    });
    // Number 4 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16
            | FormatFlags::FfAlpha as u16
            | FormatFlags::FfPlaincomp as u16,
        pixel_size_bits: 8,
    });
    // Number 5 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16
            | FormatFlags::FfAlpha as u16
            | FormatFlags::FfPlaincomp as u16,
        pixel_size_bits: 8,
    });
    // Number 6 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfAlpha as u16 | FormatFlags::FfPlaincomp as u16,
        pixel_size_bits: 4,
    });
    // Number 7 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfColor as u16,
        pixel_size_bits: 8,
    });
    // Number 8 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfBicolorcomp as u16,
        pixel_size_bits: 8,
    });
    // Number 9 format data
    format_data.push(Format {
        flag_data: FormatFlags::FfBicolorcomp as u16,
        pixel_size_bits: 8,
    });

    if !initialize_huffmantree_dict(texture_huffmantree_dict)? {
        println!("Failed to initialize huffmantree dict!");
    }

    Ok(())
}

fn decode_white_color(
    state_data: &mut StateData,
    texture_huffmantree_dict: &mut HuffmanTree,
    alpha_bitmap: &mut Vec<bool>,
    color_bitmap: &mut Vec<bool>,
    fullformat_data: &FullFormat,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut pixel_block_position: u32 = 0;
    while pixel_block_position < fullformat_data.pixel_blocks {
        let mut temp_code: u16 = 0;
        read_code(texture_huffmantree_dict, state_data, &mut temp_code)?;
        let mut value_data = 0;
        value_data = read_bits(state_data, 1)?;
        drop_bits(state_data, 1)?;
        while temp_code > 0 {
            if !color_bitmap[pixel_block_position as usize] {
                if value_data != 0 {
                    output_data
                        [(fullformat_data.bytes_pixel_blocks * pixel_block_position) as usize] =
                        std::u64::MAX as u8;
                    alpha_bitmap[pixel_block_position as usize] = true;
                    color_bitmap[pixel_block_position as usize] = true;
                }
                temp_code = temp_code.wrapping_sub(1);
            }
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }
    }

    while pixel_block_position < fullformat_data.pixel_blocks
        && color_bitmap[pixel_block_position as usize]
    {
        pixel_block_position = pixel_block_position.wrapping_add(1);
    }
    Ok(())
}

fn decode_constant_alpha_from_4_bits(
    state_data: &mut StateData,
    texture_huffmantree_dict: &mut HuffmanTree,
    alpha_bitmap: &mut Vec<bool>,
    fullformat_data: &FullFormat,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut alpha_value_byte: u8 = 0;
    alpha_value_byte = read_bits(state_data, 4)? as u8;
    drop_bits(state_data, 4)?;
    let mut pixel_block_position: u32 = 0;

    let mut intermediate_byte: u16 = (alpha_value_byte | (alpha_value_byte << 4)) as u16;
    let mut interediate_word: u32 = (intermediate_byte | (intermediate_byte << 8)) as u32;
    let mut intermediate_dword: u64 = (interediate_word | (interediate_word << 16)) as u64;
    let mut alpha_value: u64 = intermediate_dword | (intermediate_dword << 32);
    let mut zero_data: u64 = 0;

    while pixel_block_position < fullformat_data.pixel_blocks {
        let mut temp_code: u16 = 0;
        read_code(texture_huffmantree_dict, state_data, &mut temp_code)?;
        let mut value_data: u32 = 0;
        value_data = read_bits(state_data, 1)?;
        drop_bits(state_data, 1)?;
        let mut exist: u8 = 0;
        exist = read_bits(state_data, 1)? as u8;
        if value_data != 0 {
            drop_bits(state_data, 1)?;
        }

        while temp_code > 0 {
            if !alpha_bitmap[pixel_block_position as usize] {
                if value_data != 0 {
                    let destination = &mut output_data[fullformat_data.bytes_pixel_blocks
                        as usize
                        * pixel_block_position as usize..];
                    let source = if exist != 0 { &alpha_value } else { &zero_data };

                    destination[0..fullformat_data.bytes_component as usize].copy_from_slice(
                        &source.to_le_bytes()[..fullformat_data.bytes_component as usize],
                    );

                    alpha_bitmap[pixel_block_position as usize] = true;
                }
                temp_code = temp_code.wrapping_sub(1);
            }
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }

        while pixel_block_position < fullformat_data.pixel_blocks
            && alpha_bitmap[pixel_block_position as usize]
        {
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }
    }
    Ok(())
}

fn decode_constant_alpha_from_8_bits(
    state_data: &mut StateData,
    texture_huffmantree_dict: &mut HuffmanTree,
    alpha_bitmap: &mut Vec<bool>,
    fullformat_data: &FullFormat,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut alpha_value_byte: u8 = 0;
    alpha_value_byte = read_bits(state_data, 8)? as u8;
    drop_bits(state_data, 8)?;
    let mut pixel_block_position: u32 = 0;

    let mut alpha_value: u64 = (alpha_value_byte | (alpha_value_byte.wrapping_shl(8))) as u64;
    let mut zero_data: u64 = 0;

    while pixel_block_position < fullformat_data.pixel_blocks {
        let mut temp_code: u16 = 0;
        read_code(texture_huffmantree_dict, state_data, &mut temp_code)?;
        let mut value_data: u32 = 0;
        value_data = read_bits(state_data, 1)?;
        drop_bits(state_data, 1)?;

        let mut exist: u8 = 0;
        exist = read_bits(state_data, 1)? as u8;
        if value_data != 0 {
            drop_bits(state_data, 1)?;
        }

        while temp_code > 0 {
            if !alpha_bitmap[pixel_block_position as usize] {
                if value_data != 0 {
                    let destination = &mut output_data[fullformat_data.bytes_pixel_blocks
                        as usize
                        * pixel_block_position as usize..];
                    let source = if exist != 0 { &alpha_value } else { &zero_data };

                    destination[0..fullformat_data.bytes_component as usize].copy_from_slice(
                        &source.to_le_bytes()[..fullformat_data.bytes_component as usize],
                    );
                    alpha_bitmap[pixel_block_position as usize] = true;
                }
                temp_code = temp_code.wrapping_sub(1);
            }
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }

        while pixel_block_position < fullformat_data.pixel_blocks
            && alpha_bitmap[pixel_block_position as usize]
        {
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }
    }
    Ok(())
}

fn decode_plain_color(
    state_data: &mut StateData,
    texture_huffmantree_dict: &mut HuffmanTree,
    color_bitmap: &mut Vec<bool>,
    fullformat_data: &FullFormat,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut blue_data: u16 = 0;
    blue_data = read_bits(state_data, 8)? as u16;
    drop_bits(state_data, 8)?;

    let mut green_data: u16 = 0;
    green_data = read_bits(state_data, 8)? as u16;
    drop_bits(state_data, 8)?;

    let mut red_data: u16 = 0;
    red_data = read_bits(state_data, 8)? as u16;
    drop_bits(state_data, 8)?;
    let mut temp_red_data_1: u8 = 0;
    let mut temp_blue_data_1: u8 = 0;
    let mut temp_green_data_1: u16 = 0;

    temp_red_data_1 = ((red_data - (red_data >> 5)) >> 3) as u8;
    temp_blue_data_1 = ((blue_data - (blue_data >> 5)) >> 3) as u8;
    temp_green_data_1 = (green_data - (green_data >> 6)) >> 2;

    let mut temp_red_data_2: u8 = 0;
    let mut temp_blue_data_2: u8 = 0;
    let mut temp_green_data_2: u16 = 0;

    temp_red_data_2 = (temp_red_data_1 << 3) + (temp_red_data_1 >> 2);
    temp_blue_data_2 = (temp_blue_data_1 << 3) + (temp_blue_data_1 >> 2);
    temp_green_data_2 = (temp_green_data_1 << 2) + (temp_green_data_1 >> 4);

    let mut comparison_red: u32 = 0;
    let mut comparison_blue: u32 = 0;
    let mut comparison_green: u32 = 0;
    unimplemented!();
    // comparison_red = 12 * (red_data - temp_red_data_2) / (8 - ((temp_red_data_1 & 0x11) == 0x11 ? 1 : 0));
    // comparison_blue = 12 * (blue_data - temp_blue_data_2) / (8 - ((temp_blue_data_1 & 0x11) == 0x11 ? 1 : 0));
    // comparison_green = 12 * (green_data - temp_green_data_2) / (8 - ((temp_green_data_1 & 0x1111) == 0x1111 ? 1 : 0));

    let mut value_red_1: u32 = 0;
    let mut value_red_2: u32 = 0;

    if (comparison_red < 2) {
        value_red_1 = temp_red_data_1 as u32;
        value_red_2 = temp_red_data_1 as u32;
    } else if (comparison_red < 6) {
        value_red_1 = temp_red_data_1 as u32;
        value_red_2 = temp_red_data_1 as u32 + 1;
    } else if (comparison_red < 10) {
        value_red_1 = temp_red_data_1 as u32 + 1;
        value_red_2 = temp_red_data_1 as u32;
    } else {
        value_red_1 = temp_red_data_1 as u32 + 1;
        value_red_2 = temp_red_data_1 as u32 + 1;
    }

    let mut value_blue_1: u32 = 0;
    let mut value_blue_2: u32 = 0;

    if (comparison_blue < 2) {
        value_blue_1 = temp_blue_data_1 as u32;
        value_blue_2 = temp_blue_data_1 as u32;
    } else if (comparison_blue < 6) {
        value_blue_1 = temp_blue_data_1 as u32;
        value_blue_2 = temp_blue_data_1 as u32 + 1;
    } else if (comparison_blue < 10) {
        value_blue_1 = temp_blue_data_1 as u32 + 1;
        value_blue_2 = temp_blue_data_1 as u32;
    } else {
        value_blue_1 = temp_blue_data_1 as u32 + 1;
        value_blue_2 = temp_blue_data_1 as u32 + 1;
    }

    let mut value_green_1: u32 = 0;
    let mut value_green_2: u32 = 0;

    if (comparison_green < 2) {
        value_green_1 = temp_green_data_1 as u32;
        value_green_2 = temp_green_data_1 as u32;
    } else if (comparison_green < 6) {
        value_green_1 = temp_green_data_1 as u32;
        value_green_2 = temp_green_data_1 as u32 + 1;
    } else if (comparison_green < 10) {
        value_green_1 = temp_green_data_1 as u32 + 1;
        value_green_2 = temp_green_data_1 as u32;
    } else {
        value_green_1 = temp_green_data_1 as u32 + 1;
        value_green_2 = temp_green_data_1 as u32 + 1;
    }

    let mut value_color_1: u32 = 0;
    let mut value_color_2: u32 = 0;

    value_color_1 = value_red_1 | ((value_green_1 | (value_blue_1 << 6)) << 5);
    value_color_2 = value_red_2 | ((value_green_2 | (value_blue_2 << 6)) << 5);

    let mut temp_value_1: u32 = 0;
    let mut temp_value_2: u32 = 0;

    if (value_red_1 != value_red_2) {
        if (value_red_1 == temp_red_data_1 as u32) {
            temp_value_1 += comparison_red;
        } else {
            temp_value_1 += (12 - comparison_red);
        }
        temp_value_2 += 1;
    }

    if (value_blue_1 != value_blue_2) {
        if (value_blue_1 == temp_blue_data_1 as u32) {
            temp_value_1 += comparison_blue;
        } else {
            temp_value_1 += (12 - comparison_blue);
        }
        temp_value_2 += 1;
    }

    if (value_green_1 != value_green_2) {
        if (value_green_1 == temp_green_data_1 as u32) {
            temp_value_1 += comparison_green;
        } else {
            temp_value_1 += (12 - comparison_green);
        }
        temp_value_2 += 1;
    }

    if (temp_value_2 > 0) {
        temp_value_1 = (temp_value_1 + (temp_value_2 / 2)) / temp_value_2;
    }

    let mut special_case_dxt1 = false;
    special_case_dxt1 =
        ((fullformat_data.format.flag_data & FormatFlags::FfDeducedalphacomp as u16) != 0)
            && (temp_value_1 == 5 || temp_value_1 == 6 || temp_value_2 != 0);

    if (temp_value_2 > 0 && !special_case_dxt1) {
        if (value_color_2 == 0xFFFF) {
            temp_value_1 = 12;
            value_color_1 = value_color_1.wrapping_sub(1);
        } else {
            temp_value_1 = 0;
            value_color_2 = value_color_2.wrapping_add(1);
        }
    }

    if value_color_2 >= value_color_1 {
        let mut swap_temp: u32 = 0;
        swap_temp = value_color_1;
        value_color_1 = value_color_2;
        value_color_2 = swap_temp;

        temp_value_1 = temp_value_1.wrapping_sub(1);
    }
    let mut color_selected: u32 = 0;

    if (special_case_dxt1) {
        color_selected = 2;
    } else {
        if (temp_value_1 < 2) {
            color_selected = 0;
        } else if (temp_value_1 < 6) {
            color_selected = 2;
        } else if (temp_value_1 < 10) {
            color_selected = 3;
        } else {
            color_selected = 1;
        }
    }

    let mut temp_value: u64 = 0;

    temp_value = color_selected as u64
        | (color_selected.wrapping_shl(2) as u64)
        | ((color_selected as u64 | (color_selected.wrapping_shl(2) as u64)) << 4);

    temp_value = temp_value | (temp_value.wrapping_shl(8));
    temp_value = temp_value | (temp_value.wrapping_shl(16));
    let mut final_value: u64 = 0;
    final_value = value_color_1 as u64
        | (value_color_2.wrapping_shl(16) as u64)
        | (temp_value.wrapping_shl(32) as u64);
    let mut pixel_block_position: u32 = 0;

    while pixel_block_position < fullformat_data.pixel_blocks {
        let mut temp_code: u16 = 0;
        read_code(texture_huffmantree_dict, state_data, &mut temp_code)?;
        let mut value_data: u32 = 0;
        value_data = read_bits(state_data, 1)?;
        drop_bits(state_data, 1)?;

        while temp_code > 0 {
            if !color_bitmap[pixel_block_position as usize] {
                if value_data != 0 {
                    color_bitmap[pixel_block_position as usize] = true;
                    unimplemented!()
                }
                temp_code = temp_code.wrapping_sub(1);
            }
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }
        while pixel_block_position < fullformat_data.pixel_blocks
            && color_bitmap[pixel_block_position as usize]
        {
            pixel_block_position = pixel_block_position.wrapping_add(1);
        }
    }

    Ok(())
}

fn deduce_format(fourcc_data: u32, format_data: Vec<Format>) -> std::io::Result<Format> {
    let mut format_texture = Format::default();
    match fourcc_data {
        // DXT1
        0x31545844 => format_texture = format_data[0].clone(),
        // DXT2
        0x32545844 => format_texture = format_data[1].clone(),
        // DXT3
        0x33545844 => format_texture = format_data[2].clone(),
        // DXT4
        0x34545844 => format_texture = format_data[3].clone(),
        // DXT5
        0x35545844 => format_texture = format_data[4].clone(),
        // DXTA
        0x41545844 => format_texture = format_data[5].clone(),
        // DXTL
        0x4C545844 => format_texture = format_data[6].clone(),
        // DXTN
        0x4E545844 => format_texture = format_data[7].clone(),
        // 3DCX
        0x58434433 => format_texture = format_data[8].clone(),
        _ => println!("Format not found!"),
    }
    Ok(format_texture)
}

fn initialize_huffmantree_dict(huffmantree_data: &mut HuffmanTree) -> std::io::Result<bool> {
    let mut huffmantree_builder = HuffmanTreeBuilder::default();
    add_symbol(&mut huffmantree_builder, 0x01, 1)?;

    add_symbol(&mut huffmantree_builder, 0x12, 2)?;

    add_symbol(&mut huffmantree_builder, 0x11, 6)?;
    add_symbol(&mut huffmantree_builder, 0x10, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0F, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0E, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0D, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0C, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0B, 6)?;
    add_symbol(&mut huffmantree_builder, 0x0A, 6)?;
    add_symbol(&mut huffmantree_builder, 0x09, 6)?;
    add_symbol(&mut huffmantree_builder, 0x08, 6)?;
    add_symbol(&mut huffmantree_builder, 0x07, 6)?;
    add_symbol(&mut huffmantree_builder, 0x06, 6)?;
    add_symbol(&mut huffmantree_builder, 0x05, 6)?;
    add_symbol(&mut huffmantree_builder, 0x04, 6)?;
    add_symbol(&mut huffmantree_builder, 0x03, 6)?;
    add_symbol(&mut huffmantree_builder, 0x02, 6)?;

    if !build_huffmantree(huffmantree_data, &mut huffmantree_builder)? {
        return Ok(false);
    } else {
        Ok(true)
    }
}

fn add_symbol(
    huffmantree_builder: &mut HuffmanTreeBuilder,
    symbol_data: u16,
    bit_data: u8,
) -> std::io::Result<()> {
    if huffmantree_builder.bits_head_exist[bit_data as usize] {
        huffmantree_builder.bits_body[symbol_data as usize] =
            huffmantree_builder.bits_head[bit_data as usize];

        huffmantree_builder.bits_body_exist[symbol_data as usize] = true;

        huffmantree_builder.bits_head[bit_data as usize] = symbol_data;
    } else {
        huffmantree_builder.bits_head[bit_data as usize] = symbol_data;

        huffmantree_builder.bits_head_exist[bit_data as usize] = true;
    }
    Ok(())
}

fn check_bits_head(huffmantree_builder: &mut HuffmanTreeBuilder) -> std::io::Result<bool> {
    for head in huffmantree_builder.bits_head_exist {
        if head == true {
            return Ok(false);
        }
    }

    Ok(true)
}

fn build_huffmantree(
    huffmantree_data: &mut HuffmanTree,
    huffmantree_builder: &mut HuffmanTreeBuilder,
) -> std::io::Result<bool> {
    if check_bits_head(huffmantree_builder)? {
        return Ok(false);
    }
    *huffmantree_data = HuffmanTree::default();
    let mut temp_code: u32 = 0;
    let mut temp_bits: u8 = 0;

    // First part, filling hashTable for codes that are of less than 8 bits
    while temp_bits <= MAX_BITS_HASH as u8 {
        let mut data_exist: bool = huffmantree_builder.bits_head_exist[temp_bits as usize];

        if data_exist {
            let mut current_symbol: u16 = huffmantree_builder.bits_head[temp_bits as usize];

            while data_exist {
                // Processing hash values
                let mut hash_value: u16 = (temp_code << (MAX_BITS_HASH as u8 - temp_bits)) as u16;
                let next_hash_value: u16 =
                    ((temp_code.wrapping_add(1)) << (MAX_BITS_HASH as u8 - temp_bits)) as u16;

                while hash_value < next_hash_value {
                    huffmantree_data.symbol_value_hash_exist[hash_value as usize] = true;
                    huffmantree_data.symbol_value_hash[hash_value as usize] = current_symbol;
                    huffmantree_data.code_bits_hash[hash_value as usize] = temp_bits;
                    hash_value = hash_value.wrapping_add(1);
                }

                data_exist = huffmantree_builder.bits_body_exist[current_symbol as usize];
                current_symbol = huffmantree_builder.bits_body[current_symbol as usize];
                temp_code = temp_code.wrapping_sub(1);
            }
        }

        temp_code = (temp_code << 1) + 1;
        temp_bits = temp_bits.wrapping_add(1);
    }

    let mut temp_code_comparison_index: u16 = 0;
    let mut symbol_offset: u16 = 0;

    // Second part, filling classical structure for other codes
    while temp_bits < MAX_CODE_BITS_LENGTH as u8 {
        let mut data_exist: bool = huffmantree_builder.bits_head_exist[temp_bits as usize];

        if data_exist {
            let mut current_symbol: u16 = huffmantree_builder.bits_head[temp_bits as usize];

            while data_exist {
                // Registering the code
                huffmantree_data.symbol_value[symbol_offset as usize] = current_symbol;

                symbol_offset = symbol_offset.wrapping_add(1);
                data_exist = huffmantree_builder.bits_body_exist[current_symbol as usize];
                current_symbol = huffmantree_builder.bits_body[current_symbol as usize];

                temp_code = temp_code.wrapping_sub(1);
            }

            // Minimum code value for temp_bits bits
            huffmantree_data.code_comparison[temp_code_comparison_index as usize] =
                temp_code.wrapping_add(1) << (32 - temp_bits);

            // Number of bits for l_codeCompIndex index
            huffmantree_data.code_bits[temp_code_comparison_index as usize] = temp_bits;

            // Offset in symbol_value table to reach the value
            huffmantree_data.symbol_value_offset[temp_code_comparison_index as usize] =
                symbol_offset.wrapping_sub(1);

            temp_code_comparison_index = temp_code_comparison_index.wrapping_add(1);
        }

        temp_code = (temp_code << 1) + 1;
        temp_bits = temp_bits.wrapping_add(1);
    }

    Ok(true)
}
