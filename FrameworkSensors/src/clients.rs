//! Callbacks from SensorsCx (`EvtSensor*`).

use core::{mem::size_of, ptr};

use wdk::nt_success;
use wdk_sys::{
    NTSTATUS,
    STATUS_BUFFER_TOO_SMALL,
    STATUS_DEVICE_NOT_READY,
    STATUS_INVALID_PARAMETER,
    STATUS_NOT_SUPPORTED,
    STATUS_SUCCESS,
    ULONG,
    WDFREQUEST,
};
use windows_sys::Win32::{
    Devices::Sensors::{
        IsKeyPresentInPropertyList,
        PropKeyFindKeySetPropVariant,
        SensorState_Idle,
        SENSOR_COLLECTION_LIST,
        SENSOR_PROPERTY_LIST,
        SENSOR_VALUE_PAIR,
    },
    Foundation::PROPERTYKEY,
};

use crate::{sensor::Sensor, sensorscx::SENSOROBJECT, trace};

/// Runs `f` with the sensor's context, or fails with
/// `STATUS_INVALID_PARAMETER` if it has none.
fn with_sensor(sensor: SENSOROBJECT, f: impl FnOnce(&Sensor) -> NTSTATUS) -> NTSTATUS {
    Sensor::from_sensor(sensor).map_or_else(
        || {
            trace!("sensor has no context");
            STATUS_INVALID_PARAMETER
        },
        f,
    )
}

/// The `EvtSensorGet*` pattern: stores the size `f` returns in `*size`, or
/// 0 if it fails.
fn sized(
    sensor: SENSOROBJECT,
    size: *mut ULONG,
    f: impl FnOnce(&Sensor) -> Result<ULONG, NTSTATUS>,
) -> NTSTATUS {
    // SAFETY: SensorsCx passes a valid pointer or null.
    let Some(size) = (unsafe { size.as_mut() }) else {
        return STATUS_INVALID_PARAMETER;
    };
    let result = Sensor::from_sensor(sensor)
        .ok_or(STATUS_INVALID_PARAMETER)
        .and_then(f);
    match result {
        Ok(s) => {
            *size = s;
            STATUS_SUCCESS
        }
        Err(status) => {
            *size = 0;
            status
        }
    }
}

/// `OnStart`: starts polling.
pub unsafe extern "C" fn evt_sensor_start(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |s| {
        let mut state = s.lock_state();
        if !state.powered_on {
            trace!("start: sensor is not powered on");
            return STATUS_DEVICE_NOT_READY;
        }
        state.start();
        drop(state);
        s.start_timer(0);
        STATUS_SUCCESS
    })
}

/// `OnStop`: stops polling.
pub unsafe extern "C" fn evt_sensor_stop(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |s| {
        s.lock_state().started = false;
        s.stop_timer();
        s.lock_state().set_sensor_state(SensorState_Idle);
        STATUS_SUCCESS
    })
}

/// `OnGetSupportedDataFields`
pub unsafe extern "C" fn evt_sensor_get_supported_data_fields(
    sensor: SENSOROBJECT,
    fields: *mut SENSOR_PROPERTY_LIST,
    size: *mut ULONG,
) -> NTSTATUS {
    // SAFETY: SensorsCx passes a valid list or null.
    sized(sensor, size, |s| unsafe {
        s.supported_data_fields.copy_to(fields)
    })
}

