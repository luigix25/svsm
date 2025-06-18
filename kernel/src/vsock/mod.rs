// SPDX-License-Identifier: MIT
//
// Copyright (c) 2025 Red Hat, Inc.
//
// Author: Luigi Leonardi <leonardi@redhat.com>

pub mod api;
pub mod error;
#[cfg(feature = "virtio-drivers")]
pub mod virtio_vsock;

pub use error::VsockError;

extern crate alloc;
use crate::{utils::immut_after_init::ImmutAfterInitCell, vsock::api::VsockDriver};
use alloc::boxed::Box;

static VSOCK_DEVICE: ImmutAfterInitCell<Box<dyn VsockDriver>> = ImmutAfterInitCell::uninit();
