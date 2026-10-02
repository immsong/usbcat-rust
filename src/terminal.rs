use std::{
    io::{self, Write},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use nusb::{
    Interface,
    transfer::{Buffer, Bulk, In, Out, TransferError},
};

use crate::usb::BulkEndpoint;

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(1);
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

    println!();
    println!("Hex terminal");
    println!("  Enter bytes separated by spaces.");
    println!("  Example: 01 02 AA FF");
    println!("  Type 'q' to quit.");
    println!();

    loop {
        print!("> ");
        io::stdout().flush()?;

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
                println!("Invalid hex input: {err}");
                continue;
            }
        };

        print_hex_dump("TX", &data);

        ep_out
            .transfer_blocking(data.clone().into(), TRANSFER_TIMEOUT)
            .into_result()
            .map_err(|err| anyhow!("Bulk OUT transfer failed: {err}"))?;

        let result = ep_in
            .transfer_blocking(Buffer::new(rx_size), TRANSFER_TIMEOUT)
            .into_result();

        match result {
            Ok(buffer) => {
                print_hex_dump("RX", &buffer);
            }

            Err(TransferError::Cancelled) => {
                println!("RX timeout");
            }

            Err(err) => {
                return Err(anyhow!("Bulk IN transfer failed: {err}"));
            }
        }

        println!();
    }

    Ok(())
}

fn aligned_rx_size(size: usize, max_packet_size: usize) -> usize {
    let max_packet_size = max_packet_size.max(1);

    ((size + max_packet_size - 1) / max_packet_size) * max_packet_size
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
    println!("{label} [{} bytes]", data.len());

    for (offset, chunk) in data.chunks(16).enumerate() {
        print!("{:08x}  ", offset * 16);

        for byte in chunk {
            print!("{byte:02X} ");
        }

        println!();
    }
}
