use crate::{
    error::SvsmError,
    io::{Read, Write},
    vsock::{VsockError, VSOCK_DEVICE},
};

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
