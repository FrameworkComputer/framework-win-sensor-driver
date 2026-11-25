//! The lid accelerometer, reported as a 3D accelerometer.

use sensors_core::{
    accel::{
        self,
        Axes,
        Filter,
        AXIS_MAXIMUM,
        AXIS_MINIMUM,
        AXIS_RESOLUTION,
        DEFAULT_AXIS_THRESHOLD,
    },
    ec::{self, AccelIndices},
};
use wdk_sys::{NTSTATUS, STATUS_DEVICE_NOT_READY, STATUS_IO_DEVICE_ERROR, STATUS_NOT_FOUND};
use windows_sys::{
    core::GUID,
    Win32::Devices::Sensors::{
        GUID_SensorType_Accelerometer3D,
        SensorConnectionType_Integrated,
        SensorState_Initializing,
    },
};

use crate::{
    collections::{
        pv_bool,
        pv_clsid,
        pv_f32,
        pv_filetime,
        pv_string,
        pv_u32,
        CollectionList,
        DEVPKEY_Sensor_ConnectionType,
        DEVPKEY_Sensor_IsPrimary,
        DEVPKEY_Sensor_Manufacturer,
        DEVPKEY_Sensor_Model,
        DEVPKEY_Sensor_PersistentUniqueId,
        DEVPKEY_Sensor_Type,
        PKEY_SensorDataField_RangeMaximum,
        PKEY_SensorDataField_RangeMinimum,
        PKEY_SensorDataField_Resolution,
        PKEY_SensorData_AccelerationX_Gs,
        PKEY_SensorData_AccelerationY_Gs,
        PKEY_SensorData_AccelerationZ_Gs,
        PKEY_SensorData_Shake,
        PKEY_SensorData_Timestamp,
        PKEY_Sensor_MaximumDataFieldSize_Bytes,
        PKEY_Sensor_MinimumDataInterval_Ms,
        PKEY_Sensor_State,
        PKEY_Sensor_Type,
        PropertyList,
    },
    ec::CrosEc,
    sensor::{now, Description, Kind, Sample},
    trace,
};

/// {A931067B-D450-42A4-8B20-2647D4CA9D2D}, same as the C++ driver so
/// Windows sees the same sensor.
const GUID_ACCELEROMETER_UNIQUE_ID: GUID =
    GUID::from_u128(0xA931_067B_D450_42A4_8B20_2647_D4CA_9D2D);

/// Default (and minimum) data interval.
const DEFAULT_MIN_DATA_INTERVAL_MS: u32 = 4;

// Data fields, in `supported_data_fields` and `data` order.
const DATA_X: usize = 0;
const DATA_Y: usize = 1;
const DATA_Z: usize = 2;
const DATA_TIMESTAMP: usize = 3;
const DATA_SHAKE: usize = 4;

pub struct State {
    /// `None` until the EC told us which sensor is the lid accelerometer.
    accel_indices: Option<AccelIndices>,
    pub filter: Filter,
}

/// Finds the lid accelerometer. `Ok(None)` if the EC can't be asked (yet).
fn probe(ec: Option<&mut CrosEc>) -> Result<Option<AccelIndices>, NTSTATUS> {
    let Some(ec) = ec else {
        return Ok(None);
    };
    let Ok(count) = ec::motion_sensor_count(ec) else {
        return Ok(None);
    };
    trace!("Found {count} motion sensors");
    if count == 0 {
        return Err(STATUS_NOT_FOUND);
    }
    let indices = ec::find_accel_indices(ec, count).ok_or(STATUS_NOT_FOUND)?;
    trace!(
        "Lid accelerometer: {}, base accelerometer: {}",
        indices.lid,
        indices.base
    );
    Ok(Some(indices))
}

