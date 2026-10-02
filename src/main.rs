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

    usb::print_device_details(selected, &device);

    let targets = usb::find_bulk_targets(&device)?;

    if targets.is_empty() {
        println!();
        println!("No interface with both Bulk IN and Bulk OUT endpoints found.");

        return Ok(());
    }

    println!();
    println!("Usable Bulk interfaces");
    println!();

    for (index, target) in targets.iter().enumerate() {
        usb::print_bulk_target(index, target);
        println!();
    }

    let target_index = if targets.len() == 1 {
        println!("Using interface automatically: 0");
        0
    } else {
        let Some(index) = select_bulk_target(targets.len())? else {
            return Ok(());
        };

        index
    };

    let target = &targets[target_index];

    let Some(in_endpoint) = select_endpoint("Bulk IN", &target.in_endpoints)? else {
        return Ok(());
    };

    let Some(out_endpoint) = select_endpoint("Bulk OUT", &target.out_endpoints)? else {
        return Ok(());
    };

    println!();
    println!("Claiming interface {}...", target.interface_number);

    let _interface = match usb::claim_bulk_target(&device, target) {
        Ok(interface) => interface,

        Err(err) => {
            eprintln!();
            eprintln!("Error: {err:#}");
            eprintln!();
            eprintln!("The interface may already be claimed by another driver or process.");

            return Ok(());
        }
    };

    println!();
    println!("Ready");
    println!("  Interface : {}", target.interface_number);
    println!("  Alt       : {}", target.alternate_setting);
    println!("  Bulk IN   : 0x{:02x}", in_endpoint.address);
    println!("  Bulk OUT  : 0x{:02x}", out_endpoint.address);

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

fn select_endpoint(
    name: &str,
    endpoints: &[usb::BulkEndpoint],
) -> Result<Option<usb::BulkEndpoint>> {
    if endpoints.len() == 1 {
        let endpoint = endpoints[0];

        println!("{name} selected automatically: 0x{:02x}", endpoint.address);

        return Ok(Some(endpoint));
    }

    println!();
    println!("Available {name} endpoints");

    for (index, endpoint) in endpoints.iter().enumerate() {
        println!(
            "[{index}] 0x{:02x}  max_packet_size={}",
            endpoint.address, endpoint.max_packet_size,
        );
    }

    loop {
        print!(
            "Select {name} endpoint [0-{}] (q to quit): ",
            endpoints.len() - 1
        );

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
            Ok(index) if index < endpoints.len() => {
                return Ok(Some(endpoints[index]));
            }

            _ => {
                println!("Invalid selection.");
            }
        }
    }
}

fn select_bulk_target(count: usize) -> Result<Option<usize>> {
    loop {
        print!("Select interface [0-{}] (q to quit): ", count - 1);
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
