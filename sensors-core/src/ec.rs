//! Talking to the EC through the Chrome EC bus driver (`crosecbus`, bound to
//! `ACPI\FRMWC004`), which exposes `\Device\CrosEC` to user mode.
//!
//! Only `EC_CMD_MOTION_SENSE_CMD` is needed, to find out which motion
//! sensor is the lid accelerometer. Samples are read from the EC memory map.

use core::mem::size_of;

/// `FILE_DEVICE_CROS_EMBEDDED_CONTROLLER`
pub const FILE_DEVICE_CROS_EMBEDDED_CONTROLLER: u32 = 0x80EC;

const METHOD_BUFFERED: u32 = 0;
const FILE_READ_DATA: u32 = 0x0001;
const FILE_WRITE_DATA: u32 = 0x0002;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

/// Sends a host command. In and out buffer are both a [`CrosEcCommand`].
pub const IOCTL_CROSEC_XCMD: u32 = ctl_code(
    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
    0x801,
    METHOD_BUFFERED,
    FILE_READ_DATA | FILE_WRITE_DATA,
);
/// Reads the EC memory map. In and out buffer are both a [`CrosEcReadMem`].
pub const IOCTL_CROSEC_RDMEM: u32 = ctl_code(
    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
    0x802,
    METHOD_BUFFERED,
    FILE_READ_DATA,
);
const _: () = assert!(IOCTL_CROSEC_XCMD == 0x80EC_E004);
const _: () = assert!(IOCTL_CROSEC_RDMEM == 0x80EC_6008);

/// `CROS_EC_CMD_MAX_REQUEST`: size of [`CrosEcCommand::data`].
pub const CROS_EC_CMD_MAX_REQUEST: usize = 0x100 - 8;
/// `CROSEC_MEMMAP_SIZE`: size of the EC memory map and of
/// [`CrosEcReadMem::buffer`].
pub const CROSEC_MEMMAP_SIZE: usize = 0xFF;

/// `EC_RES_SUCCESS`
pub const EC_RES_SUCCESS: u32 = 0;

pub const EC_CMD_MOTION_SENSE_CMD: u16 = 0x002B;
pub const MOTIONSENSE_CMD_DUMP: u8 = 0;
pub const MOTIONSENSE_CMD_INFO: u8 = 1;
pub const MOTIONSENSE_TYPE_ACCEL: u8 = 0;
pub const MOTIONSENSE_LOC_BASE: u8 = 0;
pub const MOTIONSENSE_LOC_LID: u8 = 1;

/// `CROSEC_COMMAND`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CrosEcCommand {
    pub version: u32,
    pub command: u32,
    pub outlen: u32,
    pub inlen: u32,
    pub result: u32,
    pub data: [u8; CROS_EC_CMD_MAX_REQUEST],
}
const _: () = assert!(size_of::<CrosEcCommand>() == 0x10C);

/// `CROSEC_READMEM`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CrosEcReadMem {
    pub offset: u32,
    pub bytes: u32,
    pub buffer: [u8; CROSEC_MEMMAP_SIZE],
}
const _: () = assert!(size_of::<CrosEcReadMem>() == 0x108);

/// A handle to `\Device\CrosEC`.
///
/// Both calls are a `DeviceIoControl` with the structure as in and out
/// buffer. `Err` carries the Win32 error code.
pub trait EcDevice {
    /// `IOCTL_CROSEC_XCMD`
    ///
    /// # Errors
    ///
    /// The Win32 error code if the IOCTL failed.
    fn xcmd(&mut self, cmd: &mut CrosEcCommand) -> Result<(), u32>;

