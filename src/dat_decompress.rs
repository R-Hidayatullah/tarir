#![allow(dead_code)]

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Seek};

const MAX_BITS_HASH: usize = 8;
const MAX_CODE_BITS_LENGTH: usize = 32;
const MAX_SYMBOL_VALUE: usize = 285;

// const SKIPPED_BYTES_PER_CHUNK: usize = 65536; // its using CRC-32C in hxd editor
// CRC-32C (Cyclic Redundancy Check 32-bit Castagnoli) is a variant of the CRC-32 algorithm that uses the Castagnoli polynomial.
// its in each SKIPPED_BYTES_PER_CHUNK-4 until SKIPPED_BYTES_PER_CHUNK and 4 bytes before the end of chunk
const SKIPPED_BYTES_PER_CHUNK: usize = 0;

const BYTES_TO_REMOVE: usize = 4; // sizeof(u32)

#[derive(Debug, Default)]
struct StateData {
    input_buffer: Cursor<Vec<u8>>,
    buffer_position_bytes: u64,
    buffer_position_bit: u64,
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

fn pull_byte(
    state_data: &mut StateData,
    head_data: &mut u32,
    bytes_available_data: &mut u8,
) -> std::io::Result<()> {
    if state_data.bytes_available >= std::mem::size_of::<u32>() as u32 {
        *head_data = state_data.input_buffer.read_u32::<LittleEndian>()?;
        state_data.bytes_available -= std::mem::size_of::<u32>() as u32;
        state_data.buffer_position_bytes = state_data.input_buffer.position();
        state_data.buffer_position_bit = state_data.buffer_position_bit + 32;
        *bytes_available_data = (std::mem::size_of::<u32>() as u32 * 8) as u8;
    } else {
        *head_data = 0;
        *bytes_available_data = 0;
    }
    Ok(())
}

fn read_bits(state_data: &mut StateData, bits_number: u8) -> std::io::Result<u32> {
    // Extract the available bits
    let mut value = state_data.head_data >> (std::mem::size_of::<u32>() as u8 * 8 - bits_number);

    if state_data.bytes_available_data < bits_number {
        println!(
            "Not enough bits available to read the value. In position: {}",
            state_data.input_buffer.position()
        );

        // If the number of bits is less than 32, pad with zeros
        if bits_number < 32 {
            let padding_bits = 32 - bits_number;
            value <<= padding_bits; // Shift the value to the left, adding zeros
        }
    }

    Ok(value)
}

fn drop_bits(state_data: &mut StateData, bits_number: u8) -> std::io::Result<()> {
    if state_data.bytes_available_data < bits_number {
        println!("Too much bits were asked to be dropped.");
    }
    #[allow(unused_assignments)]
    let mut new_bits_available: u8 = 0;
    state_data.buffer_position_bit = state_data.buffer_position_bit + bits_number as u64;
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
pub fn inflate_dat_file_buffer(
    input_data: Vec<u8>,
    output_data_size: &mut u32,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut state_data = StateData::default();
    state_data.bytes_available = input_data.len() as u32;
    state_data.input_buffer = Cursor::new(input_data);
    state_data.skipped_bytes = SKIPPED_BYTES_PER_CHUNK as u32;
    let mut head_data: u32 = 0;
    let mut bytes_available_data: u8 = 0;
    // println!("Buffer position : {}", state_data.input_buffer.position());

    pull_byte(&mut state_data, &mut head_data, &mut bytes_available_data)?;

    state_data.head_data = head_data;
    state_data.bytes_available_data = bytes_available_data;

    drop_bits(&mut state_data, 32)?;

    *output_data_size = read_bits(&mut state_data, 32)?;

    drop_bits(&mut state_data, 32)?;

    output_data.resize(*output_data_size as usize, 0);

    inflate_data(&mut state_data, output_data_size, output_data)?;
    Ok(())
}

fn inflate_data(
    state_data: &mut StateData,
    output_data_size: &mut u32,
    output_data: &mut Vec<u8>,
) -> std::io::Result<()> {
    let mut output_position: u32 = 0;
    #[allow(unused_assignments)]
    let mut write_size_const_addition: u16 = 0;
    let mut max_size_count: u32 = 0;
    drop_bits(state_data, 4)?;
    write_size_const_addition = read_bits(state_data, 4)? as u16;
    write_size_const_addition += 1;
    drop_bits(state_data, 4)?;

    let mut dat_file_huffmantree_dict = HuffmanTree::default();
    let mut huffmantree_copy = HuffmanTree::default();
    let mut huffmantree_symbol = HuffmanTree::default();
    if !initialize_huffmantree_dict(&mut dat_file_huffmantree_dict)? {
        println!("Failed to initialize huffmantree dict!");
    }

    let mut huffmantree_builder = HuffmanTreeBuilder::default();

    while output_position < *output_data_size {
        if !parse_huffmantree(
            state_data,
            &mut huffmantree_symbol,
            &mut dat_file_huffmantree_dict,
            &mut huffmantree_builder,
        )? || !parse_huffmantree(
            state_data,
            &mut huffmantree_copy,
            &mut dat_file_huffmantree_dict,
            &mut huffmantree_builder,
        )? {
            println!("Failed to parse huffmantree.");
            break;
        }

        #[allow(unused_assignments)]
        let mut max_count: u32 = 0;
        max_count = read_bits(state_data, 4)?;
        max_count = (max_count + 1) << 12;
        max_size_count = max_size_count + 1;
        drop_bits(state_data, 4)?; // Because this dropping using half byte make the read bits not enough data

        let mut current_code_read_count: u32 = 0;
        while (current_code_read_count < max_count) && (output_position < *output_data_size) {
            current_code_read_count = current_code_read_count.wrapping_add(1);
            let mut symbol_data = 0;
            read_code(&mut huffmantree_symbol, state_data, &mut symbol_data)?;

            if symbol_data < 0x100 {
                let index_num = output_position as usize;

                output_data[index_num] = symbol_data as u8;

                output_position = output_position.wrapping_add(1);
                continue;
            }
            symbol_data = symbol_data.wrapping_sub(0x100);
            // Write size
            let temp_code_div4_quot = symbol_data / 4;
            let temp_code_div4_rem = symbol_data % 4;

            let mut write_size: u32 = 0;

            if temp_code_div4_quot == 0 {
                write_size = symbol_data as u32
            } else if temp_code_div4_quot < 7 {
                write_size =
                    (1 << (temp_code_div4_quot.wrapping_sub(1))) * (4 + temp_code_div4_rem) as u32
            } else if symbol_data == 28 {
                write_size = 0xFF
            } else {
                println!("Invalid value for write_size code.");
            }

            if temp_code_div4_quot > 1 && symbol_data != 28 {
                let write_size_add_bits: u8 = temp_code_div4_quot.wrapping_sub(1) as u8;
                #[allow(unused_assignments)]
                let mut write_size_add: u32 = 0;
                write_size_add = read_bits(state_data, write_size_add_bits)?;
                write_size |= write_size_add;
                drop_bits(state_data, write_size_add_bits)?;
            }

            write_size = write_size.wrapping_add(write_size_const_addition as u32);

            read_code(&mut huffmantree_copy, state_data, &mut symbol_data)?;
            let temp_code_div2_quot = symbol_data / 2;
            let temp_code_div2_rem = symbol_data % 2;

            let mut write_offset: u32 = 0;

            if temp_code_div2_quot == 0 {
                write_offset = symbol_data as u32
            } else if temp_code_div2_quot < 17 {
                write_offset =
                    (1 << (temp_code_div2_quot.wrapping_sub(1))) * (2 + temp_code_div2_rem) as u32
            } else {
                println!("Invalid value for writeOffset code.");
            }

            if temp_code_div2_quot > 1 {
                let write_offset_add_bits: u8 = temp_code_div2_quot.wrapping_sub(1) as u8;
                #[allow(unused_assignments)]
                let mut write_offset_add: u32 = 0;
                write_offset_add = read_bits(state_data, write_offset_add_bits)?;
                write_offset |= write_offset_add;
                drop_bits(state_data, write_offset_add_bits)?;
            }

            write_offset = write_offset.wrapping_add(1);

            let mut already_written: u32 = 0;
            while (already_written < write_size) && (output_position < *output_data_size) {
                output_data[output_position as usize] =
                    output_data[(output_position - write_offset) as usize];
                output_position = output_position.wrapping_add(1);
                already_written = already_written.wrapping_add(1);
            }
        }
    }
    println!(
        "Max size count : {}, read bits : {}, read bytes : {} ",
        max_size_count,
        max_size_count * 4,
        (max_size_count * 4) / 8
    );
    println!(
        "Buffer bits position : {}, Buffer bits size left before EOF : {},",
        state_data.buffer_position_bit,
        if (state_data.input_buffer.stream_len()? * 16) > state_data.buffer_position_bit {
            (state_data.input_buffer.stream_len()? * 16)
                .wrapping_sub(state_data.buffer_position_bit)
        } else {
            0
        }
    );
    Ok(())
}

fn initialize_huffmantree_dict(huffmantree_data: &mut HuffmanTree) -> std::io::Result<bool> {
    let mut huffmantree_builder = HuffmanTreeBuilder::default();

    add_symbol(&mut huffmantree_builder, 0x0A, 3)?;
    add_symbol(&mut huffmantree_builder, 0x09, 3)?;
    add_symbol(&mut huffmantree_builder, 0x08, 3)?;

    add_symbol(&mut huffmantree_builder, 0x0C, 4)?;
    add_symbol(&mut huffmantree_builder, 0x0B, 4)?;
    add_symbol(&mut huffmantree_builder, 0x07, 4)?;
    add_symbol(&mut huffmantree_builder, 0x00, 4)?;

    add_symbol(&mut huffmantree_builder, 0xE0, 5)?;
    add_symbol(&mut huffmantree_builder, 0x2A, 5)?;
    add_symbol(&mut huffmantree_builder, 0x29, 5)?;
    add_symbol(&mut huffmantree_builder, 0x06, 5)?;

    add_symbol(&mut huffmantree_builder, 0x4A, 6)?;
    add_symbol(&mut huffmantree_builder, 0x40, 6)?;
    add_symbol(&mut huffmantree_builder, 0x2C, 6)?;
    add_symbol(&mut huffmantree_builder, 0x2B, 6)?;
    add_symbol(&mut huffmantree_builder, 0x28, 6)?;
    add_symbol(&mut huffmantree_builder, 0x20, 6)?;
    add_symbol(&mut huffmantree_builder, 0x05, 6)?;
    add_symbol(&mut huffmantree_builder, 0x04, 6)?;

    add_symbol(&mut huffmantree_builder, 0x49, 7)?;
    add_symbol(&mut huffmantree_builder, 0x48, 7)?;
    add_symbol(&mut huffmantree_builder, 0x27, 7)?;
    add_symbol(&mut huffmantree_builder, 0x26, 7)?;
    add_symbol(&mut huffmantree_builder, 0x25, 7)?;
    add_symbol(&mut huffmantree_builder, 0x0D, 7)?;
    add_symbol(&mut huffmantree_builder, 0x03, 7)?;

    add_symbol(&mut huffmantree_builder, 0x6A, 8)?;
    add_symbol(&mut huffmantree_builder, 0x69, 8)?;
    add_symbol(&mut huffmantree_builder, 0x4C, 8)?;
    add_symbol(&mut huffmantree_builder, 0x4B, 8)?;
    add_symbol(&mut huffmantree_builder, 0x47, 8)?;
    add_symbol(&mut huffmantree_builder, 0x24, 8)?;

    add_symbol(&mut huffmantree_builder, 0xE8, 9)?;
    add_symbol(&mut huffmantree_builder, 0xA0, 9)?;
    add_symbol(&mut huffmantree_builder, 0x89, 9)?;
    add_symbol(&mut huffmantree_builder, 0x88, 9)?;
    add_symbol(&mut huffmantree_builder, 0x68, 9)?;
    add_symbol(&mut huffmantree_builder, 0x67, 9)?;
    add_symbol(&mut huffmantree_builder, 0x63, 9)?;
    add_symbol(&mut huffmantree_builder, 0x60, 9)?;
    add_symbol(&mut huffmantree_builder, 0x46, 9)?;
    add_symbol(&mut huffmantree_builder, 0x23, 9)?;

    add_symbol(&mut huffmantree_builder, 0xE9, 10)?;
    add_symbol(&mut huffmantree_builder, 0xC9, 10)?;
    add_symbol(&mut huffmantree_builder, 0xC0, 10)?;
    add_symbol(&mut huffmantree_builder, 0xA9, 10)?;
    add_symbol(&mut huffmantree_builder, 0xA8, 10)?;
    add_symbol(&mut huffmantree_builder, 0x8A, 10)?;
    add_symbol(&mut huffmantree_builder, 0x87, 10)?;
    add_symbol(&mut huffmantree_builder, 0x80, 10)?;
    add_symbol(&mut huffmantree_builder, 0x66, 10)?;
    add_symbol(&mut huffmantree_builder, 0x65, 10)?;
    add_symbol(&mut huffmantree_builder, 0x45, 10)?;
    add_symbol(&mut huffmantree_builder, 0x44, 10)?;
    add_symbol(&mut huffmantree_builder, 0x43, 10)?;
    add_symbol(&mut huffmantree_builder, 0x2D, 10)?;
    add_symbol(&mut huffmantree_builder, 0x02, 10)?;
    add_symbol(&mut huffmantree_builder, 0x01, 10)?;

    add_symbol(&mut huffmantree_builder, 0xE5, 11)?;
    add_symbol(&mut huffmantree_builder, 0xC8, 11)?;
    add_symbol(&mut huffmantree_builder, 0xAA, 11)?;
    add_symbol(&mut huffmantree_builder, 0xA5, 11)?;
    add_symbol(&mut huffmantree_builder, 0xA4, 11)?;
    add_symbol(&mut huffmantree_builder, 0x8B, 11)?;
    add_symbol(&mut huffmantree_builder, 0x85, 11)?;
    add_symbol(&mut huffmantree_builder, 0x84, 11)?;
    add_symbol(&mut huffmantree_builder, 0x6C, 11)?;
    add_symbol(&mut huffmantree_builder, 0x6B, 11)?;
    add_symbol(&mut huffmantree_builder, 0x64, 11)?;
    add_symbol(&mut huffmantree_builder, 0x4D, 11)?;
    add_symbol(&mut huffmantree_builder, 0x0E, 11)?;

    add_symbol(&mut huffmantree_builder, 0xE7, 12)?;
    add_symbol(&mut huffmantree_builder, 0xCA, 12)?;
    add_symbol(&mut huffmantree_builder, 0xC7, 12)?;
    add_symbol(&mut huffmantree_builder, 0xA7, 12)?;
    add_symbol(&mut huffmantree_builder, 0xA6, 12)?;
    add_symbol(&mut huffmantree_builder, 0x86, 12)?;
    add_symbol(&mut huffmantree_builder, 0x83, 12)?;

    add_symbol(&mut huffmantree_builder, 0xE6, 13)?;
    add_symbol(&mut huffmantree_builder, 0xE4, 13)?;
    add_symbol(&mut huffmantree_builder, 0xC4, 13)?;
    add_symbol(&mut huffmantree_builder, 0x8C, 13)?;
    add_symbol(&mut huffmantree_builder, 0x2E, 13)?;
    add_symbol(&mut huffmantree_builder, 0x22, 13)?;

    add_symbol(&mut huffmantree_builder, 0xEC, 14)?;
    add_symbol(&mut huffmantree_builder, 0xC6, 14)?;
    add_symbol(&mut huffmantree_builder, 0x6D, 14)?;
    add_symbol(&mut huffmantree_builder, 0x4E, 14)?;

    add_symbol(&mut huffmantree_builder, 0xEA, 15)?;
    add_symbol(&mut huffmantree_builder, 0xCC, 15)?;
    add_symbol(&mut huffmantree_builder, 0xAC, 15)?;
    add_symbol(&mut huffmantree_builder, 0xAB, 15)?;
    add_symbol(&mut huffmantree_builder, 0x8D, 15)?;
    add_symbol(&mut huffmantree_builder, 0x11, 15)?;
    add_symbol(&mut huffmantree_builder, 0x10, 15)?;
    add_symbol(&mut huffmantree_builder, 0x0F, 15)?;

    add_symbol(&mut huffmantree_builder, 0xFF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xFE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xFD, 16)?;
    add_symbol(&mut huffmantree_builder, 0xFC, 16)?;
    add_symbol(&mut huffmantree_builder, 0xFB, 16)?;
    add_symbol(&mut huffmantree_builder, 0xFA, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF9, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF8, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF7, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF6, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF5, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF4, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF1, 16)?;
    add_symbol(&mut huffmantree_builder, 0xF0, 16)?;
    add_symbol(&mut huffmantree_builder, 0xEF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xEE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xED, 16)?;
    add_symbol(&mut huffmantree_builder, 0xEB, 16)?;
    add_symbol(&mut huffmantree_builder, 0xE3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xE2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xE1, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDD, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDC, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDB, 16)?;
    add_symbol(&mut huffmantree_builder, 0xDA, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD9, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD8, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD7, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD6, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD5, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD4, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD1, 16)?;
    add_symbol(&mut huffmantree_builder, 0xD0, 16)?;
    add_symbol(&mut huffmantree_builder, 0xCF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xCE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xCD, 16)?;
    add_symbol(&mut huffmantree_builder, 0xCB, 16)?;
    add_symbol(&mut huffmantree_builder, 0xC5, 16)?;
    add_symbol(&mut huffmantree_builder, 0xC3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xC2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xC1, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBD, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBC, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBB, 16)?;
    add_symbol(&mut huffmantree_builder, 0xBA, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB9, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB8, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB7, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB6, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB5, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB4, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB1, 16)?;
    add_symbol(&mut huffmantree_builder, 0xB0, 16)?;
    add_symbol(&mut huffmantree_builder, 0xAF, 16)?;
    add_symbol(&mut huffmantree_builder, 0xAE, 16)?;
    add_symbol(&mut huffmantree_builder, 0xAD, 16)?;
    add_symbol(&mut huffmantree_builder, 0xA3, 16)?;
    add_symbol(&mut huffmantree_builder, 0xA2, 16)?;
    add_symbol(&mut huffmantree_builder, 0xA1, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9D, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9C, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9B, 16)?;
    add_symbol(&mut huffmantree_builder, 0x9A, 16)?;
    add_symbol(&mut huffmantree_builder, 0x99, 16)?;
    add_symbol(&mut huffmantree_builder, 0x98, 16)?;
    add_symbol(&mut huffmantree_builder, 0x97, 16)?;
    add_symbol(&mut huffmantree_builder, 0x96, 16)?;
    add_symbol(&mut huffmantree_builder, 0x95, 16)?;
    add_symbol(&mut huffmantree_builder, 0x94, 16)?;
    add_symbol(&mut huffmantree_builder, 0x93, 16)?;
    add_symbol(&mut huffmantree_builder, 0x92, 16)?;
    add_symbol(&mut huffmantree_builder, 0x91, 16)?;
    add_symbol(&mut huffmantree_builder, 0x90, 16)?;
    add_symbol(&mut huffmantree_builder, 0x8F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x8E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x82, 16)?;
    add_symbol(&mut huffmantree_builder, 0x81, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7D, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7C, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7B, 16)?;
    add_symbol(&mut huffmantree_builder, 0x7A, 16)?;
    add_symbol(&mut huffmantree_builder, 0x79, 16)?;
    add_symbol(&mut huffmantree_builder, 0x78, 16)?;
    add_symbol(&mut huffmantree_builder, 0x77, 16)?;
    add_symbol(&mut huffmantree_builder, 0x76, 16)?;
    add_symbol(&mut huffmantree_builder, 0x75, 16)?;
    add_symbol(&mut huffmantree_builder, 0x74, 16)?;
    add_symbol(&mut huffmantree_builder, 0x73, 16)?;
    add_symbol(&mut huffmantree_builder, 0x72, 16)?;
    add_symbol(&mut huffmantree_builder, 0x71, 16)?;
    add_symbol(&mut huffmantree_builder, 0x70, 16)?;
    add_symbol(&mut huffmantree_builder, 0x6F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x6E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x62, 16)?;
    add_symbol(&mut huffmantree_builder, 0x61, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5D, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5C, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5B, 16)?;
    add_symbol(&mut huffmantree_builder, 0x5A, 16)?;
    add_symbol(&mut huffmantree_builder, 0x59, 16)?;
    add_symbol(&mut huffmantree_builder, 0x58, 16)?;
    add_symbol(&mut huffmantree_builder, 0x57, 16)?;
    add_symbol(&mut huffmantree_builder, 0x56, 16)?;
    add_symbol(&mut huffmantree_builder, 0x55, 16)?;
    add_symbol(&mut huffmantree_builder, 0x54, 16)?;
    add_symbol(&mut huffmantree_builder, 0x53, 16)?;
    add_symbol(&mut huffmantree_builder, 0x52, 16)?;
    add_symbol(&mut huffmantree_builder, 0x51, 16)?;
    add_symbol(&mut huffmantree_builder, 0x50, 16)?;
    add_symbol(&mut huffmantree_builder, 0x4F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x42, 16)?;
    add_symbol(&mut huffmantree_builder, 0x41, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3D, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3C, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3B, 16)?;
    add_symbol(&mut huffmantree_builder, 0x3A, 16)?;
    add_symbol(&mut huffmantree_builder, 0x39, 16)?;
    add_symbol(&mut huffmantree_builder, 0x38, 16)?;
    add_symbol(&mut huffmantree_builder, 0x37, 16)?;
    add_symbol(&mut huffmantree_builder, 0x36, 16)?;
    add_symbol(&mut huffmantree_builder, 0x35, 16)?;
    add_symbol(&mut huffmantree_builder, 0x34, 16)?;
    add_symbol(&mut huffmantree_builder, 0x33, 16)?;
    add_symbol(&mut huffmantree_builder, 0x32, 16)?;
    add_symbol(&mut huffmantree_builder, 0x31, 16)?;
    add_symbol(&mut huffmantree_builder, 0x30, 16)?;
    add_symbol(&mut huffmantree_builder, 0x2F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x21, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1F, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1E, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1D, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1C, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1B, 16)?;
    add_symbol(&mut huffmantree_builder, 0x1A, 16)?;
    add_symbol(&mut huffmantree_builder, 0x19, 16)?;
    add_symbol(&mut huffmantree_builder, 0x18, 16)?;
    add_symbol(&mut huffmantree_builder, 0x17, 16)?;
    add_symbol(&mut huffmantree_builder, 0x16, 16)?;
    add_symbol(&mut huffmantree_builder, 0x15, 16)?;
    add_symbol(&mut huffmantree_builder, 0x14, 16)?;
    add_symbol(&mut huffmantree_builder, 0x13, 16)?;
    add_symbol(&mut huffmantree_builder, 0x12, 16)?;

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

fn parse_huffmantree(
    state_data: &mut StateData,
    huffmantree_data: &mut HuffmanTree,
    dat_file_huffmantree_dict: &mut HuffmanTree,
    huffmantree_builder: &mut HuffmanTreeBuilder,
) -> std::io::Result<bool> {
    #[allow(unused_assignments)]
    let mut symbol_number: u16 = 0;
    symbol_number = read_bits(state_data, 16)? as u16;
    drop_bits(state_data, 16)?;
    if symbol_number > MAX_SYMBOL_VALUE as u16 {
        println!("Too many symbols to decode.");
    }
    *huffmantree_builder = HuffmanTreeBuilder::default();
    let mut remaining_symbol: i16 = symbol_number.wrapping_sub(1) as i16;
    while remaining_symbol >= 0 {
        let mut temp_code: u16 = 0;
        read_code(dat_file_huffmantree_dict, state_data, &mut temp_code)?;
        let temp_code_number_bits: u8 = (temp_code & 0x1F) as u8;
        let mut temp_code_number_symbol: u16 = (temp_code >> 5) + 1;

        if temp_code_number_bits == 0 {
            remaining_symbol = remaining_symbol.wrapping_sub(temp_code_number_symbol as i16);
        } else {
            while temp_code_number_symbol > 0 {
                add_symbol(
                    huffmantree_builder,
                    remaining_symbol as u16,
                    temp_code_number_bits,
                )?;

                remaining_symbol = remaining_symbol.wrapping_sub(1);
                temp_code_number_symbol = temp_code_number_symbol.wrapping_sub(1);
            }
        }
    }
    Ok(build_huffmantree(huffmantree_data, huffmantree_builder)?)
}
