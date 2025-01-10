use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Seek};

const MAX_BITS_HASH: usize = 8;
const MAX_CODE_BITS_LENGTH: usize = 32;
const MAX_SYMBOL_VALUE: usize = 285;
const HALF_BYTE: u8 = 4;
const U8_IN_BITS: u8 = 8;
const U16_IN_BITS: u8 = 16;
const U32_IN_BITS: u8 = 32;

#[derive(Debug, Default)]
struct StateData {
    input_buffer: Cursor<Vec<u8>>,
    buffer_position_bytes: u64,
    bytes_available: u32,
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
    let index_num = read_bits(state_data, U8_IN_BITS as u8)? as usize;

    let exist = huffmantree_data.symbol_value_hash_exist[index_num];

    if exist {
        *symbol_data =
            huffmantree_data.symbol_value_hash[read_bits(state_data, U8_IN_BITS as u8)? as usize];

        let code_bits_hash =
            huffmantree_data.code_bits_hash[read_bits(state_data, U8_IN_BITS as u8)? as usize];

        drop_bits(state_data, code_bits_hash)?;
    } else {
        let mut index_data: u16 = 0;
        while read_bits(state_data, U32_IN_BITS)?
            < huffmantree_data.code_comparison[index_data as usize]
        {
            index_data = index_data.wrapping_add(1);
        }

        let temp_bits: u8 = huffmantree_data.code_bits[index_data as usize];

        // Step 1: Read 32 bits from state_data
        let read_bits_value = read_bits(state_data, U32_IN_BITS)?;

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
    let mut head_data: u32 = 0;
    let mut bytes_available_data: u8 = 0;

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
    drop_bits(state_data, HALF_BYTE)?;
    write_size_const_addition = read_bits(state_data, HALF_BYTE)? as u16;
    write_size_const_addition += 1;
    drop_bits(state_data, HALF_BYTE)?;

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
        max_count = read_bits(state_data, HALF_BYTE)?;
        max_count = (max_count + 1) << 12;
        max_size_count = max_size_count + 1;
        drop_bits(state_data, HALF_BYTE)?;

        let mut current_code_read_count: u32 = 0;
        while (current_code_read_count < max_count) && (output_position < *output_data_size) {
            if state_data.input_buffer.stream_len()? == state_data.buffer_position_bytes {
                break;
            }

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

        if state_data.input_buffer.stream_len()? == state_data.buffer_position_bytes {
            break;
        }
    }
    Ok(())
}

fn initialize_huffmantree_dict(huffmantree_data: &mut HuffmanTree) -> std::io::Result<bool> {
    let mut huffmantree_builder = HuffmanTreeBuilder::default();

    let bits_data: [u8; 256] = [
        3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8,
        8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10,
        10, 10, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 13,
        13, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
        16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    ];

    let symbols_data: [u16; 256] = [
        0x0A, 0x09, 0x08, 0x0C, 0x0B, 0x07, 0x00, 0xE0, 0x2A, 0x29, 0x06, 0x4A, 0x40, 0x2C, 0x2B,
        0x28, 0x20, 0x05, 0x04, 0x49, 0x48, 0x27, 0x26, 0x25, 0x0D, 0x03, 0x6A, 0x69, 0x4C, 0x4B,
        0x47, 0x24, 0xE8, 0xA0, 0x89, 0x88, 0x68, 0x67, 0x63, 0x60, 0x46, 0x23, 0xE9, 0xC9, 0xC0,
        0xA9, 0xA8, 0x8A, 0x87, 0x80, 0x66, 0x65, 0x45, 0x44, 0x43, 0x2D, 0x02, 0x01, 0xE5, 0xC8,
        0xAA, 0xA5, 0xA4, 0x8B, 0x85, 0x84, 0x6C, 0x6B, 0x64, 0x4D, 0x0E, 0xE7, 0xCA, 0xC7, 0xA7,
        0xA6, 0x86, 0x83, 0xE6, 0xE4, 0xC4, 0x8C, 0x2E, 0x22, 0xEC, 0xC6, 0x6D, 0x4E, 0xEA, 0xCC,
        0xAC, 0xAB, 0x8D, 0x11, 0x10, 0x0F, 0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8, 0xF7,
        0xF6, 0xF5, 0xF4, 0xF3, 0xF2, 0xF1, 0xF0, 0xEF, 0xEE, 0xED, 0xEB, 0xE3, 0xE2, 0xE1, 0xDF,
        0xDE, 0xDD, 0xDC, 0xDB, 0xDA, 0xD9, 0xD8, 0xD7, 0xD6, 0xD5, 0xD4, 0xD3, 0xD2, 0xD1, 0xD0,
        0xCF, 0xCE, 0xCD, 0xCB, 0xC5, 0xC3, 0xC2, 0xC1, 0xBF, 0xBE, 0xBD, 0xBC, 0xBB, 0xBA, 0xB9,
        0xB8, 0xB7, 0xB6, 0xB5, 0xB4, 0xB3, 0xB2, 0xB1, 0xB0, 0xAF, 0xAE, 0xAD, 0xA3, 0xA2, 0xA1,
        0x9F, 0x9E, 0x9D, 0x9C, 0x9B, 0x9A, 0x99, 0x98, 0x97, 0x96, 0x95, 0x94, 0x93, 0x92, 0x91,
        0x90, 0x8F, 0x8E, 0x82, 0x81, 0x7F, 0x7E, 0x7D, 0x7C, 0x7B, 0x7A, 0x79, 0x78, 0x77, 0x76,
        0x75, 0x74, 0x73, 0x72, 0x71, 0x70, 0x6F, 0x6E, 0x62, 0x61, 0x5F, 0x5E, 0x5D, 0x5C, 0x5B,
        0x5A, 0x59, 0x58, 0x57, 0x56, 0x55, 0x54, 0x53, 0x52, 0x51, 0x50, 0x4F, 0x42, 0x41, 0x3F,
        0x3E, 0x3D, 0x3C, 0x3B, 0x3A, 0x39, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x30,
        0x2F, 0x21, 0x1F, 0x1E, 0x1D, 0x1C, 0x1B, 0x1A, 0x19, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
        0x12,
    ];

    for index in 0..256 {
        add_symbol(
            &mut huffmantree_builder,
            symbols_data[index],
            bits_data[index],
        )?;
    }

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
    symbol_number = read_bits(state_data, U16_IN_BITS)? as u16;
    drop_bits(state_data, U16_IN_BITS)?;
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
