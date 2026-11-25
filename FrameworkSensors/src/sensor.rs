//! A SensorsCx sensor polled from the EC: what all sensor types share.
//!
//! A passive level WDF timer polls the EC memory map every data interval,
//! and readings that pass the software thresholds are pushed to SensorsCx.
//! The sensor types ([`crate::accelerometer`], [`crate::als`]) provide the
//! property lists, the EC reads and the thresholds.

use core::{mem::MaybeUninit, ptr};
use std::sync::{Mutex, MutexGuard, PoisonError};

use sensors_core::schedule::Schedule;
use wdk::nt_success;
use wdk_sys::{
    call_unsafe_wdf_function_binding,
    _WDF_EXECUTION_LEVEL,
    NTSTATUS,
    PCWDF_OBJECT_CONTEXT_TYPE_INFO,
    STATUS_INVALID_HANDLE,
    WDFOBJECT,
    WDFTIMER,
    WDF_OBJECT_ATTRIBUTES,
    WDF_OBJECT_CONTEXT_TYPE_INFO,
    WDF_TIMER_CONFIG,
};
use windows_sys::Win32::{
    Devices::Sensors::{
        GetPerformanceTime,
        SensorState_Active,
        SENSOR_COLLECTION_LIST,
        SENSOR_STATE,
    },
    Foundation::FILETIME,
    System::SystemInformation::GetSystemTimePreciseAsFileTime,
};

#[cfg(feature = "accelerometer")]
use crate::accelerometer;
#[cfg(feature = "als")]
use crate::als;
use crate::{
    collections::{pv_u32, CollectionList, PropertyList},
    device::object_attributes,
    ec::CrosEc,
    sensorscx::{self, SENSOROBJECT},
    trace,
    wdf_object_context::{wdf_declare_context_type_with_name, wdf_get_context_type_info},
    wdf_struct_size,
};

#[cfg(not(any(feature = "accelerometer", feature = "als")))]
compile_error!("enable at least one sensor feature: accelerometer, als");

/// The sensor types this build exposes, in creation order.
#[derive(Debug, Clone, Copy)]
pub enum KindId {
    #[cfg(feature = "accelerometer")]
    Accelerometer,
    #[cfg(feature = "als")]
    Als,
}

pub const SENSORS: &[KindId] = &[
    #[cfg(feature = "accelerometer")]
    KindId::Accelerometer,
    #[cfg(feature = "als")]
    KindId::Als,
];

/// Sensor type specific state.
pub enum Kind {
    #[cfg(feature = "accelerometer")]
    Accelerometer(accelerometer::State),
    #[cfg(feature = "als")]
    Als(als::State),
}

/// A reading to report.
pub enum Sample {
    #[cfg(feature = "accelerometer")]
    Accelerometer(sensors_core::accel::Axes),
    #[cfg(feature = "als")]
    Als(f32),
}

impl Kind {
    const fn name(&self) -> &'static str {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(_) => "accelerometer",
            #[cfg(feature = "als")]
            Self::Als(_) => "ALS",
        }
    }

    /// Report the next reading regardless of thresholds.
    const fn restart(&mut self) {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(s) => s.filter.restart(),
            #[cfg(feature = "als")]
            Self::Als(s) => s.filter.restart(),
        }
    }

    const fn is_first_sample(&self) -> bool {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(s) => s.filter.is_first_sample(),
            #[cfg(feature = "als")]
            Self::Als(s) => s.filter.is_first_sample(),
        }
    }

    /// Reads from the EC. `Ok(None)` if the reading doesn't pass the
    /// thresholds.
    fn read(&mut self, ec: &mut CrosEc) -> Result<Option<Sample>, NTSTATUS> {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(s) => s.read(ec),
            #[cfg(feature = "als")]
            Self::Als(s) => s.read(ec),
        }
    }

    /// Reloads the cached thresholds from `thresholds`.
    fn update_thresholds(&mut self, thresholds: &CollectionList) -> Result<(), NTSTATUS> {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(s) => s.update_thresholds(thresholds),
            #[cfg(feature = "als")]
            Self::Als(s) => s.update_thresholds(thresholds),
        }
    }
}

impl Sample {
    /// Fills in the data list. The timestamp is set by the caller.
    fn write(self, data: &mut CollectionList) {
        match self {
            #[cfg(feature = "accelerometer")]
            Self::Accelerometer(axes) => accelerometer::write_data(axes, data),
            #[cfg(feature = "als")]
            Self::Als(lux) => als::write_data(lux, data),
        }
    }
}

/// What a sensor type sets up at creation.
pub struct Description {
    /// `DEVPKEY_Sensor_*`, for `SensorsCxSensorInitialize`.
    pub enumeration: CollectionList,
    pub supported_data_fields: PropertyList,
    pub data_field_properties: CollectionList,
    /// The data list. `data_timestamp` is the index of the timestamp.
    pub data: CollectionList,
    pub data_timestamp: usize,
    /// Common properties; the state must be the first.
    pub properties: CollectionList,
    pub thresholds: CollectionList,
    pub interval_ms: u32,
    pub kind: Kind,
}