/// `AccelerometerDevice::Initialize`
///
/// Fails if the EC is reachable and reports no lid and base
/// accelerometers. If the EC isn't reachable yet, the accelerometers are
/// looked up on the first sample instead.
pub fn describe(ec: Option<&mut CrosEc>) -> Result<Description, NTSTATUS> {
    let accel_indices = probe(ec)?;

    let accel3d = &GUID_SensorType_Accelerometer3D;
    let enumeration = CollectionList::new([
        (DEVPKEY_Sensor_Type, pv_clsid(accel3d)),
        (
            DEVPKEY_Sensor_Manufacturer,
            pv_string("Framework Computer Inc"),
        ),
        (DEVPKEY_Sensor_Model, pv_string("Accelerometer")),
        #[allow(clippy::cast_sign_loss, reason = "enum value 0")]
        (
            DEVPKEY_Sensor_ConnectionType,
            Ok(pv_u32(SensorConnectionType_Integrated as u32)),
        ),
        (
            DEVPKEY_Sensor_PersistentUniqueId,
            pv_clsid(&GUID_ACCELEROMETER_UNIQUE_ID),
        ),
        (DEVPKEY_Sensor_IsPrimary, Ok(pv_bool(false))),
    ])?;

    let supported_data_fields = PropertyList::new([
        PKEY_SensorData_AccelerationX_Gs,
        PKEY_SensorData_AccelerationY_Gs,
        PKEY_SensorData_AccelerationZ_Gs,
        PKEY_SensorData_Timestamp,
        PKEY_SensorData_Shake,
    ]);

    let data = CollectionList::new([
        (PKEY_SensorData_AccelerationX_Gs, Ok(pv_f32(0.0))),
        (PKEY_SensorData_AccelerationY_Gs, Ok(pv_f32(0.0))),
        (PKEY_SensorData_AccelerationZ_Gs, Ok(pv_f32(0.0))),
        (PKEY_SensorData_Timestamp, Ok(pv_filetime(now()))),
        (PKEY_SensorData_Shake, Ok(pv_bool(false))),
    ])?;

    #[allow(clippy::cast_sign_loss, reason = "enum value 0")]
    let properties = CollectionList::new([
        (
            PKEY_Sensor_State,
            Ok(pv_u32(SensorState_Initializing as u32)),
        ),
        (
            PKEY_Sensor_MinimumDataInterval_Ms,
            Ok(pv_u32(DEFAULT_MIN_DATA_INTERVAL_MS)),
        ),
        (
            PKEY_Sensor_MaximumDataFieldSize_Bytes,
            Ok(pv_u32(data.marshalled_size())),
        ),
        (PKEY_Sensor_Type, pv_clsid(accel3d)),
    ])?;

    let data_field_properties = CollectionList::new([
        (PKEY_SensorDataField_Resolution, Ok(pv_f32(AXIS_RESOLUTION))),
        (PKEY_SensorDataField_RangeMinimum, Ok(pv_f32(AXIS_MINIMUM))),
        (PKEY_SensorDataField_RangeMaximum, Ok(pv_f32(AXIS_MAXIMUM))),
    ])?;

    // Timestamp and shake have no thresholds.
    let thresholds = CollectionList::new([
        (
            PKEY_SensorData_AccelerationX_Gs,
            Ok(pv_f32(DEFAULT_AXIS_THRESHOLD)),
        ),
        (
            PKEY_SensorData_AccelerationY_Gs,
            Ok(pv_f32(DEFAULT_AXIS_THRESHOLD)),
        ),
        (
            PKEY_SensorData_AccelerationZ_Gs,
            Ok(pv_f32(DEFAULT_AXIS_THRESHOLD)),
        ),
    ])?;

    Ok(Description {
        enumeration,
        supported_data_fields,
        data_field_properties,
        data,
        data_timestamp: DATA_TIMESTAMP,
        properties,
        thresholds,
        interval_ms: DEFAULT_MIN_DATA_INTERVAL_MS,
        kind: Kind::Accelerometer(State {
            accel_indices,
            filter: Filter::default(),
        }),
    })
}

impl State {
    /// `AccelerometerDevice::GetData`
    pub fn read(&mut self, ec: &mut CrosEc) -> Result<Option<Sample>, NTSTATUS> {
        // The EC wasn't reachable during initialization.
        let indices = if let Some(indices) = self.accel_indices {
            indices
        } else {
            let count = ec::motion_sensor_count(ec)
                .ok()
                .filter(|&count| count > 0)
                .ok_or(STATUS_DEVICE_NOT_READY)?;
            let indices = ec::find_accel_indices(ec, count).ok_or(STATUS_NOT_FOUND)?;
            trace!(
                "Lid accelerometer: {}, base accelerometer: {}",
                indices.lid,
                indices.base
            );
            *self.accel_indices.insert(indices)
        };

        if cfg!(debug_assertions) {
            if let Ok(status) = ec::read_mem_u8(ec, accel::EC_MEMMAP_ACC_STATUS) {
                trace!(
                    "Accelerometer status {status:#04x}, present: {}, busy: {}",
                    status & accel::EC_MEMMAP_ACC_STATUS_PRESENCE_BIT != 0,
                    status & accel::EC_MEMMAP_ACC_STATUS_BUSY_BIT != 0
                );
            }
            if let Ok(angle) = accel::read_lid_angle(ec) {
                let unreliable = if angle == accel::LID_ANGLE_UNRELIABLE {
                    " (unreliable)"
                } else {
                    ""
                };
                trace!("Lid angle {angle}{unreliable}");
            }
        }

        let sample = accel::read_sample(ec, indices.lid).map_err(|e| {
            trace!("Reading lid accelerometer {} failed: {e:?}", indices.lid);
            STATUS_IO_DEVICE_ERROR
        })?;
        if cfg!(debug_assertions) {
            trace!(
                "Accelerometer x: {}, y: {}, z: {}",
                sample.x,
                sample.y,
                sample.z
            );
        }

        Ok(self
            .filter
            .accept(sample)
            .then_some(Sample::Accelerometer(sample)))
    }

    /// `AccelerometerDevice::UpdateCachedThreshold`
    pub fn update_thresholds(&mut self, thresholds: &CollectionList) -> Result<(), NTSTATUS> {
        self.filter.thresholds = Axes {
            x: thresholds.get_f32(&PKEY_SensorData_AccelerationX_Gs)?,
            y: thresholds.get_f32(&PKEY_SensorData_AccelerationY_Gs)?,
            z: thresholds.get_f32(&PKEY_SensorData_AccelerationZ_Gs)?,
        };
        Ok(())
    }
}

pub fn write_data(sample: Axes, data: &mut CollectionList) {
    data.set(DATA_X, pv_f32(sample.x));
    data.set(DATA_Y, pv_f32(sample.y));
    data.set(DATA_Z, pv_f32(sample.z));
    // The EC doesn't detect shakes.
    data.set(DATA_SHAKE, pv_bool(false));
}
