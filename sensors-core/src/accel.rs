//! Lid accelerometer samples from the EC memory map, and the software
//! thresholding the driver applies before reporting them.
//!
//! The EC exposes the lid angle and up to two accelerometers at
//! `EC_MEMMAP_ACC_DATA`: a little endian `u16` lid angle followed by x, y, z
//! as little endian `i16` per sensor, indexed by motion sensor number.
//!
//! Only the lid accelerometer is reported, for screen rotation. The EC uses
//! the base accelerometer itself to compute the lid angle.

use crate::ec::{read_mem_u8, EcDevice, EcError};

/// Accelerometer status: sample ID, busy bit and presence bit.
pub const EC_MEMMAP_ACC_STATUS: usize = 0x90;
/// Lid angle, then per-sensor data.
pub const EC_MEMMAP_ACC_DATA: usize = 0x92;
pub const EC_MEMMAP_ACC_STATUS_SAMPLE_ID_MASK: u8 = 0x0F;
pub const EC_MEMMAP_ACC_STATUS_BUSY_BIT: u8 = 1 << 4;
pub const EC_MEMMAP_ACC_STATUS_PRESENCE_BIT: u8 = 1 << 7;
/// Lid angle the EC reports when it can't compute one.
pub const LID_ANGLE_UNRELIABLE: u16 = 500;

/// Default change in g on any axis before a new sample is reported.
pub const DEFAULT_AXIS_THRESHOLD: f32 = 1.0;
/// Smallest step the accelerometer can report, in g.
pub const AXIS_RESOLUTION: f32 = 4.0 / 65536.0;
pub const AXIS_MINIMUM: f32 = -2.0;
pub const AXIS_MAXIMUM: f32 = 2.0;

/// Counts per g for the ±2 g range, as the C driver computed it
/// (`0xFFFF / 4` in integer arithmetic).
#[allow(clippy::cast_precision_loss, reason = "16383 is exactly representable")]
const COUNTS_PER_G: f32 = (0xFFFF / 4) as f32;

/// An acceleration in g.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Axes {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Axes {
    /// Converts a raw sample (x, y, z as little endian `i16`) to g.
    ///
    /// The EC reports +1 g on z with the lid open flat and face up; Windows
    /// expects -1 g there, so every axis is negated.
    #[must_use]
    pub fn from_raw(raw: [u8; 6]) -> Self {
        let axis = |lo: u8, hi: u8| -f32::from(i16::from_le_bytes([lo, hi])) / COUNTS_PER_G;
        Self {
            x: axis(raw[0], raw[1]),
            y: axis(raw[2], raw[3]),
            z: axis(raw[4], raw[5]),
        }
    }

    /// Whether any axis moved by at least its threshold since `last`.
    #[must_use]
    pub fn exceeds(&self, last: &Self, thresholds: &Self) -> bool {
        (self.x - last.x).abs() >= thresholds.x
            || (self.y - last.y).abs() >= thresholds.y
            || (self.z - last.z).abs() >= thresholds.z
    }
}

/// Memory map offset of motion sensor `index`'s x, y, z.
#[must_use]
pub fn sample_offset(index: u8) -> usize {
    EC_MEMMAP_ACC_DATA + 2 + 6 * usize::from(index)
}

/// Reads motion sensor `index`'s current sample.
///
/// # Errors
///
/// If any byte can't be read, so that a failed read is never reported as
/// zero acceleration.
pub fn read_sample(dev: &mut impl EcDevice, index: u8) -> Result<Axes, EcError> {
    let offset = sample_offset(index);
    let mut raw = [0u8; 6];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = read_mem_u8(dev, offset + i)?;
    }
    Ok(Axes::from_raw(raw))
}

