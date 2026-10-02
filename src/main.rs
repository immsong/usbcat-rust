mod usb;

use std::io::{self, Write};

use anyhow::Result;

fn main() -> Result<()> {
    let devices = usb::list_devices()?;

    if devices.is_empty() {
        println!("No USB devices found.");
        return Ok(());
    }

    println!("USB devices");
    println!();

    for (index, device) in devices.iter().enumerate() {
        usb::print_device_summary(index, device);
    }

    let Some(index) = select_device(devices.len())? else {
        return Ok(());
    };

    let selected = &devices[index];

    println!();
    println!(
        "Opening {:04x}:{:04x}...",
        selected.vendor_id(),
        selected.product_id(),
    );

    let device = match usb::open_device(selected) {
        Ok(device) => device,

        Err(err) => {
            eprintln!();
            eprintln!("Error: {err:#}");

            #[cfg(target_os = "windows")]
            {
                eprintln!();
                eprintln!(
                    "Hint: make sure the target device/interface is using the WinUSB driver."
                );
                eprintln!("      Driver installation/configuration is outside usbcat.");
            }

            return Ok(());
        }
    };

    usb::print_device_details(selected, &device);

    Ok(())
}

fn select_device(count: usize) -> Result<Option<usize>> {
    loop {
        print!("\n");
        print!("Select device [0-{}] (q to quit): ", count - 1);
        io::stdout().flush()?;

        let mut input = String::new();

        if io::stdin().read_line(&mut input)? == 0 {
            return Ok(None);
        }

        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            return Ok(None);
        }

        match input.parse::<usize>() {
            Ok(index) if index < count => {
                return Ok(Some(index));
            }

            _ => {
                println!("Invalid selection.");
            }
        }
    }
}
