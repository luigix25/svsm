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

    pub fn connect(&self, remote_cid : u32, remote_port : u32) -> Result<(), ()> {

        // Tengo bloccato tutto perche' non posso perdermi il pacchetto della connessione
        // andata a buon fine/fallita

        let res = self.0.device.locked_do(|dev| {
            //dev sarebbe ConnectionManager

            let local_port = 1234;
            let server_address = VsockAddr {
                cid: VMADDR_CID_HOST,
                port: remote_port,
            };

            log::info!("Connecting to host on port {remote_port}...");

            // send connection request
            match dev.connect(server_address, local_port) {
                Err(e) => {
                    log::info!("Errore connect {e:?}");
                    return Err(());
                }
                Ok(value) => {
                    log::info!("connect driver ok {value:?}");
                }
            };

            // qui in mezzo mi puo' arrivare un evento da un'altra connessione?
            // di sicuro non di connessione.
            // Forse una richiesta di chiusura?

            loop {
                let event = dev.wait_for_event().unwrap();
                if event.source != server_address || event.destination.port != local_port {
                    // non un evento per me
                    log::info!("Ricevuto un evento (non mio). {:?}",event.event_type);
                    continue;
                }

                match event.event_type {
                    VsockEventType::Disconnected { reason } => {
                        log::info!("Connessione fallita: {:?}", reason);
                        return Err(())
                    }

                    VsockEventType::Connected => {
                        log::info!("Connesso!");
                        break;
                    }

                    // Per tutti gli altri tipi di evento, fai qualcos'altro
                    _ => {
                        log::info!("Ricevuto un evento. {:?}",event.event_type);
                    }
                }

            }

            Ok(())

        });
        res
    }

    pub fn recv(&self, remote_cid : u32, remote_port : u32, buffer : &mut [u8]) -> Result<usize, ()> {

        let mut first_clean_pos : usize = 0;

        loop {
            // Q: va out of scope alla fine del loop?
            let mut dev = self.0.device.lock();
            //dev sarebbe ConnectionManager

            let local_port = 1234;
            let server_address = VsockAddr {
                cid: VMADDR_CID_HOST,
                port: remote_port,
            };

            // in questo modo se chiedo 5 byte non me ne puo' restituire di meno
            // Non puo' fare overflow nel buffer
            let received = match dev.recv(server_address, local_port, &mut buffer[first_clean_pos .. ]) {
                Ok(received) => {
                    log::info!("Ricevuti: {received}");
                    received
                },
                Err(e) => return Err(()),
            };

            first_clean_pos += received;

            // mi devo bloccare in attesa che arrivi un evento, non e' importante il tipo di evento
            // nel caso di errore, sara' la recv a dare errore
            // nel caso di dati invece la recv li leggera' correttamente
            if received < buffer.len() && first_clean_pos != buffer.len() {
                let event = dev.wait_for_event().unwrap();
                if event.source != server_address || event.destination.port != local_port {
                    // non un evento per me
                    log::info!("Ricevuto un evento (non mio). {:?}",event.event_type);
                    continue;
                }

                log::info!("evento. {:?}",event);

                /*match event.event_type {
                    VsockEventType::Disconnected
                }*/

            } else {
                break;
            }
        }

        Ok(buffer.len())

    }

    pub fn send(&self, remote_cid : u32, remote_port : u32, buffer : &[u8]) -> Result<(), ()> {
        let res = self.0.device.locked_do(|dev| {
            //dev sarebbe ConnectionManager

            let local_port = 1234;
            let server_address = VsockAddr {
                cid: VMADDR_CID_HOST,
                port: remote_port,
            };

            return dev.send(server_address, local_port, &buffer);
        });

        match res {
            Ok(a) => {
                return Ok(());
            },
            Err(e) => {
                return Err(());
            }

        }
    }

    pub fn close(&self, remote_cid : u32, remote_port : u32) -> Result<(), ()> {
        let res = self.0.device.locked_do(|dev| {
            //dev sarebbe ConnectionManager
            let local_port = 1234;
            let server_address = VsockAddr {
                cid: VMADDR_CID_HOST,
                port: remote_port,
            };

            dev.shutdown(server_address, local_port);

            /*loop {
                let event = dev.wait_for_event().unwrap();
                if event.source != server_address || event.destination.port != local_port {
                    // non un evento per me
                    log::info!("Ricevuto un evento (non mio). {:?}",event.event_type);
                    continue;
                }
            }*/


            Ok(())
        });
        res
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

        //remote_cid : u32, remote_port : u32
        match device.connect(2, 12345){
            Err(e) => {
                log::info!("Connessione fallita.");
                return;
            },
            Ok(o) => {}
        }

        let mut buffer : [u8; 5] = [0; 5];
        let ricevuto = match device.recv(2, 12345, &mut buffer) {
            Ok(value) => value,
            Err(e) => {
                log::info!("errore recv");
                return;
            }
        };

        device.close(2, 12345);
        log::info!("post close");

        //dentro VirtIOVsockDevice ho ConnectionManager
        log::info!("Mega effess, ricevuti {ricevuto}");
        let stringa = core::str::from_utf8(&buffer);
        log::info!("Mega effess, ricevuti {stringa:?}");

        match device.send(2, 12345, &buffer) {
            Ok(value) => log::info!("send ok"),
            Err(e) => {
                log::info!("errore send");
                return;
            }
        }

        //}
    }
}