/// Reads the lid angle in degrees. [`LID_ANGLE_UNRELIABLE`] if unknown.
///
/// # Errors
///
/// If either byte can't be read.
pub fn read_lid_angle(dev: &mut impl EcDevice) -> Result<u16, EcError> {
    let lo = read_mem_u8(dev, EC_MEMMAP_ACC_DATA)?;
    let hi = read_mem_u8(dev, EC_MEMMAP_ACC_DATA + 1)?;
    Ok(u16::from_le_bytes([lo, hi]))
}

/// Software thresholding: decides which samples are reported.
///
/// The first sample after a (re)start is always reported. After that, a
/// sample is only reported once it differs from the last reported one by at
/// least the threshold on some axis.
#[derive(Debug, Clone, Copy)]
pub struct Filter {
    first_sample: bool,
    last: Axes,
    pub thresholds: Axes,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            first_sample: true,
            last: Axes::default(),
            thresholds: Axes {
                x: DEFAULT_AXIS_THRESHOLD,
                y: DEFAULT_AXIS_THRESHOLD,
                z: DEFAULT_AXIS_THRESHOLD,
            },
        }
    }
}

impl Filter {
    /// Report the next sample regardless of thresholds.
    pub const fn restart(&mut self) {
        self.first_sample = true;
    }

    #[must_use]
    pub const fn is_first_sample(&self) -> bool {
        self.first_sample
    }

    /// Returns whether `sample` should be reported, and if so records it as
    /// the last reported sample.
    pub fn accept(&mut self, sample: Axes) -> bool {
        if !self.first_sample && !sample.exceeds(&self.last, &self.thresholds) {
            return false;
        }
        self.first_sample = false;
        self.last = sample;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::SimEc;

    #[test]
    fn decode() {
        // +1 g on z from the EC is -1 g for Windows.
        let a = Axes::from_raw([0, 0, 0, 0, 0xFF, 0x3F]);
        assert_eq!(
            a,
            Axes {
                x: 0.0,
                y: 0.0,
                z: -1.0
            }
        );

        let a = Axes::from_raw([0x01, 0xC0, 0x00, 0x80, 0xFF, 0x7F]);
        assert_eq!(a.x, 1.0); // -16383
        assert_eq!(a.y, 32768.0 / 16383.0);
        assert_eq!(a.z, -32767.0 / 16383.0);
    }

    #[test]
    fn offsets() {
        assert_eq!(sample_offset(0), 0x94);
        assert_eq!(sample_offset(1), 0x9A);
        assert_eq!(sample_offset(2), 0xA0);
    }

    #[test]
    fn reads_lid_sample() {
        let mut ec = SimEc::new();
        ec.set_sample(2, [100, -200, 16383]);
        ec.memmap[0x92..0x94].copy_from_slice(&123u16.to_le_bytes());

        let a = read_sample(&mut ec, 2).unwrap();
        assert_eq!(a.x, -100.0 / 16383.0);
        assert_eq!(a.y, 200.0 / 16383.0);
        assert_eq!(a.z, -1.0);
        assert_eq!(read_lid_angle(&mut ec), Ok(123));
    }

    #[test]
    fn failed_read_is_an_error() {
        let mut ec = SimEc::new();
        ec.fail_rdmem_after = Some(3);
        assert_eq!(read_sample(&mut ec, 0), Err(EcError::Io(31)));
    }

    #[test]
    fn sample_past_memmap_is_an_error() {
        let mut ec = SimEc::new();
        assert_eq!(read_sample(&mut ec, 40), Err(EcError::OutOfRange));
    }

    #[test]
    fn filter() {
        let mut f = Filter::default();
        let flat = Axes {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        assert!(f.accept(flat));
        assert!(!f.is_first_sample());
        assert!(!f.accept(Axes { x: 0.9, ..flat }));
        assert!(f.accept(Axes { x: 1.0, ..flat }));
        // Compared against the last reported sample, not the last read.
        assert!(!f.accept(Axes { x: 0.5, ..flat }));
        assert!(f.accept(flat));

        f.restart();
        assert!(f.accept(flat));

        f.thresholds = Axes {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        assert!(f.accept(flat));
    }
}
