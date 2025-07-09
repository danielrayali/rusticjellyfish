use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 4 {
        eprintln!("Usage: {} <input_binary> <output_binary> <mapping_file>", args[0]);
        eprintln!("Example: {} input.bin output.bin mappings.txt", args[0]);
        process::exit(1);
    }
    
    let input_path = &args[1];
    let output_path = &args[2];
    let mapping_path = &args[3];
    
    // Read the binary file
    let mut data = match fs::read(input_path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error reading input file '{}': {}", input_path, e);
            process::exit(1);
        }
    };
    
    // Read the mapping file
    let mappings = match read_mappings(mapping_path) {
        Ok(mappings) => mappings,
        Err(e) => {
            eprintln!("Error reading mapping file '{}': {}", mapping_path, e);
            process::exit(1);
        }
    };
    
    // Perform replacements
    let mut replacements_made = 0;
    for (search, replace) in mappings {
        let count = replace_bytes_in_data(&mut data, &search, &replace);
        if count > 0 {
            println!("Replaced '{}' with '{}' ({} occurrences)", 
                     String::from_utf8_lossy(&search), 
                     String::from_utf8_lossy(&replace), 
                     count);
            replacements_made += count;
        }
    }
    
    // Write the modified data to output file
    if let Err(e) = fs::write(output_path, &data) {
        eprintln!("Error writing output file '{}': {}", output_path, e);
        process::exit(1);
    }
    
    println!("Successfully processed file. Total replacements made: {}", replacements_made);
}

fn read_mappings(path: &str) -> io::Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut mappings = Vec::new();
    
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse line in format: "search_string" -> "replace_string"
        if let Some(arrow_pos) = line.find("->") {
            let search_part = line[..arrow_pos].trim();
            let replace_part = line[arrow_pos + 2..].trim();
            
            let search_bytes = parse_string_literal(search_part)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, 
                    format!("Line {}: Error parsing search string: {}", line_num + 1, e)))?;
            
            let replace_bytes = parse_string_literal(replace_part)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, 
                    format!("Line {}: Error parsing replace string: {}", line_num + 1, e)))?;
            
            if search_bytes.len() != replace_bytes.len() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, 
                    format!("Line {}: Search and replace strings must have the same length. '{}' ({} bytes) vs '{}' ({} bytes)", 
                        line_num + 1,
                        String::from_utf8_lossy(&search_bytes), search_bytes.len(),
                        String::from_utf8_lossy(&replace_bytes), replace_bytes.len())));
            }
            
            mappings.push((search_bytes, replace_bytes));
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, 
                format!("Line {}: Invalid format. Expected 'search' -> 'replace'", line_num + 1)));
        }
    }
    
    Ok(mappings)
}

fn parse_string_literal(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    
    // Handle quoted strings
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        let inner = &s[1..s.len()-1];
        return Ok(unescape_string(inner)?);
    }
    
    // Handle hex strings (format: 0x48656c6c6f or 48656c6c6f)
    if s.starts_with("0x") || s.chars().all(|c| c.is_ascii_hexdigit()) {
        let hex_str = if s.starts_with("0x") { &s[2..] } else { s };
        if hex_str.len() % 2 != 0 {
            return Err("Hex string must have even number of characters".to_string());
        }
        
        let mut bytes = Vec::new();
        for chunk in hex_str.chars().collect::<Vec<_>>().chunks(2) {
            let hex_byte: String = chunk.iter().collect();
            match u8::from_str_radix(&hex_byte, 16) {
                Ok(byte) => bytes.push(byte),
                Err(_) => return Err(format!("Invalid hex byte: {}", hex_byte)),
            }
        }
        return Ok(bytes);
    }
    
    // Default: treat as UTF-8 string
    Ok(s.as_bytes().to_vec())
}

fn unescape_string(s: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push(b'\n'),
                Some('r') => result.push(b'\r'),
                Some('t') => result.push(b'\t'),
                Some('\\') => result.push(b'\\'),
                Some('"') => result.push(b'"'),
                Some('\'') => result.push(b'\''),
                Some('0') => result.push(b'\0'),
                Some(other) => return Err(format!("Unknown escape sequence: \\{}", other)),
                None => return Err("Incomplete escape sequence".to_string()),
            }
        } else {
            let mut buf = [0; 4];
            let bytes = ch.encode_utf8(&mut buf).as_bytes();
            result.extend_from_slice(bytes);
        }
    }
    
    Ok(result)
}

fn replace_bytes_in_data(data: &mut Vec<u8>, search: &[u8], replace: &[u8]) -> usize {
    if search.is_empty() || search.len() != replace.len() {
        return 0;
    }
    
    let mut count = 0;
    let mut i = 0;
    
    while i <= data.len().saturating_sub(search.len()) {
        if data[i..i + search.len()] == *search {
            // Replace the bytes
            data[i..i + search.len()].copy_from_slice(replace);
            count += 1;
            i += search.len(); // Skip past the replacement to avoid overlapping matches
        } else {
            i += 1;
        }
    }
    
    count
}