//! Parts of the Framework sensor driver that do not depend on the WDK:
//!
//! * [`ec`]: the `\Device\CrosEC` IOCTL ABI (`IOCTL_CROSEC_XCMD`,
//!   `IOCTL_CROSEC_RDMEM`) and the motion sense queries made over it,
//! * [`accel`]: reading and decoding lid accelerometer samples from the EC
//!   memory map, and the software thresholding applied to them,
//! * [`als`]: ambient light readings from the EC memory map and their
//!   thresholding,
//! * [`schedule`]: when to fire the next sampling timer.
//!
//! Device access goes through the [`ec::EcDevice`] trait, so all of this is
//! unit tested on the host against a simulated EC.

#![no_std]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

#[cfg(test)]
extern crate std;

pub mod accel;
pub mod als;
pub mod ec;
pub mod schedule;

#[cfg(test)]
mod sim;
