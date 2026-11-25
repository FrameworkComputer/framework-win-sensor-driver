//! Bindings for the Sensor Class Extension (`SensorsCx.h` in
//! `um\sensors\1.1`), which neither `wdk-sys` nor `windows-sys` cover.
//!
//! Like WDF itself, SensorsCx is called through a function table.
//! `sensorscxstub.lib` provides the table and puts a bind info record in the
//! `.kmdfclassbind` section, where the WDF stub finds it and binds the class
//! extension before `DriverEntry` runs.

#![allow(non_snake_case, reason = "C structure field names")]
#![allow(clippy::upper_case_acronyms, reason = "C type names")]

use core::ffi::c_void;

use wdk_sys::{
    NTSTATUS,
    PWDFDEVICE_INIT,
    PWDF_OBJECT_ATTRIBUTES,
    ULONG,
    WDFDEVICE,
    WDFOBJECT,
    WDFREQUEST,
    WDF_TRI_STATE,
};
use windows_sys::Win32::{
    Devices::Sensors::{SENSOR_COLLECTION_LIST, SENSOR_PROPERTY_LIST},
    Foundation::PROPERTYKEY,
};

use crate::wdf_struct_size;

pub type SENSOROBJECT = WDFOBJECT;

pub type PfnStartSensor = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnStopSensor = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnGetSupportedDataFields = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    data_fields: *mut SENSOR_PROPERTY_LIST,
    size: *mut ULONG,
) -> NTSTATUS;
pub type PfnGetDataFieldProperties = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    data_field: *const PROPERTYKEY,
    properties: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS;
pub type PfnGetDataInterval =
    unsafe extern "C" fn(sensor: SENSOROBJECT, data_rate_ms: *mut ULONG) -> NTSTATUS;
pub type PfnSetDataInterval =
    unsafe extern "C" fn(sensor: SENSOROBJECT, data_rate_ms: ULONG) -> NTSTATUS;
pub type PfnGetDataThresholds = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    thresholds: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS;
pub type PfnSetDataThresholds =
    unsafe extern "C" fn(sensor: SENSOROBJECT, thresholds: *mut SENSOR_COLLECTION_LIST) -> NTSTATUS;
pub type PfnGetProperties = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    properties: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS;
pub type PfnDeviceIoControl = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    request: WDFREQUEST,
    output_buffer_length: usize,
    input_buffer_length: usize,
    io_control_code: ULONG,
) -> NTSTATUS;
pub type PfnStartSensorHistory = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnStopSensorHistory = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnClearSensorHistory = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnStartHistoryRetrieval = unsafe extern "C" fn(
    sensor: SENSOROBJECT,
    history: *mut SENSOR_COLLECTION_LIST,
    history_size_in_bytes: ULONG,
) -> NTSTATUS;
pub type PfnCancelHistoryRetrieval =
    unsafe extern "C" fn(sensor: SENSOROBJECT, bytes_written: *mut ULONG) -> NTSTATUS;
pub type PfnSetBatchLatency =
    unsafe extern "C" fn(sensor: SENSOROBJECT, batch_latency_ms: ULONG) -> NTSTATUS;
pub type PfnStateChangeNotification = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnEnableWake = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;
pub type PfnDisableWake = unsafe extern "C" fn(sensor: SENSOROBJECT) -> NTSTATUS;