/// `OnGetProperties`
pub unsafe extern "C" fn evt_sensor_get_properties(
    sensor: SENSOROBJECT,
    properties: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS {
    // SAFETY: SensorsCx passes a valid list or null.
    sized(sensor, size, |s| unsafe {
        s.lock_state().properties.copy_to(properties)
    })
}

/// `OnGetDataFieldProperties`: the same range and resolution for every
/// data field.
pub unsafe extern "C" fn evt_sensor_get_data_field_properties(
    sensor: SENSOROBJECT,
    data_field: *const PROPERTYKEY,
    properties: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS {
    sized(sensor, size, |s| {
        if data_field.is_null() {
            return Err(STATUS_INVALID_PARAMETER);
        }
        // SAFETY: Both are valid.
        if !unsafe { IsKeyPresentInPropertyList(s.supported_data_fields.as_ptr(), data_field) } {
            trace!("no properties for this data field");
            return Err(STATUS_NOT_SUPPORTED);
        }
        // SAFETY: SensorsCx passes a valid list or null.
        unsafe { s.data_field_properties.copy_to(properties) }
    })
}

/// `OnGetDataInterval`
pub unsafe extern "C" fn evt_sensor_get_data_interval(
    sensor: SENSOROBJECT,
    data_rate_ms: *mut ULONG,
) -> NTSTATUS {
    with_sensor(sensor, |s| {
        // SAFETY: SensorsCx passes a valid pointer or null.
        let Some(data_rate_ms) = (unsafe { data_rate_ms.as_mut() }) else {
            return STATUS_INVALID_PARAMETER;
        };
        *data_rate_ms = s.lock_state().interval_ms;
        STATUS_SUCCESS
    })
}

/// `OnSetDataInterval`: restarts polling at the new interval if running.
pub unsafe extern "C" fn evt_sensor_set_data_interval(
    sensor: SENSOROBJECT,
    data_rate_ms: ULONG,
) -> NTSTATUS {
    if data_rate_ms == 0 {
        return STATUS_INVALID_PARAMETER;
    }
    with_sensor(sensor, |s| {
        let was_started = {
            let mut state = s.lock_state();
            state.interval_ms = data_rate_ms;
            core::mem::replace(&mut state.started, false)
        };
        if was_started {
            s.stop_timer();
            s.lock_state().start();
            s.start_timer(0);
        }
        STATUS_SUCCESS
    })
}

/// `OnGetDataThresholds`
pub unsafe extern "C" fn evt_sensor_get_data_thresholds(
    sensor: SENSOROBJECT,
    thresholds: *mut SENSOR_COLLECTION_LIST,
    size: *mut ULONG,
) -> NTSTATUS {
    // SAFETY: SensorsCx passes a valid list or null.
    sized(sensor, size, |s| unsafe {
        s.lock_state().thresholds.copy_to(thresholds)
    })
}

/// `OnSetDataThresholds`
pub unsafe extern "C" fn evt_sensor_set_data_thresholds(
    sensor: SENSOROBJECT,
    thresholds: *mut SENSOR_COLLECTION_LIST,
) -> NTSTATUS {
    if thresholds.is_null() {
        return STATUS_INVALID_PARAMETER;
    }
    with_sensor(sensor, |s| {
        // SAFETY: SensorsCx passes a valid list.
        let (allocated, count) =
            unsafe { ((*thresholds).AllocatedSizeInBytes, (*thresholds).Count) };
        let header = size_of::<SENSOR_COLLECTION_LIST>() - size_of::<SENSOR_VALUE_PAIR>();
        let capacity = (allocated as usize).saturating_sub(header) / size_of::<SENSOR_VALUE_PAIR>();
        if count as usize > capacity {
            return STATUS_INVALID_PARAMETER;
        }

        let mut state = s.lock_state();
        // SAFETY: The list holds `count` entries, checked against its size.
        let first = unsafe { ptr::addr_of!((*thresholds).List).cast::<SENSOR_VALUE_PAIR>() };
        for i in 0..count as usize {
            // SAFETY: i < count.
            let pair = unsafe { &*first.add(i) };
            // SAFETY: Both lists are valid, key and value point into pair.
            let status = unsafe {
                PropKeyFindKeySetPropVariant(
                    state.thresholds.as_mut_ptr(),
                    &raw const pair.Key,
                    true,
                    &raw const pair.Value,
                )
            };
            if !nt_success(status) {
                trace!("no threshold for this data field {status:#010X}");
                return STATUS_INVALID_PARAMETER;
            }
        }

        match state.update_cached_thresholds() {
            Ok(()) => STATUS_SUCCESS,
            Err(status) => status,
        }
    })
}

/// `OnIoControl`: no private IOCTLs.
pub const unsafe extern "C" fn evt_sensor_device_io_control(
    _sensor: SENSOROBJECT,
    _request: WDFREQUEST,
    _output_buffer_length: usize,
    _input_buffer_length: usize,
    _io_control_code: ULONG,
) -> NTSTATUS {
    STATUS_NOT_SUPPORTED
}

// History and wake are not supported.

pub unsafe extern "C" fn evt_sensor_start_history(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}

pub unsafe extern "C" fn evt_sensor_stop_history(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}

pub unsafe extern "C" fn evt_sensor_clear_history(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}

pub unsafe extern "C" fn evt_sensor_start_history_retrieval(
    sensor: SENSOROBJECT,
    _history: *mut SENSOR_COLLECTION_LIST,
    history_size_in_bytes: ULONG,
) -> NTSTATUS {
    with_sensor(sensor, |_| {
        if (history_size_in_bytes as usize) < size_of::<SENSOR_COLLECTION_LIST>() {
            return STATUS_BUFFER_TOO_SMALL;
        }
        STATUS_NOT_SUPPORTED
    })
}

pub unsafe extern "C" fn evt_sensor_cancel_history_retrieval(
    sensor: SENSOROBJECT,
    _bytes_written: *mut ULONG,
) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}

pub unsafe extern "C" fn evt_sensor_enable_wake(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}

pub unsafe extern "C" fn evt_sensor_disable_wake(sensor: SENSOROBJECT) -> NTSTATUS {
    with_sensor(sensor, |_| STATUS_NOT_SUPPORTED)
}
