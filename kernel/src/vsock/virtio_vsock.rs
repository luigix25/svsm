// SPDX-License-Identifier: MIT
//
// Copyright (c) 2025 Red Hat, Inc.
//
// Author: Luigi Leonardi <leonardi@redhat.com>

use crate::error::SvsmError;
use crate::utils::immut_after_init::ImmutAfterInitCell;
use crate::vsock::api::VsockDriver;
extern crate alloc;
use crate::io::{Read, Write};
use crate::virtio::devices::{MMIOSlot, VirtIOVsockDevice, MMIO_SLOTS};
use crate::vsock::VsockError;
use alloc::boxed::Box;

use virtio_drivers::device::socket::{ConnectionStatus, SocketError, VsockAddr};
use virtio_drivers::Error;
pub struct VirtIOVsockDriver(Box<VirtIOVsockDevice>);

impl core::fmt::Debug for VirtIOVsockDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtIOVsockDriver").finish()
    }
}

static VSOCK_DEVICE: ImmutAfterInitCell<Box<dyn VsockDriver>> = ImmutAfterInitCell::uninit();

pub fn initialize_vsock() {
    let mut binding = MMIO_SLOTS.lock_write();
    let slots = binding.as_deref_mut();
    if slots.is_none() {
        return;
    }

    let driver = slots
        .unwrap()
        .iter_mut()
        .filter(|slot| slot.free)
        .find_map(|slot| VirtIOVsockDriver::new(slot).ok());

    if driver.is_none() {
        log::info!("virtio-vsock device not found");
        return;
    }

    VSOCK_DEVICE
        .init(Box::new(driver.unwrap()))
        .expect("vsock driver already initialized");
}

impl VirtIOVsockDriver {
    pub fn new(mmio_slot: &mut MMIOSlot) -> Result<Self, SvsmError> {
        Ok(VirtIOVsockDriver(VirtIOVsockDevice::new(mmio_slot)?))
    }
}

impl VsockDriver for VirtIOVsockDriver {
    fn connect(&self, remote_cid: u64, local_port: u32, remote_port: u32) -> Result<(), Error> {
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        self.0
            .device
            .locked_do(|dev| dev.connect(server_address, local_port))?;

        loop {
            let mut dev = self.0.device.lock();

            dev.wait_for_event()?;
            let status = dev.get_connection_status(server_address, local_port)?;

            match status {
                ConnectionStatus::Connected => {
                    return Ok(());
                }
                ConnectionStatus::Connecting => {}
                _ => {
                    return Err(SocketError::NotConnected.into());
                }
            }
        }
    }

    fn recv(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let mut total_received: usize = 0;

        loop {
            let mut dev = self.0.device.lock();

            let server_address = VsockAddr {
                cid: remote_cid,
                port: remote_port,
            };

            // In case of error return the bytes read so far
            let received = match dev.recv(server_address, local_port, &mut buffer[total_received..])
            {
                Ok(value) => value,
                Err(error) => {
                    if total_received > 0 {
                        return Ok(total_received);
                    } else {
                        return Err(error);
                    }
                }
            };
            log::debug!("[vsock] received: {received}");

            total_received += received;

            let result = dev.update_credit(server_address, local_port);
            if result.is_err() {
                return Ok(total_received);
            }

            if total_received == buffer.len() {
                break;
            }

            dev.wait_for_event()?;
        }

        Ok(buffer.len())
    }

    fn send(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &[u8],
    ) -> Result<usize, Error> {
        let mut dev = self.0.device.lock();

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.send(server_address, local_port, buffer)?;
        Ok(buffer.len())
    }

    fn shutdown(&self, remote_cid: u64, local_port: u32, remote_port: u32) -> Result<(), Error> {
        let mut dev = self.0.device.lock();

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.shutdown(server_address, local_port)
    }

    fn force_shutdown(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
    ) -> Result<(), Error> {
        let mut dev = self.0.device.lock();
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.force_close(server_address, local_port)
    }

    fn get_first_free_port(&self) -> Result<u32, Error> {
        unimplemented!()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum VsockStreamStatus {
    Connected,
    Closed,
}

#[derive(Debug)]
pub struct VsockStream {
    local_port: u32,
    remote_port: u32,
    remote_cid: u64,
    status: VsockStreamStatus,
}

impl VsockStream {
    pub fn connect(local_port: u32, remote_port: u32, remote_cid: u64) -> Result<Self, SvsmError> {
        if VSOCK_DEVICE.try_get_inner().is_err() {
            return Err(SvsmError::Vsock(VsockError::Failed));
        }

        if VSOCK_DEVICE
            .connect(remote_cid, local_port, remote_port)
            .is_err()
        {
            return Err(SvsmError::Vsock(VsockError::ConnectFailed));
        }

        Ok(Self {
            local_port,
            remote_port,
            remote_cid,
            status: VsockStreamStatus::Connected,
        })
    }

    pub fn shutdown(&mut self) -> Result<(), SvsmError> {
        if VSOCK_DEVICE.try_get_inner().is_err() {
            return Err(SvsmError::Vsock(VsockError::Failed));
        }

        self.status = VsockStreamStatus::Closed;
        VSOCK_DEVICE
            .shutdown(self.remote_cid, self.local_port, self.remote_port)
            .map_err(|_| SvsmError::Vsock(VsockError::Failed))
    }
}

impl Read for VsockStream {
    type Err = SvsmError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Err> {
        if VSOCK_DEVICE.try_get_inner().is_err() {
            return Err(SvsmError::Vsock(VsockError::Failed));
        }

        if self.status == VsockStreamStatus::Closed {
            return Err(SvsmError::Vsock(VsockError::RecvFailed));
        }

        match VSOCK_DEVICE.recv(self.remote_cid, self.local_port, self.remote_port, buf) {
            Ok(some) => Ok(some),
            Err(_e) => Ok(0),
        }
    }
}

impl Write for VsockStream {
    type Err = SvsmError;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Err> {
        if VSOCK_DEVICE.try_get_inner().is_err() {
            return Err(SvsmError::Vsock(VsockError::Failed));
        }

        VSOCK_DEVICE
            .send(self.remote_cid, self.local_port, self.remote_port, buf)
            .map_err(|_| SvsmError::Vsock(VsockError::SendFailed))
    }
}

impl Drop for VsockStream {
    fn drop(&mut self) {
        if self.status == VsockStreamStatus::Closed || VSOCK_DEVICE.try_get_inner().is_err() {
            return;
        }

        let _ = VSOCK_DEVICE.force_shutdown(self.remote_cid, self.local_port, self.remote_port);
    }
}
