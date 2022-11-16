// TODO parse ND response
// TODO send remote AT command
// TODO try out tokio-serial
// TODO CLI for sending AT/data, wait for X responses
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

// fn main() -> Result<(), Box<dyn std::error::Error>> {
fn main() -> Result<(), String> {
    let serial_path = "/dev/ttyUSB0";
    let baud_rate = 19200;

    // TODO allow any byte stream interface, including an async interface
    let mut port = serialport::new(serial_path, baud_rate)
        // parity=PARITY_NONE, stopbits=STOPBITS_ONE
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open serial device");

    println!("Send the NI (Node Identifier) AT command to the coordinator");
    let frame = zb_frames::local_at_command("NI".as_bytes(), None, None)?;
    port.write(&frame).map_err(|e| format!("{:?}", e))?;

    // TODO
    // println!("Send the NI (Node Identifier) AT command to the remote xbee with address {}", remote_addr);
    // let frame = zb_frames::remote_at_command(remote_addr, "NI".as_bytes(), None, None)?;
    // port.write(&frame).map_err(|e| format!("{:?}", e))?;

    println!("Send the ND (Node Discovery) AT command to the coordinator");
    println!("Expect a response from every remote xbee");
    // TODO use enum or builder/function/method pattern
    // e.g. zb_frames::nd_command(None) zb_frames::nd_command(Some("destination"))
    let frame2 = zb_frames::local_at_command("ND".as_bytes(), None, None)?;
    port.write(&frame2).map_err(|e| format!("{:?}", e))?;

    /*
    // Send data to the remote xbee's serial output (e.g. Webasto/Tesla commands)
    let frame3 = zb_frames::send_data(
        Some(&[0, 19, 162, 0, 65, 103, 52, 98]),
        None,
        "test".as_bytes(),
        0x88,
    )?;
    port.write(&frame3).map_err(|e| format!("{:?}", e))?;
    */

    println!("Polling for responses");
    let mut count = 0;
    loop {
        let frame = zb_parse_frame::parse_frame(&mut port, 2)?;

        // println!("{:?}", &response);
        match frame {
            zb_parse_frame::Frame::Response(response) => {
                if count % 10 == 0 {
                    println!("{} frames received, and", count);
                    println!("{:?} {}", &response.addr64, show(&response.data));
                }
                count += 1
            }
            _ => {
                println!("{:?}", frame);
                match frame {
                    zb_parse_frame::Frame::LocalATCommand(at_resp) => {
                        println!("{}", show(&at_resp.data_bytes.unwrap_or(vec![])))
                    }
                    _ => {}
                }
            }
        }
    }
}
