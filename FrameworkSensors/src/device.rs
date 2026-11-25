//! Device creation and `PnP` / power callbacks.

use core::ptr;

use wdk::nt_success;
use wdk_sys::{
    call_unsafe_wdf_function_binding,
    _WDF_EXECUTION_LEVEL,
    _WDF_SYNCHRONIZATION_SCOPE,
    NTSTATUS,
    PWDFDEVICE_INIT,
    STATUS_INVALID_PARAMETER,
    STATUS_SUCCESS,
    ULONG,
    WDFCMRESLIST,
    WDFDEVICE,
    WDF_OBJECT_ATTRIBUTES,
    WDF_PNPPOWER_EVENT_CALLBACKS,
    WDF_POWER_DEVICE_STATE,
};
use windows_sys::Win32::Devices::Sensors::SensorState_Idle;

use crate::{
    clients,
    sensor::{self, KindId, Sensor, SENSORS},
    sensorscx::{self, SENSOROBJECT, SENSOR_CONFIG, SENSOR_CONTROLLER_CONFIG},
    trace,
    wdf_struct_size,
};

/// Upper bound for the sensor list SensorsCx returns.
const SENSOR_COUNT: usize = SENSORS.len();

/// `WDF_OBJECT_ATTRIBUTES_INIT`
pub fn object_attributes() -> WDF_OBJECT_ATTRIBUTES {
    WDF_OBJECT_ATTRIBUTES {
        Size: wdf_struct_size!(WDF_OBJECT_ATTRIBUTES),
        ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
        SynchronizationScope: _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
        ..WDF_OBJECT_ATTRIBUTES::default()
    }
}

/// `OnDeviceAdd`: creates the device and registers it with SensorsCx.
pub fn create(mut device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    let mut fdo_attributes = object_attributes();

    // SAFETY: device_init comes from the framework.
    let status = unsafe { sensorscx::device_init_config(device_init, &raw mut fdo_attributes, 0) };
    if !nt_success(status) {
        trace!("SensorsCxDeviceInitConfig failed {status:#010X}");
        return status;
    }

    let mut callbacks = WDF_PNPPOWER_EVENT_CALLBACKS {
        Size: wdf_struct_size!(WDF_PNPPOWER_EVENT_CALLBACKS),
        EvtDevicePrepareHardware: Some(evt_device_prepare_hardware),
        EvtDeviceReleaseHardware: Some(evt_device_release_hardware),
        EvtDeviceD0Entry: Some(evt_device_d0_entry),
        EvtDeviceD0Exit: Some(evt_device_d0_exit),
        ..WDF_PNPPOWER_EVENT_CALLBACKS::default()
    };
    // SAFETY: device_init comes from the framework, callbacks is a valid
    // local.
    unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceInitSetPnpPowerEventCallbacks,
            device_init,
            &raw mut callbacks
        );
    }

    let mut device: WDFDEVICE = ptr::null_mut();
    // SAFETY: All pointers are valid locals.
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            &raw mut device_init,
            &raw mut fdo_attributes,
            &raw mut device,
        )
    };
    if !nt_success(status) {
        trace!("WdfDeviceCreate failed {status:#010X}");
        return status;
    }

    let mut config = SENSOR_CONTROLLER_CONFIG {
        EvtSensorStart: Some(clients::evt_sensor_start),
        EvtSensorStop: Some(clients::evt_sensor_stop),
        EvtSensorGetSupportedDataFields: Some(clients::evt_sensor_get_supported_data_fields),
        EvtSensorGetDataInterval: Some(clients::evt_sensor_get_data_interval),
        EvtSensorSetDataInterval: Some(clients::evt_sensor_set_data_interval),
        EvtSensorGetDataFieldProperties: Some(clients::evt_sensor_get_data_field_properties),
        EvtSensorGetDataThresholds: Some(clients::evt_sensor_get_data_thresholds),
        EvtSensorSetDataThresholds: Some(clients::evt_sensor_set_data_thresholds),
        EvtSensorGetProperties: Some(clients::evt_sensor_get_properties),
        EvtSensorDeviceIoControl: Some(clients::evt_sensor_device_io_control),
        EvtSensorStartHistory: Some(clients::evt_sensor_start_history),
        EvtSensorStopHistory: Some(clients::evt_sensor_stop_history),
        EvtSensorClearHistory: Some(clients::evt_sensor_clear_history),
        EvtSensorStartHistoryRetrieval: Some(clients::evt_sensor_start_history_retrieval),
        EvtSensorCancelHistoryRetrieval: Some(clients::evt_sensor_cancel_history_retrieval),
        EvtSensorEnableWake: Some(clients::evt_sensor_enable_wake),
        EvtSensorDisableWake: Some(clients::evt_sensor_disable_wake),
        ..SENSOR_CONTROLLER_CONFIG::default()
    };
    // SAFETY: device was just created, config is a valid local.
    let status = unsafe { sensorscx::device_initialize(device, &raw mut config) };
    if !nt_success(status) {
        trace!("SensorsCxDeviceInitialize failed {status:#010X}");
    }
    status
}

