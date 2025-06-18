use crate::address::PhysAddr;
use crate::error::SvsmError;
extern crate alloc;
use alloc::boxed::Box;
use crate::virtio::devices::VirtIOVsockDevice;

use virtio_drivers::device::socket::{VsockAddr, VMADDR_CID_HOST, VsockEventType};
pub struct VirtIOVsockDriver(Box<VirtIOVsockDevice>);

impl core::fmt::Debug for VirtIOVsockDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtIOVsockDriver").finish()
    }
}

impl VirtIOVsockDriver {
    pub fn new(mmio_base: PhysAddr) -> Result<Self, SvsmError> {
        Ok(VirtIOVsockDriver(VirtIOVsockDevice::new(mmio_base)?))
    }

    pub fn prova(&self) -> bool {
        self.0.device.locked_do(|dev| {
            //dev sarebbe ConnectionManager

            let port = 1234;
            let host_address = VsockAddr {
                cid: VMADDR_CID_HOST,
                port,
            };

            log::info!("Connecting to host on port {port}...");
            dev.connect(host_address, port);

            let event = dev.wait_for_event().unwrap();

            log::info!("{:?}",event.event_type);

            let buffer = b"abcd";
            //log::info!("{}",buffer.size_of());

            let res = dev.send(host_address, port, buffer);
            {
                //Ricevo un credit update
                let event = dev.wait_for_event().unwrap();
                //Sintassi a me sconosciuta
                /*let VsockEventType::Received { length, .. } = event.event_type else {
                    panic!("Received unexpected socket event {:?}", event);
                };*/
                //log::info!("{:?}",event.event_type);

                let mut buffer = [0u8; 24];
                let read_length = dev.recv(host_address, port, &mut buffer);
                log::info!(
                    "Received message: {:?}({:?}), len: {:?}",
                    buffer,
                    core::str::from_utf8(&buffer[..4]),
                    read_length
                );
            }

            log::info!("{res:?}");

        });
        true
    }

}

#[cfg(all(test, test_in_svsm))]
mod tests {
    use crate::{
        fw_cfg::FwCfg, platform::SVSM_PLATFORM, testutils::is_qemu_test_env, address::PhysAddr
    };

    use super::*;

    fn get_vsock_device() -> VirtIOVsockDriver {
        let cfg = FwCfg::new(SVSM_PLATFORM.get_io_port());

        let dev = cfg
            .get_virtio_mmio_addresses()
            .unwrap_or_default()
            .iter()
            .find_map(|a| VirtIOVsockDriver::new(PhysAddr::from(*a)).ok())
            .expect("No virtio-vsock device found");

        dev
    }

    #[test]
    #[cfg_attr(not(test_in_svsm), ignore = "Can only be run inside guest")]
    fn test_virtio_vsock() {

        let device = get_vsock_device();
        device.prova();

        //dentro VirtIOVsockDevice ho ConnectionManager
        log::info!(
            "Mega effess"
        );

        //}
    }
}