/// `SENSOR_CONTROLLER_CONFIG`
#[repr(C)]
pub struct SENSOR_CONTROLLER_CONFIG {
    pub Size: ULONG,
    pub DriverIsPowerPolicyOwner: WDF_TRI_STATE,
    pub EvtSensorStart: Option<PfnStartSensor>,
    pub EvtSensorStop: Option<PfnStopSensor>,
    pub EvtSensorGetSupportedDataFields: Option<PfnGetSupportedDataFields>,
    pub EvtSensorGetDataFieldProperties: Option<PfnGetDataFieldProperties>,
    pub EvtSensorGetDataInterval: Option<PfnGetDataInterval>,
    pub EvtSensorSetDataInterval: Option<PfnSetDataInterval>,
    pub EvtSensorGetDataThresholds: Option<PfnGetDataThresholds>,
    pub EvtSensorSetDataThresholds: Option<PfnSetDataThresholds>,
    pub EvtSensorGetProperties: Option<PfnGetProperties>,
    pub EvtSensorDeviceIoControl: Option<PfnDeviceIoControl>,
    pub EvtSensorStartHistory: Option<PfnStartSensorHistory>,
    pub EvtSensorStopHistory: Option<PfnStopSensorHistory>,
    pub EvtSensorClearHistory: Option<PfnClearSensorHistory>,
    pub EvtSensorStartHistoryRetrieval: Option<PfnStartHistoryRetrieval>,
    pub EvtSensorCancelHistoryRetrieval: Option<PfnCancelHistoryRetrieval>,
    pub EvtSensorSetBatchLatency: Option<PfnSetBatchLatency>,
    pub EvtSensorStartStateChangeNotification: Option<PfnStateChangeNotification>,
    pub EvtSensorStopStateChangeNotification: Option<PfnStateChangeNotification>,
    pub EvtSensorEnableWake: Option<PfnEnableWake>,
    pub EvtSensorDisableWake: Option<PfnDisableWake>,
}

impl Default for SENSOR_CONTROLLER_CONFIG {
    /// `SENSOR_CONTROLLER_CONFIG_INIT`
    fn default() -> Self {
        Self {
            Size: wdf_struct_size!(SENSOR_CONTROLLER_CONFIG),
            DriverIsPowerPolicyOwner: wdk_sys::_WDF_TRI_STATE::WdfUseDefault,
            EvtSensorStart: None,
            EvtSensorStop: None,
            EvtSensorGetSupportedDataFields: None,
            EvtSensorGetDataFieldProperties: None,
            EvtSensorGetDataInterval: None,
            EvtSensorSetDataInterval: None,
            EvtSensorGetDataThresholds: None,
            EvtSensorSetDataThresholds: None,
            EvtSensorGetProperties: None,
            EvtSensorDeviceIoControl: None,
            EvtSensorStartHistory: None,
            EvtSensorStopHistory: None,
            EvtSensorClearHistory: None,
            EvtSensorStartHistoryRetrieval: None,
            EvtSensorCancelHistoryRetrieval: None,
            EvtSensorSetBatchLatency: None,
            EvtSensorStartStateChangeNotification: None,
            EvtSensorStopStateChangeNotification: None,
            EvtSensorEnableWake: None,
            EvtSensorDisableWake: None,
        }
    }
}

/// `SENSOR_CONFIG`
#[repr(C)]
pub struct SENSOR_CONFIG {
    pub Size: ULONG,
    pub pEnumerationList: *mut SENSOR_COLLECTION_LIST,
}

impl SENSOR_CONFIG {
    /// `SENSOR_CONFIG_INIT`, plus the enumeration list.
    pub const fn new(enumeration_list: *mut SENSOR_COLLECTION_LIST) -> Self {
        Self {
            Size: wdf_struct_size!(SENSOR_CONFIG),
            pEnumerationList: enumeration_list,
        }
    }
}

/// `SENSORSCXFUNCENUM`
#[repr(usize)]
#[derive(Clone, Copy)]
enum Function {
    DeviceInitConfig = 0,
    DeviceInitialize = 1,
    SensorCreate = 2,
    SensorInitialize = 3,
    SensorDataReady = 4,
    DeviceGetSensorList = 5,
}
const FUNCTION_TABLE_NUM_ENTRIES: usize = 8;

unsafe extern "C" {
    static SensorscxFunctions: [Option<unsafe extern "C" fn()>; FUNCTION_TABLE_NUM_ENTRIES];
    static SensorscxDriverGlobals: *mut c_void;
}

