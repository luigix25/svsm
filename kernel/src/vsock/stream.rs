// SPDX-License-Identifier: MIT
//
// Copyright (c) 2025 Red Hat, Inc.
//
// Author: Luigi Leonardi <leonardi@redhat.com>

use crate::{
    error::SvsmError,
    io::{Read, Write},
    vsock::{VSOCK_DEVICE, VsockError},
};

/// A vsock stream for communication between a virtual machine
/// and its host.
///
/// `VsockStream` provides a TCP-like socket interface over the VSOCK transport,
/// which is designed for communication between a guest VM and its host.
/// It implements the [`Read`] and [`Write`] traits for I/O operations.
///
/// # Examples
///
/// ```no_run
/// use crate::svsm::io::{Read, Write};
/// use crate::svsm::vsock::{VMADDR_CID_HOST, stream::VsockStream};
/// use svsm::error;
///
/// // Connect to host on port 12345
/// let mut stream = VsockStream::connect(12345, VMADDR_CID_HOST)?;
///
/// // Write data
/// let data = b"Hello, host!";
/// stream.write(data)?;
///
/// // Read response
/// let mut buffer = [0u8; 10];
/// let n = stream.read(&mut buffer)?;
/// # Ok::<(), error::SvsmError>(())
/// ```
///
/// # Connection Lifecycle
///
/// - A stream is created in the `Connected` state via [`connect()`](Self::connect).
/// - When dropped, the stream is automatically shutdown.
#[derive(Debug)]
pub struct VsockStream {
    local_port: u32,
    remote_port: u32,
    remote_cid: u32,
}

impl VsockStream {
    /// Establishes a VSOCK connection to a remote endpoint.
    ///
    /// Creates a new VSOCK stream and connects to the specified remote port and CID
    /// The local port is automatically assigned from available ports.
    ///
    /// # Arguments
    ///
    /// * `remote_port` - The port number on the remote endpoint to connect to.
    /// * `remote_cid` - The CID of the remote endpoint.
    ///
    /// # Returns
    ///
    /// Returns a connected `VsockStream` on success, or an error if:
    /// - The VSOCK device is not available (`VsockError::DriverError`)
    /// - No free local ports are available
    /// - The connection fails
    pub fn connect(remote_port: u32, remote_cid: u32) -> Result<Self, SvsmError> {
        let local_port = VSOCK_DEVICE.get_first_free_port()?;
        VSOCK_DEVICE.connect(remote_cid, local_port, remote_port)?;

        Ok(Self {
            local_port,
            remote_port,
            remote_cid,
        })
    }
}

impl Read for VsockStream {
    type Err = SvsmError;

    /// Perform a blocking read from the VSOCK stream into the provided buffer.
    ///
    /// # Arguments
    ///
    /// * `buf` - The buffer to read data into.
    ///
    /// # Returns
    ///
    /// Returns the number of bytes read on success, or 0 if the peer shut
    /// the connection down.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The VSOCK device is not available (`VsockError::DriverError`)
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Err> {
        match VSOCK_DEVICE.recv(self.remote_cid, self.local_port, self.remote_port, buf) {
            Ok(value) => Ok(value),
            Err(SvsmError::Vsock(VsockError::NotConnected)) => Ok(0),
            Err(SvsmError::Vsock(VsockError::PeerSocketShutdown)) => Ok(0),
            Err(e) => Err(e),
        }
    }
}

impl Write for VsockStream {
    type Err = SvsmError;

    /// Writes data from the provided buffer to the VSOCK stream.
    ///
    /// # Arguments
    ///
    /// * `buf` - The buffer containing data to write.
    ///
    /// # Returns
    ///
    /// Returns the number of bytes written on success, or an error if:
    /// - The VSOCK device is not available (`VsockError::DriverError`)
    /// - The send operation fails
    fn write(&mut self, buf: &[u8]) -> Result<usize, SvsmError> {
        VSOCK_DEVICE.send(self.remote_cid, self.local_port, self.remote_port, buf)
    }
}

impl Drop for VsockStream {
    fn drop(&mut self) {
        if VSOCK_DEVICE.try_get_inner().is_err() {
            return;
        }

        let _ = VSOCK_DEVICE.shutdown(self.remote_cid, self.local_port, self.remote_port, true);
    }
}
