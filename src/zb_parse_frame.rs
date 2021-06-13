use std::collections::HashMap;
use std::convert::TryFrom;

const EXPLICIT_RX_TYPE: u8 = 0x91;
const LOCAL_AT_COMMAND_RESP_TYPE: u8 = 0x88;
const MTO_RRI_TYPE: u8 = 0xa3;

const MAX_FRAME_SIZE: usize = 0xffff;
const ESCAPE: u8 = 0x7D;
const ESCAPE_MASK: u8 = 0x20;
// Starting offsets
const LENGTH: u16 = 1;
const FRAME_TYPE: u16 = 3;
const HEADER_SIZE: u16 = FRAME_TYPE;

const ADDR64: u16 = 4;
const ADDR16: u16 = 12;
const IGNORE91: u16 = 14;
const DATA91: u16 = 21;

const DATA88: u16 = 8;

#[derive(Debug)]
pub enum DeviceType {
    COORDINATOR = 0,
    ROUTER,
    ENDDEVICE,
}

#[derive(Debug)]
pub struct NodeDiscoveryData {
    addr64: Vec<u8>,
    node_name: String,
    device_type: DeviceType,
    status: CommandStatus,
    digidevice_type: Option<Vec<u8>>,
    last_hop_rssi: Option<u8>,
}

#[derive(Debug)]
pub enum ATCommandData {
    String(String),
    Integer(i32),
    NodeDiscovery(NodeDiscoveryData),
    Celsius(i32),
    Bytes,
    Unknown,
}

#[derive(Debug, Hash)]
enum ATCommandDataTag {
    String,
    Integer,
    NodeDiscovery,
    Celsius,
    Bytes,
}

fn at_command_to_data_type<'a>() -> HashMap<String, ATCommandDataTag> {
    let mut map: HashMap<String, ATCommandDataTag> = HashMap::new();
    map.insert("ND".into(), ATCommandDataTag::NodeDiscovery);
    map.insert("NI".into(), ATCommandDataTag::String);
    map.insert("VL".into(), ATCommandDataTag::String);
    map.insert("TP".into(), ATCommandDataTag::Celsius);
    map
}

#[derive(Debug)]
pub struct ResponseFrame {
    pub addr64: Vec<u8>,
    pub addr16: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct LocalATCommandFrame {
    pub frame_id: u8,
    pub command: String,
    pub status: CommandStatus,
    pub data_bytes: Option<Vec<u8>>,
    pub data: Option<ATCommandData>,
}

#[derive(Debug)]
pub enum Frame {
    Response(ResponseFrame),
    LocalATCommand(LocalATCommandFrame),
    ManyToOneRRI,
}

#[derive(Debug)]
pub enum CommandStatus {
    Ok = 0,
    Error,
    InvalidCommand,
    InvalidParameter,
    Unknown = 0xff,
}

impl TryFrom<u8> for CommandStatus {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, String> {
        match v {
            x if x == Self::Ok as u8 => Ok(Self::Ok),
            x if x == Self::Error as u8 => Ok(Self::Error),
            x if x == Self::InvalidCommand as u8 => Ok(Self::InvalidCommand),
            x if x == Self::InvalidParameter as u8 => Ok(Self::InvalidParameter),
            _ => Err(format!("Unknown command status code {}", v)),
        }
    }
}

type SerialPort = Box<dyn serialport::SerialPort>;
fn read_byte(port: &mut SerialPort) -> Result<u8, String> {
    let mut response: Vec<u8> = vec![0; 1];
    loop {
        match port.read(response.as_mut_slice()) {
            Ok(_) => return Ok(response[0]),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
            Err(e) => return Err(format!("{:?}", e)),
        }
    }
}

fn skip_until_magic_byte(state: &mut ParseState) -> Result<(), String> {
    loop {
        let byte = read_byte(state.port)?;
        if byte == b'~' {
            state.index += 1;
            break;
        }
        // TODO what if too many unknown bytes?
    }
    Ok(())
}

fn parse_length(state: &mut ParseState) -> Result<u16, String> {
    let mut length;
    length = (read_byte(&mut state.port)? as u16) * 255;
    state.index += 1;
    length += read_byte(&mut state.port)? as u16;
    state.index += 1;
    Ok(length)
}

struct ParseState<'a> {
    port: &'a mut SerialPort,
    length: u16,
    index: u16,
    content: Vec<u8>,
    ap_param: i32,
}

