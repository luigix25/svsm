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

#[cfg(all(test, test_in_svsm))]
mod tests {
    use crate::testutils::has_test_iorequests;

    use super::*;

    fn start_vsock_server_host() {
        use crate::serial::Terminal;
        use crate::testing::{svsm_test_io, IORequest};

        let sp = svsm_test_io().unwrap();

        sp.put_byte(IORequest::StartVsockServer as u8);

        let _ = sp.get_byte();
    }

    #[test]
    #[cfg_attr(not(test_in_svsm), ignore = "Can only be run inside guest")]
    fn test_virtio_vsock() {
        if !has_test_iorequests() {
            return;
        }

        start_vsock_server_host();

        let cid = 2;
        let local_port = 1234;
        let remote_port = 12345;

        let mut stream =
            VsockStream::connect(local_port, remote_port, cid).expect("connection failed");

        VsockStream::connect(local_port, remote_port, cid)
            .expect_err("The second connection operation was expected to fail, but it succeeded.");

        let mut buffer: [u8; 11] = [0; 11];
        let n_bytes = stream.read(&mut buffer).expect("read failed");
        assert!(
            n_bytes == buffer.len(),
            "Received less bytes than requested"
        );

        let string = core::str::from_utf8(&buffer).unwrap();
        log::info!("received: {string:?}");

        let n_bytes = stream.write(&buffer).expect("write failed");
        assert!(
            n_bytes == buffer.len(),
            "Sent less bytes than requested"
        );

        stream.shutdown().expect("shutdown failed");

        stream
            .write(&buffer)
            .expect_err("The write operation was expected to fail, but it succeeded.");

        stream
            .read(&mut buffer)
            .expect_err("The read operation was expected to fail, but it succeeded");
    }
}
