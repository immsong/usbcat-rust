use anyhow::{Context, Result};
use nusb::{
    Device, DeviceInfo, Interface, MaybeFuture,
    descriptors::{ConfigurationDescriptor, TransferType},
};

#[derive(Debug, Clone, Copy)]
pub struct BulkEndpoint {
    pub address: u8,
    pub max_packet_size: usize,
}

#[derive(Debug, Clone)]
pub struct BulkTarget {
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,

    pub in_endpoints: Vec<BulkEndpoint>,
    pub out_endpoints: Vec<BulkEndpoint>,
}

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

pub fn find_bulk_targets(device: &Device) -> Result<Vec<BulkTarget>> {
    let config = device
        .active_configuration()
        .context("failed to get active USB configuration")?;

    let mut targets = Vec::new();

    for interface in config.interface_alt_settings() {
        let mut in_endpoints = Vec::new();
        let mut out_endpoints = Vec::new();

        for endpoint in interface.endpoints() {
            if endpoint.transfer_type() != TransferType::Bulk {
                continue;
            }

            let endpoint = BulkEndpoint {
                address: endpoint.address(),
                max_packet_size: usize::from(endpoint.max_packet_size()),
            };

            if endpoint.address & 0x80 != 0 {
                in_endpoints.push(endpoint);
            } else {
                out_endpoints.push(endpoint);
            }
        }

        if in_endpoints.is_empty() || out_endpoints.is_empty() {
            continue;
        }

        targets.push(BulkTarget {
            interface_number: interface.interface_number(),
            alternate_setting: interface.alternate_setting(),
            class: interface.class(),
            subclass: interface.subclass(),
            protocol: interface.protocol(),
            in_endpoints,
            out_endpoints,
        });
    }

    Ok(targets)
}

pub fn print_bulk_target(index: usize, target: &BulkTarget) {
    println!(
        "[{index}] Interface {} / Alt {}",
        target.interface_number, target.alternate_setting,
    );

    println!(
        "    Class    : 0x{:02x}{}",
        target.class,
        class_name(target.class),
    );

    println!("    Subclass : 0x{:02x}", target.subclass);
    println!("    Protocol : 0x{:02x}", target.protocol);

    print!("    Bulk IN  :");

    for endpoint in &target.in_endpoints {
        print!(
            " 0x{:02x} (max={})",
            endpoint.address, endpoint.max_packet_size,
        );
    }

    println!();

    print!("    Bulk OUT :");

    for endpoint in &target.out_endpoints {
        print!(
            " 0x{:02x} (max={})",
            endpoint.address, endpoint.max_packet_size,
        );
    }

    println!();
}

pub fn claim_bulk_target(device: &Device, target: &BulkTarget) -> Result<Interface> {
    let interface = device
        .claim_interface(target.interface_number)
        .wait()
        .with_context(|| format!("failed to claim interface {}", target.interface_number))?;

    if target.alternate_setting != 0 {
        interface
            .set_alt_setting(target.alternate_setting)
            .wait()
            .with_context(|| {
                format!(
                    "failed to select alternate setting {} on interface {}",
                    target.alternate_setting, target.interface_number,
                )
            })?;
    }

    Ok(interface)
}
