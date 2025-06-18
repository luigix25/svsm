#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VsockError {
    /// Generic error for all socket operations on a vsock device.
    Failed,
}
