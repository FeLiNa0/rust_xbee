// example: Consider 7E 00 08 08 01 4E 49 58 42 45 45 3B
// send AT ND command to coordinator: 0x7E 0x00 0x04 0x08 0x01 0x4E 0x44 0x64
//
const START_DELIMITER: u8 = 0x7E;
const TRANSMIT_REQUEST: u8 = 0x10;
const TRANSMIT_STATUS: u8 = 0x8B;
const LOCAL_AT_COMMAND_TYPE: u8 = 0x08;
const REMOTE_COMMAND_REQUEST_TYPE: u8 = 0x17;
const AT_COMMAND_RESPONSE_TYPE: u8 = 0x88;
const REMOTE_COMMAND_RESPONSE: u8 = 0x97;

pub fn local_at_command(
    command: &[u8],
    parameter: Option<&[u8]>,
    frame_id: Option<u8>,
) -> Result<Vec<u8>, String> {
    if command.len() != 2 {
        return Err("command must be of length 2".into());
    }
    let fid = frame_id.unwrap_or(0x42);
    let mut content = vec![LOCAL_AT_COMMAND_TYPE];
    content.push(fid);
    content.extend(command);
    match parameter {
        Some(bytes) => content.extend(bytes),
        _ => {}
    }
    make_api_frame(&content)
}

pub fn send_data(
    address: Option<&[u8; 8]>,
    address16: Option<&[u8; 2]>,
    data: &[u8],
    frame_id: u8,
) -> Result<Vec<u8>, String> {
    let mut addr64: [u8; 8] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut addr16: [u8; 2] = [0xFF, 0xFE];
    match (address, address16) {
        (Some(a64), _) => addr64 = *a64,
        (None, Some(a16)) => addr16 = *a16,
        (None, None) => return Err("at least one address type is required".into()),
    }
    let broadcast_radius = 0;
    let transmit_options = 0;

    let mut content = vec![TRANSMIT_REQUEST];
    content.push(frame_id);
    content.extend(&addr64);
    content.extend(&addr16);
    content.push(broadcast_radius);
    content.push(transmit_options);
    content.extend(data);
    make_api_frame(&content)
}

pub fn make_api_frame(content: &Vec<u8>) -> Result<Vec<u8>, String> {
    if content.len() > 0xff {
        return Err("content longer than 255".into());
    }
    let length: u8 = content.len() as u8;
    let checksum = compute_checksum(&content);
    let mut frame = vec![START_DELIMITER, 0x00, length];
    frame.extend(content);
    frame.push(checksum);
    Ok(frame)
}

pub fn compute_checksum(content: &[u8]) -> u8 {
    let mut sum: u32 = 0;
    for &byte in content {
        sum += byte as u32;
    }
    sum &= 0xFF;
    sum = (0xFF - (sum as i32)) as u32;
    sum as u8
}

pub fn check_checksum(content: &[u8], checksum: u8) -> bool {
    let mut sum: u32 = 0;
    for &byte in content {
        sum += byte as u32;
    }
    sum += checksum as u32;
    sum &= 0xFF;
    sum == 0xFFu32
}
