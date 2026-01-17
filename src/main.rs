use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, SwitchCode};
use input::{Libinput, LibinputInterface, event::EventTrait};
use libc::{O_RDONLY, O_RDWR, O_WRONLY};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::os::unix::{fs::OpenOptionsExt, io::OwnedFd};
use std::path::Path;

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(File::from(fd))
    }
}

fn main() -> std::io::Result<()> {
    let mut device = create_virtual_device().unwrap();

    let mut input = Libinput::new_with_udev(Interface);
    input.udev_assign_seat("seat0").unwrap();

    let mut n_keyboards = 0;
    loop {
        input.dispatch().unwrap_or_default();
        for event in &mut input {
            let delta = match &event {
                input::Event::Device(device_event) => match is_keyboard(device_event.device()) {
                    true => match device_event {
                        input::event::DeviceEvent::Added(_) => 1,
                        input::event::DeviceEvent::Removed(_) => -1,
                        _ => 0,
                    },
                    false => 0,
                },
                _ => 0,
            };
            let n_keyboards_next = n_keyboards + delta;
            let mode = match (n_keyboards, n_keyboards_next) {
                (0, 1) => Some(0),
                (1, 0) => Some(1),
                _ => None,
            };
            match mode {
                Some(value) => write_tablet_mode(&mut device, value),
                None => {}
            }
            n_keyboards = n_keyboards_next;
        }
    }
}

fn create_virtual_device() -> std::io::Result<VirtualDevice> {
    let mut switches = AttributeSet::<SwitchCode>::new();
    switches.insert(SwitchCode::SW_TABLET_MODE);

    VirtualDevice::builder()?
        .name("Virtual Tablet Mode")
        .with_switches(&switches)?
        .build()
}

fn is_keyboard(device: input::Device) -> bool {
    let has_cap = device.has_capability(input::DeviceCapability::Keyboard);

    let name = device.name().to_string();
    // TODO: Make this configurable (obviously).
    let mut blacklist = HashSet::new();
    for s in [
        "Video Bus",
        "Power Button",
        "Legion-Controller 1-B0 Keyboard",
        "Ideapad extra buttons",
        "AT Translated Set 2 keyboard",
    ] {
        blacklist.insert(s.to_string());
    }

    has_cap && !blacklist.contains(&name)
}

fn write_tablet_mode(device: &mut VirtualDevice, value: i32) {
    let event = evdev::InputEvent::new(
        evdev::EventType::SWITCH.0,
        evdev::SwitchCode::SW_TABLET_MODE.0,
        value,
    );
    device.emit(&[event]).unwrap();
    println!("SW_TABLET_MODE {}", value);
}
