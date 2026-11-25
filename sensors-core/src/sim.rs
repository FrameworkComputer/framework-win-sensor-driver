//! A simulated `\Device\CrosEC` for tests.

use crate::{
    accel::sample_offset,
    ec::{
        CrosEcCommand,
        CrosEcReadMem,
        EcDevice,
        CROSEC_MEMMAP_SIZE,
        EC_CMD_MOTION_SENSE_CMD,
        MOTIONSENSE_CMD_DUMP,
        MOTIONSENSE_CMD_INFO,
        MOTIONSENSE_LOC_BASE,
        MOTIONSENSE_LOC_LID,
        MOTIONSENSE_TYPE_ACCEL,
    },
};

const MOTIONSENSE_TYPE_GYRO: u8 = 1;
const EC_RES_INVALID_COMMAND: u32 = 1;
const EC_RES_INVALID_PARAM: u32 = 3;
const ERROR_GEN_FAILURE: u32 = 31;

pub struct SimEc {
    pub memmap: [u8; CROSEC_MEMMAP_SIZE],
    /// (type, location) per motion sensor.
    pub sensors: &'static [(u8, u8)],

    pub last_command: Option<CrosEcCommand>,
    /// Fail every IOCTL with this Win32 error.
    pub ioctl_error: Option<u32>,
    /// Fail every host command with this EC result.
    pub ec_result: u32,
    /// Report this response length instead of the real one.
    pub response_len: Option<u32>,
    /// Fail `MOTIONSENSE_CMD_INFO` for this sensor.
    pub fail_info_for: Option<u8>,
    /// Fail memory map reads after this many have succeeded.
    pub fail_rdmem_after: Option<usize>,
    rdmem_count: usize,
}

impl SimEc {
    /// Base accelerometer, base gyro, lid accelerometer.
    pub fn new() -> Self {
        Self {
            memmap: [0; CROSEC_MEMMAP_SIZE],
            sensors: &[
                (MOTIONSENSE_TYPE_ACCEL, MOTIONSENSE_LOC_BASE),
                (MOTIONSENSE_TYPE_GYRO, MOTIONSENSE_LOC_BASE),
                (MOTIONSENSE_TYPE_ACCEL, MOTIONSENSE_LOC_LID),
            ],
            last_command: None,
            ioctl_error: None,
            ec_result: 0,
            response_len: None,
            fail_info_for: None,
            fail_rdmem_after: None,
            rdmem_count: 0,
        }
    }

    pub fn set_sample(&mut self, index: u8, xyz: [i16; 3]) {
        let offset = sample_offset(index);
        for (i, v) in xyz.iter().enumerate() {
            self.memmap[offset + 2 * i..offset + 2 * i + 2].copy_from_slice(&v.to_le_bytes());
        }
    }

    fn host_command(&self, cmd: &mut CrosEcCommand) -> Result<u32, u32> {
        if self.ec_result != 0 {
            return Err(self.ec_result);
        }
        if cmd.command != u32::from(EC_CMD_MOTION_SENSE_CMD) {
            // Pretend everything else is a two byte response.
            return Ok(2);
        }
        match cmd.data[0] {
            MOTIONSENSE_CMD_DUMP => {
                cmd.data[0] = 0;
                cmd.data[1] = u8::try_from(self.sensors.len()).unwrap();
                Ok(2)
            }
            MOTIONSENSE_CMD_INFO => {
                let index = cmd.data[1];
                if self.fail_info_for == Some(index) {
                    return Err(EC_RES_INVALID_PARAM);
                }
                let &(sensor_type, location) = self
                    .sensors
                    .get(usize::from(index))
                    .ok_or(EC_RES_INVALID_PARAM)?;
                cmd.data[..3].copy_from_slice(&[sensor_type, location, 0]);
                Ok(3)
            }
            _ => Err(EC_RES_INVALID_COMMAND),
        }
    }
}

impl EcDevice for SimEc {
    fn xcmd(&mut self, cmd: &mut CrosEcCommand) -> Result<(), u32> {
        self.last_command = Some(*cmd);
        if let Some(e) = self.ioctl_error {
            return Err(e);
        }
        match self.host_command(cmd) {
            Ok(len) => {
                cmd.result = 0;
                cmd.inlen = self.response_len.unwrap_or(len);
            }
            Err(result) => {
                cmd.result = result;
                cmd.inlen = 0;
            }
        }
        Ok(())
    }

    fn rdmem(&mut self, rm: &mut CrosEcReadMem) -> Result<(), u32> {
        if let Some(e) = self.ioctl_error {
            return Err(e);
        }
        if self.fail_rdmem_after.is_some_and(|n| self.rdmem_count >= n) {
            return Err(ERROR_GEN_FAILURE);
        }
        self.rdmem_count += 1;
        let start = rm.offset as usize;
        let len = rm.bytes as usize;
        rm.buffer[..len].copy_from_slice(&self.memmap[start..start + len]);
        Ok(())
    }
}
