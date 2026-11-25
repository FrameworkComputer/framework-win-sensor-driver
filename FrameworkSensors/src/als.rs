//! The ambient light sensor, reported in lux.
//!
//! Ported from the C++ driver's `AlsClient.cpp` (which was compiled out),
//! without the color temperature and chromaticity fields it reported as
//! constants: the EC only provides lux.

use sensors_core::als::{
    self,
    Filter,
    Thresholds,
    DEFAULT_LUX_THRESHOLD_ABS,
    DEFAULT_LUX_THRESHOLD_PCT,
};
use wdk_sys::{NTSTATUS, STATUS_IO_DEVICE_ERROR};
use windows_sys::{
    core::GUID,
    Win32::Devices::Sensors::{
        GUID_SensorType_AmbientLight,
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
        pv_u32_vector,
        CollectionList,
        DEVPKEY_LightSensor_AutoBrightnessPreferred,
        DEVPKEY_Sensor_ConnectionType,
        DEVPKEY_Sensor_IsPrimary,
        DEVPKEY_Sensor_Manufacturer,
        DEVPKEY_Sensor_Model,
        DEVPKEY_Sensor_PersistentUniqueId,
        DEVPKEY_Sensor_Type,
        PKEY_LightSensor_ResponseCurve,
        PKEY_SensorDataField_RangeMaximum,
        PKEY_SensorDataField_RangeMinimum,
        PKEY_SensorDataField_Resolution,
        PKEY_SensorData_IsValid,
        PKEY_SensorData_LightLevel_Lux,
        PKEY_SensorData_LightLevel_Lux_Threshold_AbsoluteDifference,
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

/// {506AC61B-C762-499E-A848-685FC45DF39F}, same as the C++ driver.
const GUID_ALS_UNIQUE_ID: GUID = GUID::from_u128(0x506A_C61B_C762_499E_A848_685F_C45D_F39F);

/// Default (and minimum) data interval, as in the C++ driver.
const DEFAULT_MIN_DATA_INTERVAL_MS: u32 = 10;

/// The EC reports whole lux.
const LUX_RESOLUTION: f32 = 1.0;
const LUX_MINIMUM: f32 = 0.0;
/// Direct sunlight. The EC doesn't specify a maximum.
const LUX_MAXIMUM: f32 = 100_000.0;

/// Pairs of (display brightness offset in percent, lux), from the C++
/// driver, for adaptive brightness.
const RESPONSE_CURVE: [u32; 10] = [0, 10, 10, 40, 40, 100, 68, 400, 90, 1000];

// Data fields, in `supported_data_fields` and `data` order.
const DATA_TIMESTAMP: usize = 0;
const DATA_LUX: usize = 1;

pub struct State {
    pub filter: Filter,
}

/// `AlsDevice::Initialize`
pub fn describe() -> Result<Description, NTSTATUS> {
    let als_type = &GUID_SensorType_AmbientLight;
    let enumeration = CollectionList::new([
        (DEVPKEY_Sensor_Type, pv_clsid(als_type)),
        (
            DEVPKEY_Sensor_Manufacturer,
            pv_string("Framework Computer Inc"),
        ),
        (DEVPKEY_Sensor_Model, pv_string("ALS")),
        #[allow(clippy::cast_sign_loss, reason = "enum value 0")]
        (
            DEVPKEY_Sensor_ConnectionType,
            Ok(pv_u32(SensorConnectionType_Integrated as u32)),
        ),
        (
            DEVPKEY_Sensor_PersistentUniqueId,
            pv_clsid(&GUID_ALS_UNIQUE_ID),
        ),
        (DEVPKEY_Sensor_IsPrimary, Ok(pv_bool(true))),
        (
            DEVPKEY_LightSensor_AutoBrightnessPreferred,
            Ok(pv_bool(true)),
        ),
    ])?;

    let supported_data_fields = PropertyList::new([
        PKEY_SensorData_Timestamp,
        PKEY_SensorData_LightLevel_Lux,
        PKEY_SensorData_IsValid,
    ]);

    let data = CollectionList::new([
        (PKEY_SensorData_Timestamp, Ok(pv_filetime(now()))),
        (PKEY_SensorData_LightLevel_Lux, Ok(pv_f32(0.0))),
        (PKEY_SensorData_IsValid, Ok(pv_bool(true))),
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
        (PKEY_Sensor_Type, pv_clsid(als_type)),
        (
            PKEY_LightSensor_ResponseCurve,
            pv_u32_vector(&RESPONSE_CURVE),
        ),
    ])?;

    let data_field_properties = CollectionList::new([
        (PKEY_SensorDataField_Resolution, Ok(pv_f32(LUX_RESOLUTION))),
        (PKEY_SensorDataField_RangeMinimum, Ok(pv_f32(LUX_MINIMUM))),
        (PKEY_SensorDataField_RangeMaximum, Ok(pv_f32(LUX_MAXIMUM))),
    ])?;

    // The lux key holds the relative threshold.
    let thresholds = CollectionList::new([
        (
            PKEY_SensorData_LightLevel_Lux,
            Ok(pv_f32(DEFAULT_LUX_THRESHOLD_PCT)),
        ),
        (
            PKEY_SensorData_LightLevel_Lux_Threshold_AbsoluteDifference,
            Ok(pv_f32(DEFAULT_LUX_THRESHOLD_ABS)),
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
        kind: Kind::Als(State {
            filter: Filter::default(),
        }),
    })
}

impl State {
    /// `AlsDevice::GetData`
    pub fn read(&mut self, ec: &mut CrosEc) -> Result<Option<Sample>, NTSTATUS> {
        let lux = als::read_lux(ec).map_err(|e| {
            trace!("Reading the ALS failed: {e:?}");
            STATUS_IO_DEVICE_ERROR
        })?;
        if cfg!(debug_assertions) {
            trace!("ALS {lux} lux");
        }
        Ok(self.filter.accept(lux).then_some(Sample::Als(lux)))
    }

    /// `AlsDevice::UpdateCachedThreshold`
    pub fn update_thresholds(&mut self, thresholds: &CollectionList) -> Result<(), NTSTATUS> {
        self.filter.thresholds = Thresholds {
            pct: thresholds.get_f32(&PKEY_SensorData_LightLevel_Lux)?,
            abs: thresholds
                .get_f32(&PKEY_SensorData_LightLevel_Lux_Threshold_AbsoluteDifference)?,
        };
        Ok(())
    }
}

pub fn write_data(lux: f32, data: &mut CollectionList) {
    data.set(DATA_LUX, pv_f32(lux));
}