    /// `IOCTL_CROSEC_RDMEM`
    ///
    /// # Errors
    ///
    /// The Win32 error code if the IOCTL failed.
    fn rdmem(&mut self, rm: &mut CrosEcReadMem) -> Result<(), u32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcError {
    /// Request or response doesn't fit, or there is no request.
    InvalidArgument,
    /// The IOCTL failed with this Win32 error.
    Io(u32),
    /// The EC returned this `EC_RES_*` code.
    Ec(u32),
    /// The EC succeeded but returned no data.
    EmptyResponse,
    /// `offset` is outside the memory map.
    OutOfRange,
}

/// `CrosEcSendCommand`: sends host command `command` with `request` as
/// parameters, and copies the first `response.len()` bytes of the result into
/// `response`.
///
/// Returns the response length the EC reported.
///
/// # Errors
///
/// See [`EcError`]. As in the C driver, an empty response counts as failure.
pub fn send_command(
    dev: &mut impl EcDevice,
    command: u16,
    version: u8,
    request: &[u8],
    response: &mut [u8],
) -> Result<usize, EcError> {
    if request.is_empty()
        || request.len() > CROS_EC_CMD_MAX_REQUEST
        || response.len() > CROS_EC_CMD_MAX_REQUEST
    {
        return Err(EcError::InvalidArgument);
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "request.len() <= CROS_EC_CMD_MAX_REQUEST, checked above"
    )]
    let mut cmd = CrosEcCommand {
        version: u32::from(version),
        command: u32::from(command),
        outlen: request.len() as u32,
        // Same as the C driver: ask for up to the buffer size minus the
        // 8 byte host command header.
        inlen: (CROS_EC_CMD_MAX_REQUEST - 8) as u32,
        result: 0xFF,
        data: [0; CROS_EC_CMD_MAX_REQUEST],
    };
    cmd.data[..request.len()].copy_from_slice(request);

    dev.xcmd(&mut cmd).map_err(EcError::Io)?;

    if cmd.result != EC_RES_SUCCESS {
        return Err(EcError::Ec(cmd.result));
    }
    if cmd.inlen == 0 {
        return Err(EcError::EmptyResponse);
    }

    response.copy_from_slice(&cmd.data[..response.len()]);
    Ok(cmd.inlen as usize)
}

/// `CrosEcReadMemU8`: reads one byte of the EC memory map.
///
/// Reads are done one byte at a time like the C driver. Multi-byte reads
/// that are not dword aligned overrun their buffer in older builds of the C
/// `crosecbus` driver, which is still what most machines run.
///
/// # Errors
///
/// [`EcError::OutOfRange`] or [`EcError::Io`].
pub fn read_mem_u8(dev: &mut impl EcDevice, offset: usize) -> Result<u8, EcError> {
    if offset >= CROSEC_MEMMAP_SIZE {
        return Err(EcError::OutOfRange);
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "offset < CROSEC_MEMMAP_SIZE, checked above"
    )]
    let mut rm = CrosEcReadMem {
        offset: offset as u32,
        bytes: 1,
        buffer: [0; CROSEC_MEMMAP_SIZE],
    };
    dev.rdmem(&mut rm).map_err(EcError::Io)?;
    Ok(rm.buffer[0])
}

/// Number of motion sensors the EC has (`MOTIONSENSE_CMD_DUMP`, version 1).
///
/// # Errors
///
/// If the host command fails.
pub fn motion_sensor_count(dev: &mut impl EcDevice) -> Result<u8, EcError> {
    // ec_params_motion_sense.dump: cmd, max_sensor_count. Asking for 0
    // sensors skips the per-sensor data.
    let request = [MOTIONSENSE_CMD_DUMP, 0];
    // ec_response_motion_sense.dump: module_flags, sensor_count, ...
    let mut response = [0u8; 2];
    send_command(dev, EC_CMD_MOTION_SENSE_CMD, 1, &request, &mut response)?;
    Ok(response[1])
}

/// `MOTIONSENSE_CMD_INFO` response (the version 1 part).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSensorInfo {
    pub sensor_type: u8,
    pub location: u8,
    pub chip: u8,
}

/// Type and location of motion sensor `index` (`MOTIONSENSE_CMD_INFO`,
/// version 1).
///
/// # Errors
///
/// If the host command fails.
pub fn motion_sensor_info(dev: &mut impl EcDevice, index: u8) -> Result<MotionSensorInfo, EcError> {
    let request = [MOTIONSENSE_CMD_INFO, index];
    let mut response = [0u8; 3];
    send_command(dev, EC_CMD_MOTION_SENSE_CMD, 1, &request, &mut response)?;
    Ok(MotionSensorInfo {
        sensor_type: response[0],
        location: response[1],
        chip: response[2],
    })
}

/// Motion sensor indices of the two accelerometers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccelIndices {
    pub lid: u8,
    pub base: u8,
}

