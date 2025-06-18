// SPDX-License-Identifier: MIT
//
// Copyright (c) 2025 Red Hat, Inc.
//
// Author: Luigi Leonardi <leonardi@redhat.com>

use crate::error::SvsmError;
use crate::virtio::devices::VirtIOVsockDevice;
use crate::virtio::mmio::{MmioSlot, MmioSlots};
use crate::vsock::VSOCK_MIN_PORT;
use crate::vsock::VsockDriver;
use crate::vsock::api::VsockTransport;
use crate::vsock::{VSOCK_DEVICE, VsockError};

extern crate alloc;
use alloc::boxed::Box;
use core::sync::atomic::AtomicU32;

use virtio_drivers::device::socket::VsockAddr;
use virtio_drivers::transport::DeviceType::Socket;

pub struct VirtIOVsockDriver(VirtIOVsockDevice);

impl core::fmt::Debug for VirtIOVsockDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VirtIOVsockDriver").finish()
    }
}

/// Initializes the global vsock device subsystem with a VirtIO vsock driver.
///
/// This function searches for a virtio-vsock device in the MMIO slots list.
/// If discovered, the first virtio-vsock device will be initialized and
/// registered as the global vsock device.
/// **Only one vsock device is supported**
///
/// # Arguments
///
/// * `slots` - The virtio MMIO slots list
///
/// # Returns
///
/// * Returns Ok() if:
///     * The driver is correctly initialized
///     * No virtio-vsock devices are found
/// * Returns an error if:
///     * The driver initialization fails
///     * The global vsock device has already been initialized
pub fn initialize_vsock(slots: &mut MmioSlots) -> Result<(), SvsmError> {
    let Some(slot) = slots.pop_slot(Socket) else {
        return Ok(());
    };

    let transport = VirtIOVsockDriver::new(slot)?;

    let driver = VsockDriver {
        first_free_port: AtomicU32::new(VSOCK_MIN_PORT),
        transport,
    };

    VSOCK_DEVICE.init(driver)?;

    Ok(())
}

impl VirtIOVsockDriver {
    pub fn new(mmio_slot: MmioSlot) -> Result<Box<Self>, SvsmError> {
        Ok(Box::new(VirtIOVsockDriver(VirtIOVsockDevice::new(
            mmio_slot,
        )?)))
    }
}

impl VsockTransport for VirtIOVsockDriver {
    fn connect(&self, remote_cid: u64, local_port: u32, remote_port: u32) -> Result<(), SvsmError> {
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        self.0
            .device
            .locked_do(|dev| dev.connect(server_address, local_port))
            .map_err(VsockError::from)?;

        loop {
            let mut dev = self.0.device.lock();

            dev.wait_for_event().map_err(VsockError::from)?;
            let status = dev
                .is_connection_established(server_address, local_port)
                .map_err(VsockError::from)?;

            if status {
                return Ok(());
            }
        }
    }

    fn recv(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &mut [u8],
    ) -> Result<usize, SvsmError> {
        let mut total_received: usize = 0;
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        loop {
            let mut dev = self.0.device.lock();

            let received = match dev.recv(server_address, local_port, &mut buffer[total_received..])
            {
                Ok(value) => value,
                Err(error) => {
                    if total_received > 0 {
                        return Ok(total_received);
                    } else {
                        return Err(SvsmError::Vsock(VsockError::from(error)));
                    }
                }
            };
            log::debug!("[vsock] received: {received}");

            total_received += received;

            let result = dev.update_credit(server_address, local_port);
            if result.is_err() || total_received == buffer.len() {
                break;
            }

            dev.wait_for_event().map_err(VsockError::from)?;
        }

        Ok(total_received)
    }

    fn send(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &[u8],
    ) -> Result<usize, SvsmError> {
        let mut dev = self.0.device.lock();

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.send(server_address, local_port, buffer)
            .map_err(VsockError::from)?;
        Ok(buffer.len())
    }

    fn shutdown(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
    ) -> Result<(), SvsmError> {
        let mut dev = self.0.device.lock();

        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.shutdown(server_address, local_port)
            .map_err(VsockError::from)?;
        Ok(())
    }

    fn force_shutdown(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
    ) -> Result<(), SvsmError> {
        let mut dev = self.0.device.lock();
        let server_address = VsockAddr {
            cid: remote_cid,
            port: remote_port,
        };

        dev.force_close(server_address, local_port)
            .map_err(VsockError::from)?;
        Ok(())
    }

    fn is_local_port_used(&self, port: u32) -> Result<bool, SvsmError> {
        let dev = self.0.device.lock();

        Ok(dev.is_local_port_used(port))
    }
}

impl From<virtio_drivers::Error> for VsockError {
    fn from(e: virtio_drivers::Error) -> Self {
        use virtio_drivers::Error::SocketDeviceError;
        use virtio_drivers::device::socket::SocketError;

        match e {
            SocketDeviceError(SocketError::ConnectionExists) => VsockError::ConnectionExists,
            SocketDeviceError(SocketError::NotConnected) => VsockError::NotConnected,
            SocketDeviceError(SocketError::PeerSocketShutdown) => VsockError::PeerSocketShutdown,
            _ => VsockError::DriverError,
        }
    }
}
