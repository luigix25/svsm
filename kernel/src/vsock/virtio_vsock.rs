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