/// Index of `PKEY_Sensor_State` in [`Description::properties`].
const PROPERTY_STATE: usize = 0;

/// WDF context of a sensor object. All zeroes (as allocated by WDF) is
/// valid and means not initialized yet.
pub struct SensorContext {
    initialized: bool,
    sensor: MaybeUninit<Sensor>,
}
wdf_declare_context_type_with_name!(SensorContext, sensor_get_context);

pub fn context_type_info() -> PCWDF_OBJECT_CONTEXT_TYPE_INFO {
    wdf_get_context_type_info!(SensorContext)
}

/// Drops the context when the sensor object goes away.
pub extern "C" fn evt_destroy(object: WDFOBJECT) {
    // SAFETY: Only sensor objects get this callback, and they have a
    // SensorContext.
    let context = unsafe { sensor_get_context(object) };
    // SAFETY: Nothing else can access the context once the object is being
    // destroyed.
    if let Some(context) = unsafe { context.as_mut() } {
        if context.initialized {
            context.initialized = false;
            // SAFETY: initialized says it holds a value.
            unsafe { context.sensor.assume_init_drop() };
        }
    }
}

pub struct Sensor {
    object: SENSOROBJECT,
    timer: WDFTIMER,

    /// Passed to SensorsCx at initialization, which may keep the pointer.
    #[allow(dead_code, reason = "only kept alive")]
    enumeration: CollectionList,
    pub supported_data_fields: PropertyList,
    pub data_field_properties: CollectionList,

    state: Mutex<State>,
    /// The reading pushed to SensorsCx. Separate from `state` so that
    /// SensorsCx isn't called with the state locked.
    data: Mutex<CollectionList>,
    data_timestamp: usize,
}

pub struct State {
    pub properties: CollectionList,
    pub thresholds: CollectionList,

    pub powered_on: bool,
    pub started: bool,
    pub interval_ms: u32,

    /// Opened on demand, closed when leaving D0.
    pub ec: Option<CrosEc>,
    schedule: Schedule,
    kind: Kind,
}

/// `GetPerformanceTime`
fn performance_time_ms() -> Option<u32> {
    let mut ms = 0;
    // SAFETY: ms is a valid out parameter.
    nt_success(unsafe { GetPerformanceTime(&raw mut ms) }).then_some(ms)
}

pub fn now() -> FILETIME {
    let mut time = FILETIME::default();
    // SAFETY: time is a valid out parameter.
    unsafe { GetSystemTimePreciseAsFileTime(&raw mut time) };
    time
}

pub fn open_ec() -> Option<CrosEc> {
    CrosEc::open()
        .map_err(|e| trace!("Opening the EC failed: {e}"))
        .ok()
}

impl Sensor {
    /// `ComboDevice::Initialize`: sets up `sensor`'s context as a sensor of
    /// type `id`.
    ///
    /// Returns the enumeration list for `SensorsCxSensorInitialize`.
    pub fn create(
        sensor: SENSOROBJECT,
        id: KindId,
    ) -> Result<*mut SENSOR_COLLECTION_LIST, NTSTATUS> {
        #[cfg_attr(not(feature = "accelerometer"), allow(unused_mut))]
        let mut ec = open_ec();
        let description = match id {
            #[cfg(feature = "accelerometer")]
            KindId::Accelerometer => accelerometer::describe(ec.as_mut())?,
            #[cfg(feature = "als")]
            KindId::Als => als::describe()?,
        };

        let timer = create_timer(sensor)?;

        let Description {
            mut enumeration,
            supported_data_fields,
            data_field_properties,
            data,
            data_timestamp,
            properties,
            thresholds,
            interval_ms,
            kind,
        } = description;
        // A heap pointer, so moving the list doesn't change it.
        let enumeration_list = enumeration.as_mut_ptr();

        let sensor_object = sensor;
        let sensor = Self {
            object: sensor_object,
            timer,
            enumeration,
            supported_data_fields,
            data_field_properties,
            state: Mutex::new(State {
                properties,
                thresholds,
                powered_on: false,
                started: false,
                interval_ms,
                ec,
                schedule: Schedule::default(),
                kind,
            }),
            data: Mutex::new(data),
            data_timestamp,
        };

        // SAFETY: The sensor object was created with a SensorContext.
        let context = unsafe { &mut *sensor_get_context(sensor_object) };
        context.sensor.write(sensor);
        context.initialized = true;

        Ok(enumeration_list)
    }

    /// The context of `sensor`, if it has been initialized.
    pub fn from_sensor<'a>(sensor: SENSOROBJECT) -> Option<&'a Self> {
        if sensor.is_null() {
            return None;
        }
        // SAFETY: All sensor objects are created with a SensorContext. It
        // lives until the sensor object is destroyed, after the last
        // callback for it.
        let context = unsafe { sensor_get_context(sensor).as_ref()? };
        // SAFETY: initialized says it holds a value.
        context
            .initialized
            .then(|| unsafe { context.sensor.assume_init_ref() })
    }