/// Calls `f` for each sensor SensorsCx has for `device`.
fn for_each_sensor(device: WDFDEVICE, mut f: impl FnMut(SENSOROBJECT) -> NTSTATUS) -> NTSTATUS {
    let mut sensors: [SENSOROBJECT; SENSOR_COUNT] = [ptr::null_mut(); SENSOR_COUNT];
    let mut count = ULONG::try_from(SENSOR_COUNT).expect("SENSOR_COUNT is small");
    // SAFETY: sensors has room for count entries.
    let status =
        unsafe { sensorscx::device_get_sensor_list(device, sensors.as_mut_ptr(), &raw mut count) };
    if !nt_success(status) {
        trace!("SensorsCxDeviceGetSensorList failed {status:#010X}");
        return STATUS_INVALID_PARAMETER;
    }

    let count = usize::try_from(count).map_or(SENSOR_COUNT, |c| c.min(SENSOR_COUNT));
    for &sensor in &sensors[..count] {
        let status = f(sensor);
        if !nt_success(status) {
            return status;
        }
    }
    STATUS_SUCCESS
}

/// `OnPrepareHardware`: creates the sensors and registers them with
/// SensorsCx.
extern "C" fn evt_device_prepare_hardware(
    device: WDFDEVICE,
    _resources_raw: WDFCMRESLIST,
    _resources_translated: WDFCMRESLIST,
) -> NTSTATUS {
    for &id in SENSORS {
        let status = create_sensor(device, id);
        if !nt_success(status) {
            return status;
        }
    }
    STATUS_SUCCESS
}

fn create_sensor(device: WDFDEVICE, id: KindId) -> NTSTATUS {
    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        ContextTypeInfo: sensor::context_type_info(),
        EvtDestroyCallback: Some(sensor::evt_destroy),
        ..object_attributes()
    };
    let mut sensor: SENSOROBJECT = ptr::null_mut();
    // SAFETY: device comes from the framework, the rest are valid locals.
    let status = unsafe { sensorscx::sensor_create(device, &raw mut attributes, &raw mut sensor) };
    if !nt_success(status) {
        trace!("SensorsCxSensorCreate failed {status:#010X}");
        return status;
    }

    let enumeration_list = match Sensor::create(sensor, id) {
        Ok(list) => list,
        Err(status) => {
            trace!("{id:?} initialization failed {status:#010X}");
            return status;
        }
    };

    let mut config = SENSOR_CONFIG::new(enumeration_list);
    // SAFETY: sensor was just created; the enumeration list lives as long as
    // the sensor.
    let status = unsafe { sensorscx::sensor_initialize(sensor, &raw mut config) };
    if !nt_success(status) {
        trace!("SensorsCxSensorInitialize failed {status:#010X}");
    }
    status
}

/// `OnReleaseHardware`: deletes the sensors. Also called if
/// `EvtDevicePrepareHardware` failed.
extern "C" fn evt_device_release_hardware(
    device: WDFDEVICE,
    _resources_translated: WDFCMRESLIST,
) -> NTSTATUS {
    for_each_sensor(device, |sensor| {
        if let Some(s) = Sensor::from_sensor(sensor) {
            s.release();
        }
        // Deleting the sensor deletes its timer and runs evt_destroy, which
        // drops the context, closing the EC handle.
        // SAFETY: sensor comes from SensorsCx and is not used afterwards.
        unsafe { call_unsafe_wdf_function_binding!(WdfObjectDelete, sensor) };
        STATUS_SUCCESS
    })
}

fn set_power(device: WDFDEVICE, powered_on: bool) -> NTSTATUS {
    for_each_sensor(device, |sensor| {
        let Some(s) = Sensor::from_sensor(sensor) else {
            trace!("sensor has no context");
            return STATUS_INVALID_PARAMETER;
        };
        let mut state = s.lock_state();
        if !powered_on {
            // Reconnect after resume, the handle may be stale.
            state.ec = None;
        }
        state.powered_on = powered_on;
        state.set_sensor_state(SensorState_Idle);
        STATUS_SUCCESS
    })
}

/// `OnD0Entry`
extern "C" fn evt_device_d0_entry(
    device: WDFDEVICE,
    _previous_state: WDF_POWER_DEVICE_STATE,
) -> NTSTATUS {
    set_power(device, true)
}

/// `OnD0Exit`
extern "C" fn evt_device_d0_exit(
    device: WDFDEVICE,
    _target_state: WDF_POWER_DEVICE_STATE,
) -> NTSTATUS {
    set_power(device, false)
}
