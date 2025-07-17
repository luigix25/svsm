use crate::address::PhysAddr;
use crate::error::SvsmError;
extern crate alloc;
use alloc::boxed::Box;
use crate::virtio::devices::VirtIOVsockDevice;

use virtio_drivers::device::socket::{VsockAddr, SocketError, ConnectionStatus};
use virtio_drivers::Error;
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

    pub fn connect(&self, remote_cid : u64, local_port : u32, remote_port : u32) -> Result<(), Error> {

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        self.0.device.locked_do(|dev| {
            //dev sarebbe ConnectionManager

            log::info!("Connecting to host on port {remote_port}...");

            // send connection request
            dev.connect(server_address, local_port)
        })?;

        loop {
            let mut dev = self.0.device.lock();
            // attendo un qualsiasi evento, non importa quale
            dev.wait_for_event()?;

            //controllare lo stato della connessione
            let stato = dev.get_connection_status(server_address, local_port)?;

            match stato {
                ConnectionStatus::Connected => {
                    return Ok(());
                }
                ConnectionStatus::Connecting => {
                }
                _ => {
                    return Err(SocketError::NotConnected.into());
                }

            }

        }
    }

    pub fn recv(&self, remote_cid : u64, local_port: u32, remote_port : u32, buffer : &mut [u8]) -> Result<usize, Error> {

        let mut first_clean_pos : usize = 0;

        loop {
            let mut dev = self.0.device.lock();
            //dev sarebbe ConnectionManager

            let server_address = VsockAddr {
                cid: remote_cid,
                port: remote_port,
            };

            // in questo modo se chiedo 5 byte non me ne puo' restituire di meno
            // Non puo' fare overflow nel buffer
            let received = dev.recv(server_address, local_port, &mut buffer[first_clean_pos .. ])?;
            log::info!("Ricevuti: {received}");

            first_clean_pos += received;

            // mi devo bloccare in attesa che arrivi un evento, non e' importante il tipo di evento
            // nel caso di errore, sara' la recv a dare errore
            // nel caso di dati invece la recv li leggera' correttamente
            if received < buffer.len() && first_clean_pos != buffer.len() {
                dev.wait_for_event()?;
            } else {
                break;
            }
        }

        Ok(buffer.len())
    }

    pub fn send(&self, remote_cid : u64, local_port : u32, remote_port : u32, buffer : &[u8]) -> Result<(), Error> {
        let mut dev = self.0.device.lock();
        //dev sarebbe ConnectionManager

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.send(server_address, local_port, buffer)
    }

    pub fn close(&self, remote_cid : u64, local_port : u32, remote_port : u32) -> Result<(), Error> {
        let mut dev = self.0.device.lock();

        //dev sarebbe ConnectionManager
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.shutdown(server_address, local_port)
    }
}

#[cfg(all(test, test_in_svsm))]
mod tests {
    use crate::{
        fw_cfg::FwCfg, platform::SVSM_PLATFORM, testutils::has_test_iorequests, address::PhysAddr
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

        if let Err(e) = device.connect(2, 1234, 12345) {
            log::info!("Connection failed: {e}");
            return;
        }

        let result = device.connect(2, 1234, 12345);
        assert!(result.is_err(), "The second connection operation was expected to fail, but it succeeded.");

        let mut buffer : [u8; 5] = [0; 5];
        let n_bytes = device.recv(2, 1234, 12345, &mut buffer).unwrap_or_else(|error| {
            log::info!("errore recv {error}");
            return 0;
        });

        if n_bytes < buffer.len() {
            log::info!("received less bytes than requested.");
            return;
        }

        let stringa = core::str::from_utf8(&buffer).unwrap();
        log::info!("received: {stringa:?}");

        if let Err(e) = device.close(2, 1234, 12345) {
            log::info!("Close failed: {e}");
            return;
        }

        let result = device.send(2, 1234, 12345, &buffer);
        assert!(result.is_err(), "The send operation was expected to fail, but it succeeded.");
    }
}