    pub fn lock_state(&self) -> MutexGuard<'_, State> {
        // Panics abort the process, so the lock can't actually be poisoned.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts polling after `due_ms` milliseconds.
    pub fn start_timer(&self, due_ms: u64) {
        // WDF_REL_TIMEOUT_IN_MS
        let due_time =
            i64::try_from(due_ms.saturating_mul(10_000)).map_or(i64::MIN, i64::wrapping_neg);
        // Returns whether the timer was already queued, which is fine either
        // way. SAFETY: The timer lives as long as the sensor.
        let _ = unsafe { call_unsafe_wdf_function_binding!(WdfTimerStart, self.timer, due_time) };
    }

    /// Stops polling and waits for a running timer callback to finish.
    pub fn stop_timer(&self) {
        // SAFETY: The timer lives as long as the sensor. Never called from
        // the timer callback.
        let _ = unsafe { call_unsafe_wdf_function_binding!(WdfTimerStop, self.timer, 1) };
    }

    /// Before the sensor is deleted.
    pub fn release(&self) {
        self.stop_timer();
        self.lock_state().ec = None;
    }

    /// `OnTimerExpire`: takes a reading, reports it if it passes the
    /// thresholds, and schedules the next one.
    fn on_timer(&self) {
        let sample = self.lock_state().read_sample();
        match sample {
            Ok(Some(sample)) => self.report(sample),
            Ok(None) => {}
            Err(status) => trace!("Reading the sensor failed {status:#010X}"),
        }

        // Decided under the lock so that a concurrent stop either sees the
        // timer armed (and cancels it) or makes us not re-arm it.
        let mut state = self.lock_state();
        if state.powered_on && state.started {
            let interval_ms = state.interval_ms;
            let due_ms = state
                .schedule
                .next_due_ms(interval_ms, performance_time_ms());
            self.start_timer(due_ms);
        }
        drop(state);
    }

    /// Pushes `sample` to SensorsCx.
    fn report(&self, sample: Sample) {
        let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
        sample.write(&mut data);
        data.set(self.data_timestamp, crate::collections::pv_filetime(now()));
        // SAFETY: The sensor is initialized, data is a valid list.
        unsafe { sensorscx::sensor_data_ready(self.object, data.as_mut_ptr()) };
    }
}

impl State {
    pub fn set_sensor_state(&mut self, state: SENSOR_STATE) {
        #[allow(clippy::cast_sign_loss, reason = "SENSOR_STATE values are 0..=3")]
        self.properties.set(PROPERTY_STATE, pv_u32(state as u32));
    }

    /// Activates the sensor: the next reading is reported regardless of
    /// thresholds.
    pub fn start(&mut self) {
        self.kind.restart();
        self.started = true;
        self.set_sensor_state(SensorState_Active);
    }

    /// `GetData`: takes a reading. `Ok(None)` if it doesn't pass the
    /// thresholds.
    fn read_sample(&mut self) -> Result<Option<Sample>, NTSTATUS> {
        if self.ec.is_none() {
            self.ec = open_ec();
        }
        let ec = self.ec.as_mut().ok_or(STATUS_INVALID_HANDLE)?;

        let first_sample = self.kind.is_first_sample();
        let sample = self.kind.read(ec).inspect_err(|status| {
            trace!("Reading the {} failed {status:#010X}", self.kind.name());
        })?;
        if first_sample {
            self.schedule.restart(performance_time_ms());
        }
        Ok(sample)
    }

    /// `UpdateCachedThreshold`
    pub fn update_cached_thresholds(&mut self) -> Result<(), NTSTATUS> {
        self.kind
            .update_thresholds(&self.thresholds)
            .inspect_err(|status| trace!("Reading the thresholds failed {status:#010X}"))
    }
}

/// Creates the polling timer, a child of `sensor`.
fn create_timer(sensor: SENSOROBJECT) -> Result<WDFTIMER, NTSTATUS> {
    // WDF_TIMER_CONFIG_INIT
    let mut config = WDF_TIMER_CONFIG {
        Size: wdf_struct_size!(WDF_TIMER_CONFIG),
        EvtTimerFunc: Some(evt_timer),
        AutomaticSerialization: 1,
        TolerableDelay: 0,
        ..WDF_TIMER_CONFIG::default()
    };
    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        ParentObject: sensor,
        ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelPassive,
        ..object_attributes()
    };
    let mut timer: WDFTIMER = ptr::null_mut();
    // SAFETY: All pointers are valid locals.
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfTimerCreate,
            &raw mut config,
            &raw mut attributes,
            &raw mut timer
        )
    };
    if !nt_success(status) {
        trace!("WdfTimerCreate failed {status:#010X}");
        return Err(status);
    }
    Ok(timer)
}

extern "C" fn evt_timer(timer: WDFTIMER) {
    // SAFETY: timer comes from the framework.
    let sensor = unsafe { call_unsafe_wdf_function_binding!(WdfTimerGetParentObject, timer) };
    match Sensor::from_sensor(sensor) {
        Some(sensor) => sensor.on_timer(),
        None => trace!("timer fired without a sensor context"),
    }
}
