use anyhow::{Context, Result};
use nusb::{Device, DeviceInfo, MaybeFuture, descriptors::ConfigurationDescriptor};

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let devices = nusb::list_devices()
        .wait()
        .context("failed to enumerate USB devices")?
        .collect();

    Ok(devices)
}

pub fn open_device(info: &DeviceInfo) -> Result<Device> {
    info.open().wait().with_context(|| {
        format!(
            "failed to open USB device {:04x}:{:04x}",
            info.vendor_id(),
            info.product_id()
        )
    })
}

pub fn print_device_summary(index: usize, info: &DeviceInfo) {
    let product = info.product_string().unwrap_or("<unknown>");
    let serial = info.serial_number().unwrap_or("-");

    println!(
        "[{index}] {:04x}:{:04x}  {:<30}  serial={}",
        info.vendor_id(),
        info.product_id(),
        product,
        serial,
    );
}

pub fn print_device_details(info: &DeviceInfo, device: &Device) {
    println!();
    println!("Device");
    println!("  VID          : {:04x}", info.vendor_id());
    println!("  PID          : {:04x}", info.product_id());

    if let Some(manufacturer) = info.manufacturer_string() {
        println!("  Manufacturer : {manufacturer}");
    }

    if let Some(product) = info.product_string() {
        println!("  Product      : {product}");
    }

    if let Some(serial) = info.serial_number() {
        println!("  Serial       : {serial}");
    }

    if let Some(speed) = device.speed() {
        println!("  Speed        : {speed:?}");
    }

    println!("  Bus          : {}", info.bus_id());

    if !info.port_chain().is_empty() {
        let port = info
            .port_chain()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(".");

        println!("  Port         : {port}");
    }

    println!();

    match device.active_configuration() {
        Ok(config) => {
            println!("Active configuration: {}", config.configuration_value());

            print_configuration(config);
        }

        Err(err) => {
            println!("No active configuration: {err}");
            println!();
            println!("Available configurations:");

            for config in device.configurations() {
                println!("  Configuration {}", config.configuration_value());

                print_configuration(config);
            }
        }
    }
}

fn print_configuration(config: ConfigurationDescriptor<'_>) {
    for interface in config.interface_alt_settings() {
        println!();
        println!(
            "  Interface {} / Alt {}",
            interface.interface_number(),
            interface.alternate_setting(),
        );

        println!(
            "    Class    : 0x{:02x}{}",
            interface.class(),
            class_name(interface.class()),
        );

        println!("    Subclass : 0x{:02x}", interface.subclass(),);

        println!("    Protocol : 0x{:02x}", interface.protocol(),);

        for endpoint in interface.endpoints() {
            println!(
                "    Endpoint : 0x{:02x}  {:?}  {:?}  max_packet_size={}",
                endpoint.address(),
                endpoint.direction(),
                endpoint.transfer_type(),
                endpoint.max_packet_size(),
            );
        }
    }
}

fn class_name(class: u8) -> &'static str {
    match class {
        0x00 => " (Defined at interface level)",
        0x02 => " (CDC)",
        0x03 => " (HID)",
        0x08 => " (Mass Storage)",
        0x09 => " (Hub)",
        0xff => " (Vendor Specific)",
        _ => "",
    }
}