/// Looks up `function` in the SensorsCx function table, as `F`.
///
/// # Safety
///
/// `F` must be the function's type, with the driver globals as the first
/// parameter.
unsafe fn function<F: Copy>(function: Function) -> (F, *mut c_void) {
    const {
        assert!(size_of::<F>() == size_of::<unsafe extern "C" fn()>());
    }
    // SAFETY: Both are filled in when the WDF stub binds SensorsCx, before
    // DriverEntry, and never change afterwards.
    let (entry, globals) = unsafe {
        (
            SensorscxFunctions[function as usize],
            SensorscxDriverGlobals,
        )
    };
    let entry = entry.expect("SensorsCx is bound before DriverEntry runs");
    // SAFETY: The caller guarantees that F is the right function pointer type.
    (unsafe { core::mem::transmute_copy(&entry) }, globals)
}

/// `SensorsCxDeviceInitConfig`
pub unsafe fn device_init_config(
    device_init: PWDFDEVICE_INIT,
    fdo_attributes: PWDF_OBJECT_ATTRIBUTES,
    flags: ULONG,
) -> NTSTATUS {
    type Pfn = unsafe extern "C" fn(
        *mut c_void,
        PWDFDEVICE_INIT,
        PWDF_OBJECT_ATTRIBUTES,
        ULONG,
    ) -> NTSTATUS;
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::DeviceInitConfig);
        f(globals, device_init, fdo_attributes, flags)
    }
}

/// `SensorsCxDeviceInitialize`
pub unsafe fn device_initialize(
    device: WDFDEVICE,
    config: *mut SENSOR_CONTROLLER_CONFIG,
) -> NTSTATUS {
    type Pfn =
        unsafe extern "C" fn(*mut c_void, WDFDEVICE, *mut SENSOR_CONTROLLER_CONFIG) -> NTSTATUS;
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::DeviceInitialize);
        f(globals, device, config)
    }
}

/// `SensorsCxSensorCreate`
pub unsafe fn sensor_create(
    device: WDFDEVICE,
    attributes: PWDF_OBJECT_ATTRIBUTES,
    sensor: *mut SENSOROBJECT,
) -> NTSTATUS {
    type Pfn = unsafe extern "C" fn(
        *mut c_void,
        WDFDEVICE,
        PWDF_OBJECT_ATTRIBUTES,
        *mut SENSOROBJECT,
    ) -> NTSTATUS;
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::SensorCreate);
        f(globals, device, attributes, sensor)
    }
}

/// `SensorsCxSensorInitialize`
pub unsafe fn sensor_initialize(sensor: SENSOROBJECT, config: *mut SENSOR_CONFIG) -> NTSTATUS {
    type Pfn = unsafe extern "C" fn(*mut c_void, SENSOROBJECT, *mut SENSOR_CONFIG) -> NTSTATUS;
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::SensorInitialize);
        f(globals, sensor, config)
    }
}

/// `SensorsCxSensorDataReady`
pub unsafe fn sensor_data_ready(sensor: SENSOROBJECT, data: *mut SENSOR_COLLECTION_LIST) {
    type Pfn = unsafe extern "C" fn(*mut c_void, SENSOROBJECT, *mut SENSOR_COLLECTION_LIST);
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::SensorDataReady);
        f(globals, sensor, data);
    }
}

/// `SensorsCxDeviceGetSensorList`
pub unsafe fn device_get_sensor_list(
    device: WDFDEVICE,
    sensors: *mut SENSOROBJECT,
    count: *mut ULONG,
) -> NTSTATUS {
    type Pfn =
        unsafe extern "C" fn(*mut c_void, WDFDEVICE, *mut SENSOROBJECT, *mut ULONG) -> NTSTATUS;
    // SAFETY: Signature from SensorsCx.h. Arguments are the caller's.
    unsafe {
        let (f, globals) = function::<Pfn>(Function::DeviceGetSensorList);
        f(globals, device, sensors, count)
    }
}
