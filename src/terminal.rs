use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use nusb::{
    Interface,
    transfer::{Buffer, Bulk, In, Out, TransferError},
};

use crate::usb::BulkEndpoint;

const RX_TIMEOUT: Duration = Duration::from_millis(500);
const TX_TIMEOUT: Duration = Duration::from_secs(1);

const RX_BUFFER_SIZE: usize = 64 * 1024;

pub fn run_hex_terminal(
    interface: &Interface,
    in_endpoint: BulkEndpoint,
    out_endpoint: BulkEndpoint,
) -> Result<()> {
    let mut ep_in = interface
        .endpoint::<Bulk, In>(in_endpoint.address)
        .with_context(|| {
            format!(
                "failed to open Bulk IN endpoint 0x{:02x}",
                in_endpoint.address
            )
        })?;

    let mut ep_out = interface
        .endpoint::<Bulk, Out>(out_endpoint.address)
        .with_context(|| {
            format!(
                "failed to open Bulk OUT endpoint 0x{:02x}",
                out_endpoint.address
            )
        })?;

    let rx_size = aligned_rx_size(RX_BUFFER_SIZE, in_endpoint.max_packet_size);

    let running = Arc::new(AtomicBool::new(true));
    let output_lock = Arc::new(Mutex::new(()));

    let rx_running = Arc::clone(&running);
    let rx_output_lock = Arc::clone(&output_lock);

    let rx_handle = thread::spawn(move || {
        while rx_running.load(Ordering::SeqCst) {
            let result = ep_in
                .transfer_blocking(Buffer::new(rx_size), RX_TIMEOUT)
                .into_result();

            match result {
                Ok(buffer) => {
                    if buffer.is_empty() {
                        continue;
                    }

                    let _guard = rx_output_lock.lock().unwrap();

                    println!();
                    print_hex_dump("RX", &buffer);

                    print!("> ");
                    let _ = io::stdout().flush();
                }

                Err(TransferError::Cancelled) => {
                    // Timeout.
                    // No data received, so just try again.
                }

                Err(err) => {
                    let _guard = rx_output_lock.lock().unwrap();

                    eprintln!();
                    eprintln!("RX error: {err}");

                    rx_running.store(false, Ordering::SeqCst);

                    break;
                }
            }
        }
    });

    {
        let _guard = output_lock.lock().unwrap();

        println!();
        println!("Hex terminal");
        println!("  Bulk IN  : 0x{:02x}", in_endpoint.address);
        println!("  Bulk OUT : 0x{:02x}", out_endpoint.address);
        println!();
        println!("Enter hex bytes to send.");
        println!("Example: 01 02 AA FF");
        println!("Type 'q' to quit.");
        println!();
    }

    while running.load(Ordering::SeqCst) {
        {
            let _guard = output_lock.lock().unwrap();

            print!("> ");
            io::stdout().flush()?;
        }

        let mut input = String::new();

        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();

        if input.eq_ignore_ascii_case("q")
            || input.eq_ignore_ascii_case("quit")
            || input.eq_ignore_ascii_case("exit")
        {
            break;
        }

        if input.is_empty() {
            continue;
        }

        let data = match parse_hex(input) {
            Ok(data) => data,

            Err(err) => {
                let _guard = output_lock.lock().unwrap();

                println!("Invalid hex input: {err}");

                continue;
            }
        };

        let result = ep_out
            .transfer_blocking(data.clone().into(), TX_TIMEOUT)
            .into_result();

        match result {
            Ok(_) => {
                let _guard = output_lock.lock().unwrap();

                print_hex_dump("TX", &data);
            }

            Err(err) => {
                running.store(false, Ordering::SeqCst);

                return Err(anyhow!("Bulk OUT transfer failed: {err}"));
            }
        }
    }

    running.store(false, Ordering::SeqCst);

    rx_handle
        .join()
        .map_err(|_| anyhow!("RX thread panicked"))?;

    Ok(())
}

fn aligned_rx_size(size: usize, max_packet_size: usize) -> usize {
    let max_packet_size = max_packet_size.max(1);

    size.div_ceil(max_packet_size) * max_packet_size
}

fn parse_hex(input: &str) -> Result<Vec<u8>> {
    let mut result = Vec::new();

    for token in input.split_whitespace() {
        let token = token.trim_end_matches(',').trim_end_matches(':');

        let token = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);

        if token.is_empty() || token.len() > 2 {
            return Err(anyhow!("invalid byte '{token}'"));
        }

        let value =
            u8::from_str_radix(token, 16).with_context(|| format!("invalid byte '{token}'"))?;

        result.push(value);
    }

    if result.is_empty() {
        return Err(anyhow!("no data"));
    }

    Ok(result)
}

fn print_hex_dump(label: &str, data: &[u8]) {
    let current_time = chrono::Local::now();
    println!(
        "{label} [{} bytes] ({})",
        data.len(),
        current_time.format("%Y-%m-%d %H:%M:%S")
    );

    for (offset, chunk) in data.chunks(16).enumerate() {
        print!("{:08x}  ", offset * 16);

        for byte in chunk {
            print!("{byte:02X} ");
        }

        println!();
    }
}
