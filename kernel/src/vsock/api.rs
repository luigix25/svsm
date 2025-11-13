// SPDX-License-Identifier: MIT
//
// Copyright (c) 2025 Red Hat, Inc.
//
// Author: Luigi Leonardi <leonardi@redhat.com>

use crate::error::SvsmError;

pub trait VsockDriver: Sync + Send {
    fn connect(&self, remote_cid: u64, local_port: u32, remote_port: u32) -> Result<(), Error>;
    fn send(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &[u8],
    ) -> Result<usize, Error>;
    fn recv(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
        buffer: &mut [u8],
    ) -> Result<usize, Error>;
    fn shutdown(&self, remote_cid: u64, local_port: u32, remote_port: u32) -> Result<(), Error>;
    fn force_shutdown(
        &self,
        remote_cid: u64,
        local_port: u32,
        remote_port: u32,
    ) -> Result<(), Error>;
    fn get_first_free_port(&self) -> Result<u32, Error>;
}