fn unescape_byte(state: &mut ParseState) -> Result<u8, String> {
    let mut byte: u8 = read_byte(&mut state.port)?;
    if state.index > LENGTH && byte == ESCAPE {
        if state.ap_param == 2 {
            byte = read_byte(&mut state.port)? ^ ESCAPE_MASK;
        }
    }

    // Update state
    state.index += 1;
    let at_end = state.index > state.length + HEADER_SIZE;
    if state.index >= FRAME_TYPE && !at_end {
        state.content.push(byte);
    }
    if state.content.len() > MAX_FRAME_SIZE {
        return Err("greater than max frame size".into());
    }

    Ok(byte)
}

fn parse_frame_type(state: &mut ParseState) -> Result<u8, String> {
    unescape_byte(state)
}

fn validate_checksum(state: &mut ParseState) -> Result<(), String> {
    let checksum = unescape_byte(state)?;
    let content = &state.content;
    if !crate::zb_frames::check_checksum(&content, checksum) {
        println!(
            "{} {} {:?} {:x} {:x}",
            state.length,
            content.len(),
            content,
            checksum,
            crate::zb_frames::compute_checksum(&content)
        );
        // return content. length and checksum
        return Err("checksum check failed".into());
    }
    return Ok(());
}

fn parse_explicit_rx(state: &mut ParseState) -> Result<Frame, String> {
    let mut frame = ResponseFrame {
        addr64: Vec::with_capacity(8),
        addr16: Vec::with_capacity(2),
        data: Vec::with_capacity(state.length as usize),
    };

    for _ in ADDR64..ADDR16 {
        frame.addr64.push(unescape_byte(state)?);
    }

    for _ in ADDR16..IGNORE91 {
        frame.addr16.push(unescape_byte(state)?);
    }

    for _ in IGNORE91..DATA91 {
        unescape_byte(state)?;
    }

    for _ in DATA91..HEADER_SIZE + state.length {
        frame.data.push(unescape_byte(state)?);
    }

    validate_checksum(state)?;
    Ok(Frame::Response(frame))
}

fn parse_at_data(command: &String, data: &[u8]) -> Result<ATCommandData, String> {
    let map = at_command_to_data_type();
    match map.get(&*command) {
        Some(ATCommandDataTag::String) => Ok( ATCommandData::String(
                String::from_utf8(data.to_vec())
                .map_err(|e| format!("{:?}", e))?
        )),
        _ => Ok(ATCommandData::Bytes),
    }
}
fn parse_local_at_response(state: &mut ParseState) -> Result<Frame, String> {
    let mut frame = LocalATCommandFrame {
        command: String::with_capacity(2),
        frame_id: 0,
        status: CommandStatus::Unknown,
        data: Some(ATCommandData::Unknown),
        data_bytes: None,
    };

    frame.frame_id = unescape_byte(state)?;
    frame.command = String::from_utf8(vec![unescape_byte(state)?, unescape_byte(state)?])
        .map_err(|e| format!("{:?}", e))?;

    frame.status = CommandStatus::try_from(unescape_byte(state)?)?;

    let data_len = HEADER_SIZE + state.length;
    if state.index < data_len {
        let mut data = Vec::with_capacity((data_len - state.index) as usize);
        for _ in DATA88..data_len {
            data.push(unescape_byte(state)?);
        }
        frame.data = Some(parse_at_data(&frame.command, &data)?);
        frame.data_bytes = Some(data);
    } else {
        frame.data = None;
    }

    validate_checksum(state)?;
    Ok(Frame::LocalATCommand(frame))
}

fn parse_mto_rri(state: &mut ParseState) -> Result<Frame, String> {
    for _ in 4..12 {
        unescape_byte(state)?;
    }

    for _ in 12..14 {
        unescape_byte(state)?;
    }

    unescape_byte(state)?;

    validate_checksum(state)?;
    Ok(Frame::ManyToOneRRI)
}

pub fn parse_frame<'a>(port: &'a mut SerialPort, ap_param: i32) -> Result<Frame, String> {
    let mut state = ParseState {
        port: port,
        index: 0,
        length: 0xffff,
        content: Vec::new(),
        ap_param,
    };

    skip_until_magic_byte(&mut state)?;
    state.length = parse_length(&mut state)?;

    let frame_type = parse_frame_type(&mut state)?;

    match frame_type {
        EXPLICIT_RX_TYPE => return parse_explicit_rx(&mut state),
        LOCAL_AT_COMMAND_RESP_TYPE => return parse_local_at_response(&mut state),
        MTO_RRI_TYPE => return parse_mto_rri(&mut state),
        _ => return Err(format!("unknown frame type 0x{:x}", frame_type)),
    }
}
