//! # Abstract
//!
//!    Sensor driver for Framework laptops (`ACPI\FRMWC006`). A UMDF 2 client
//!    of the Sensor Class Extension (SensorsCx) that reports the lid
//!    accelerometer, for screen rotation.
//!
//!    The accelerometer is behind the EC. Samples are polled from the EC
//!    memory map through the Chrome EC bus driver's `\Device\CrosEC`.
//!
//!    Rust port of the C++ driver in this repository's history.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_safety_doc)]
// The crate (and DLL) keeps the C driver's name, which the INF refers to.
#![allow(non_snake_case)]

#[cfg(feature = "accelerometer")]
mod accelerometer;
#[cfg(feature = "als")]
mod als;
mod clients;
mod collections;
mod device;
mod driver;
mod ec;
mod sensor;
mod sensorscx;
mod wdf_object_context;

use wdk_sys::{ULONG, WDF_OBJECT_CONTEXT_TYPE_INFO};

/// `OutputDebugString` with the driver name in front.
macro_rules! trace {
    ($($arg:tt)*) => {
        wdk::println!("FrameworkSensors: {}", format_args!($($arg)*))
    };
}
pub(crate) use trace;

/// Size of a WDF structure as a `ULONG`, for their `Size` fields.
macro_rules! wdf_struct_size {
    ($t:ty) => {{
        const S: usize = core::mem::size_of::<$t>();
        const {
            assert!(
                S <= wdk_sys::ULONG::MAX as usize,
                "structure size should fit in ULONG"
            );
        };
        #[allow(
            clippy::cast_possible_truncation,
            reason = "checked by the const assert above"
        )]
        {
            S as wdk_sys::ULONG
        }
    }};
}
pub(crate) use wdf_struct_size;

const WDF_OBJECT_CONTEXT_TYPE_INFO_SIZE: ULONG = wdf_struct_size!(WDF_OBJECT_CONTEXT_TYPE_INFO);
