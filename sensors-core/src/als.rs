//! Ambient light readings from the EC memory map, and the software
//! thresholding the driver applies before reporting them.

use crate::ec::{read_mem_u8, EcDevice, EcError};

/// Ambient light level in lux. `ec_commands.h` describes this as two 16-bit
/// readings; Framework ECs store one little endian `u32`, which is how the
/// C++ driver read it too.
pub const EC_MEMMAP_ALS: usize = 0x80;

/// Default relative change (1.0 = 100%) before a new reading is reported.
pub const DEFAULT_LUX_THRESHOLD_PCT: f32 = 1.0;
/// Default absolute change in lux before a new reading is reported.
pub const DEFAULT_LUX_THRESHOLD_ABS: f32 = 0.0;

/// Reads the current light level in lux.
///
/// # Errors
///
/// If any byte can't be read.
pub fn read_lux(dev: &mut impl EcDevice) -> Result<f32, EcError> {
    let mut raw = [0u8; 4];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = read_mem_u8(dev, EC_MEMMAP_ALS + i)?;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "lux readings are far below 2^24"
    )]
    Ok(u32::from_le_bytes(raw) as f32)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// Relative change, as a fraction of the last reported reading.
    pub pct: f32,
    /// Absolute change in lux.
    pub abs: f32,
}

/// Software thresholding: decides which readings are reported.
///
/// The first reading after a (re)start is always reported. After that, a
/// reading is only reported once it differs from the last reported one by
/// both the relative and the absolute threshold.
#[derive(Debug, Clone, Copy)]
pub struct Filter {
    first_sample: bool,
    last: f32,
    pub thresholds: Thresholds,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            first_sample: true,
            last: 0.0,
            thresholds: Thresholds {
                pct: DEFAULT_LUX_THRESHOLD_PCT,
                abs: DEFAULT_LUX_THRESHOLD_ABS,
            },
        }
    }
}

impl Filter {
    /// Report the next reading regardless of thresholds.
    pub const fn restart(&mut self) {
        self.first_sample = true;
    }

    #[must_use]
    pub const fn is_first_sample(&self) -> bool {
        self.first_sample
    }

    /// Returns whether `lux` should be reported, and if so records it as the
    /// last reported reading.
    pub fn accept(&mut self, lux: f32) -> bool {
        if !self.first_sample {
            let change = (lux - self.last).abs();
            if change < self.last * self.thresholds.pct || change < self.thresholds.abs {
                return false;
            }
        }
        self.first_sample = false;
        self.last = lux;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::SimEc;

    #[test]
    fn reads_lux() {
        let mut ec = SimEc::new();
        ec.memmap[0x80..0x84].copy_from_slice(&70_000u32.to_le_bytes());
        assert_eq!(read_lux(&mut ec), Ok(70_000.0));
    }

    #[test]
    fn failed_read_is_an_error() {
        let mut ec = SimEc::new();
        ec.fail_rdmem_after = Some(2);
        assert_eq!(read_lux(&mut ec), Err(EcError::Io(31)));
    }

    #[test]
    fn filter() {
        let mut f = Filter::default();
        assert!(f.accept(100.0));
        // Default: the light level has to double (or drop to 0).
        assert!(!f.accept(199.0));
        assert!(f.accept(200.0));
        assert!(f.accept(0.0));
        // From 0, any change is 100%.
        assert!(f.accept(1.0));

        f.thresholds = Thresholds { pct: 0.1, abs: 5.0 };
        assert!(!f.accept(5.0)); // 400%, but only 4 lux
        assert!(f.accept(6.0));
        assert!(!f.accept(6.5)); // 0.5 lux
        f.restart();
        assert!(f.accept(6.5));
    }
}