/// `CrosEcGetAccelIndeces`: finds the lid and base accelerometers among the
/// first `sensor_count` motion sensors.
///
/// Sensors whose info query fails are skipped. If there are several
/// accelerometers in one location, the last one wins.
///
/// Returns `None` unless both were found.
pub fn find_accel_indices(dev: &mut impl EcDevice, sensor_count: u8) -> Option<AccelIndices> {
    let mut lid = None;
    let mut base = None;

    for index in 0..sensor_count {
        let Ok(info) = motion_sensor_info(dev, index) else {
            continue;
        };
        if info.sensor_type != MOTIONSENSE_TYPE_ACCEL {
            continue;
        }
        match info.location {
            MOTIONSENSE_LOC_LID => lid = Some(index),
            MOTIONSENSE_LOC_BASE => base = Some(index),
            _ => {}
        }
    }

    Some(AccelIndices {
        lid: lid?,
        base: base?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::SimEc;

    #[test]
    fn command_layout() {
        let mut ec = SimEc::new();
        let mut response = [0u8; 2];
        let len = send_command(&mut ec, 0x1234, 3, &[0xAA, 0xBB], &mut response).unwrap();
        assert_eq!(len, 2);

        let sent = ec.last_command.unwrap();
        assert_eq!(sent.command, 0x1234);
        assert_eq!(sent.version, 3);
        assert_eq!(sent.outlen, 2);
        assert_eq!(sent.inlen, 0xF0);
        assert_eq!(&sent.data[..2], &[0xAA, 0xBB]);
    }

    #[test]
    fn send_command_rejects_bad_sizes() {
        let mut ec = SimEc::new();
        let big = [0u8; CROS_EC_CMD_MAX_REQUEST + 1];
        let mut response = [0u8; 1];
        assert_eq!(
            send_command(&mut ec, 1, 0, &[], &mut response),
            Err(EcError::InvalidArgument)
        );
        assert_eq!(
            send_command(&mut ec, 1, 0, &big, &mut response),
            Err(EcError::InvalidArgument)
        );
        assert_eq!(
            send_command(&mut ec, 1, 0, &[0], &mut [0u8; CROS_EC_CMD_MAX_REQUEST + 1]),
            Err(EcError::InvalidArgument)
        );
        assert!(ec.last_command.is_none());
    }

    #[test]
    fn send_command_errors() {
        let mut ec = SimEc::new();
        let mut response = [0u8; 2];

        ec.ec_result = 3;
        assert_eq!(
            send_command(&mut ec, 1, 0, &[0], &mut response),
            Err(EcError::Ec(3))
        );

        ec.ec_result = 0;
        ec.response_len = Some(0);
        assert_eq!(
            send_command(&mut ec, 1, 0, &[0], &mut response),
            Err(EcError::EmptyResponse)
        );

        ec.ioctl_error = Some(31);
        assert_eq!(
            send_command(&mut ec, 1, 0, &[0], &mut response),
            Err(EcError::Io(31))
        );
    }

    #[test]
    fn read_mem() {
        let mut ec = SimEc::new();
        ec.memmap[0x90] = 0x85;
        assert_eq!(read_mem_u8(&mut ec, 0x90), Ok(0x85));
        assert_eq!(read_mem_u8(&mut ec, 0xFF), Err(EcError::OutOfRange));
        ec.ioctl_error = Some(5);
        assert_eq!(read_mem_u8(&mut ec, 0x90), Err(EcError::Io(5)));
    }

    #[test]
    fn finds_accelerometers() {
        let mut ec = SimEc::new();
        assert_eq!(motion_sensor_count(&mut ec), Ok(3));
        assert_eq!(
            find_accel_indices(&mut ec, 3),
            Some(AccelIndices { lid: 2, base: 0 })
        );
    }

    #[test]
    fn missing_accelerometer() {
        let mut ec = SimEc::new();
        // Only look at the base accelerometer and the gyro.
        assert_eq!(find_accel_indices(&mut ec, 2), None);
    }

    #[test]
    fn failing_info_is_skipped() {
        let mut ec = SimEc::new();
        ec.fail_info_for = Some(1);
        assert_eq!(
            find_accel_indices(&mut ec, 3),
            Some(AccelIndices { lid: 2, base: 0 })
        );
    }
}
