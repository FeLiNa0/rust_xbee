mod zb_frames;
mod zb_parse_frame;
use std::time::Duration;

fn show(bytes: &[u8]) -> String {
    let mut res = String::new();
    for &b in bytes {
        let part: Vec<u8> = std::ascii::escape_default(b).collect();
        res.push_str(&String::from_utf8(part).unwrap());
    }
    res
}

fn main() -> Result<(), String> {
    let serial_path = "/dev/ttyUSB0";
    let baud_rate = 19200;

    let mut port = serialport::new(serial_path, baud_rate)
        // parity=PARITY_NONE, stopbits=STOPBITS_ONE
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");

    let frame = zb_frames::local_at_command("NI".as_bytes(), None, None)?;
    println!("{:x?}", &frame);
    println!("{}", show(&frame));

    port.write(&frame).map_err(|e| format!("{:?}", e))?;

    let frame2 = zb_frames::local_at_command("ND".as_bytes(), None, None)?;

    port.write(&frame2).map_err(|e| format!("{:?}", e))?;

    let mut count = 0;
    loop {
        let frame = zb_parse_frame::parse_frame(&mut port, 2)?;

        // println!("{:?}", &response);
        match frame {
            zb_parse_frame::Frame::Response(response) => {
                if count % 10 == 0 {
                    println!("{} frames received, and", count);
                    println!("{}", show(&response.data));
                }
                count += 1
            }
            _ => {
                println!("{:?}", frame);
                match frame {
                    zb_parse_frame::Frame::LocalATCommand(at_resp) =>
                        println!("{}", show(&at_resp.data_bytes.unwrap_or(vec![]))),
                    _ => {},
                }
            },
        }
    }
}
