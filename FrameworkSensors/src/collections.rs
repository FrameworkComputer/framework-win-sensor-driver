//! `SENSOR_COLLECTION_LIST` / `SENSOR_PROPERTY_LIST` buffers, `PROPVARIANT`
//! constructors, and the property keys from `sensorsdef.h` that
//! `windows-sys` doesn't have.

#![allow(non_upper_case_globals, reason = "names from sensorsdef.h")]

use core::{
    marker::PhantomData,
    mem::{align_of, size_of},
    ptr::{self, NonNull},
};
use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};

use wdk::nt_success;
use wdk_sys::{NTSTATUS, STATUS_INSUFFICIENT_RESOURCES};
use windows_sys::{
    core::GUID,
    Win32::{
        Devices::Sensors::{
            CollectionsListCopyAndMarshall,
            CollectionsListGetMarshalledSize,
            PropKeyFindKeyGetFloat,
            PropertiesListCopy,
            SENSOR_COLLECTION_LIST,
            SENSOR_PROPERTY_LIST,
            SENSOR_VALUE_PAIR,
        },
        Foundation::{FILETIME, PROPERTYKEY},
        System::{
            Com::{
                CoTaskMemAlloc,
                StructuredStorage::{PropVariantClear, PROPVARIANT},
            },
            Variant::{VARENUM, VT_BOOL, VT_CLSID, VT_FILETIME, VT_LPWSTR, VT_R4, VT_UI4},
        },
    },
};

const fn sensor_property(pid: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID::from_u128(0xD424_7382_969D_4F24_BB14_FB96_7187_0BBF),
        pid,
    }
}

const fn sensor_data(pid: u32) -> PROPERTYKEY {
    PROPERTYKEY {
        fmtid: GUID::from_u128(0xC458_F8A7_4AE8_4777_9607_2E9B_DD65_110A),
        pid,
    }
}

pub const DEVPKEY_Sensor_Type: PROPERTYKEY = sensor_property(2);
pub const DEVPKEY_Sensor_ConnectionType: PROPERTYKEY = sensor_property(4);
pub const DEVPKEY_Sensor_IsPrimary: PROPERTYKEY = sensor_property(5);
pub const DEVPKEY_Sensor_Manufacturer: PROPERTYKEY = sensor_property(7);
pub const DEVPKEY_Sensor_Model: PROPERTYKEY = sensor_property(8);
pub const DEVPKEY_Sensor_PersistentUniqueId: PROPERTYKEY = sensor_property(9);
#[cfg(feature = "als")]
pub const DEVPKEY_LightSensor_AutoBrightnessPreferred: PROPERTYKEY = sensor_property(31);

pub const PKEY_Sensor_Type: PROPERTYKEY = sensor_property(2);
pub const PKEY_Sensor_State: PROPERTYKEY = sensor_property(20);
pub const PKEY_Sensor_MinimumDataInterval_Ms: PROPERTYKEY = sensor_property(21);
pub const PKEY_Sensor_MaximumDataFieldSize_Bytes: PROPERTYKEY = sensor_property(22);
#[cfg(feature = "als")]
pub const PKEY_LightSensor_ResponseCurve: PROPERTYKEY = sensor_property(30);

pub const PKEY_SensorDataField_Resolution: PROPERTYKEY = sensor_property(50);
pub const PKEY_SensorDataField_RangeMinimum: PROPERTYKEY = sensor_property(51);
pub const PKEY_SensorDataField_RangeMaximum: PROPERTYKEY = sensor_property(52);

pub const PKEY_SensorData_Timestamp: PROPERTYKEY = sensor_data(2);
#[cfg(feature = "als")]
pub const PKEY_SensorData_IsValid: PROPERTYKEY = sensor_data(3);
#[cfg(feature = "accelerometer")]
pub const PKEY_SensorData_AccelerationX_Gs: PROPERTYKEY = sensor_data(10);
#[cfg(feature = "accelerometer")]
pub const PKEY_SensorData_AccelerationY_Gs: PROPERTYKEY = sensor_data(11);
#[cfg(feature = "accelerometer")]
pub const PKEY_SensorData_AccelerationZ_Gs: PROPERTYKEY = sensor_data(12);
#[cfg(feature = "accelerometer")]
pub const PKEY_SensorData_Shake: PROPERTYKEY = sensor_data(33);
#[cfg(feature = "als")]
pub const PKEY_SensorData_LightLevel_Lux: PROPERTYKEY = sensor_data(60);
#[cfg(feature = "als")]
pub const PKEY_SensorData_LightLevel_Lux_Threshold_AbsoluteDifference: PROPERTYKEY =
    sensor_data(64);

const VARIANT_TRUE: i16 = -1;

fn propvariant(vt: VARENUM, set: impl FnOnce(&mut PROPVARIANT)) -> PROPVARIANT {
    // SAFETY: PROPVARIANT is plain data; all zeroes is VT_EMPTY.
    let mut pv: PROPVARIANT = unsafe { core::mem::zeroed() };
    pv.Anonymous.Anonymous.vt = vt;
    set(&mut pv);
    pv
}

/// `InitPropVariantFromUInt32`
pub fn pv_u32(value: u32) -> PROPVARIANT {
    propvariant(VT_UI4, |pv| pv.Anonymous.Anonymous.Anonymous.ulVal = value)
}

/// `InitPropVariantFromFloat`
pub fn pv_f32(value: f32) -> PROPVARIANT {
    propvariant(VT_R4, |pv| pv.Anonymous.Anonymous.Anonymous.fltVal = value)
}

/// `InitPropVariantFromBoolean`
pub fn pv_bool(value: bool) -> PROPVARIANT {
    propvariant(VT_BOOL, |pv| {
        pv.Anonymous.Anonymous.Anonymous.boolVal = if value { VARIANT_TRUE } else { 0 };
    })
}

/// `InitPropVariantFromFileTime`
pub fn pv_filetime(value: FILETIME) -> PROPVARIANT {
    propvariant(VT_FILETIME, |pv| {
        pv.Anonymous.Anonymous.Anonymous.filetime = value;
    })
}

/// Copies `values` into a new `CoTaskMemAlloc` allocation.
fn co_task_mem_copy<T: Copy>(values: &[T]) -> Result<*mut T, NTSTATUS> {
    let bytes = size_of_val(values);
    // SAFETY: Plain allocation; checked for null below.
    let p = unsafe { CoTaskMemAlloc(bytes) }.cast::<T>();
    if p.is_null() {
        return Err(STATUS_INSUFFICIENT_RESOURCES);
    }
    // SAFETY: p is a fresh allocation of `bytes` bytes, suitably aligned for
    // anything a PROPVARIANT points to.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), p, values.len()) };
    Ok(p)
}

/// `InitPropVariantFromCLSID`
pub fn pv_clsid(value: &GUID) -> Result<PROPVARIANT, NTSTATUS> {
    let p = co_task_mem_copy(core::slice::from_ref(value))?;
    Ok(propvariant(VT_CLSID, |pv| {
        pv.Anonymous.Anonymous.Anonymous.puuid = p;
    }))
}

/// `InitPropVariantFromString`
pub fn pv_string(value: &str) -> Result<PROPVARIANT, NTSTATUS> {
    let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let p = co_task_mem_copy(&wide)?;
    Ok(propvariant(VT_LPWSTR, |pv| {
        pv.Anonymous.Anonymous.Anonymous.pwszVal = p;
    }))
}

/// `InitPropVariantFromUInt32Vector`
#[cfg(feature = "als")]
pub fn pv_u32_vector(values: &[u32]) -> Result<PROPVARIANT, NTSTATUS> {
    use windows_sys::Win32::System::{Com::StructuredStorage::CAUL, Variant::VT_VECTOR};

    let p = co_task_mem_copy(values)?;
    Ok(propvariant(VT_VECTOR | VT_UI4, |pv| {
        pv.Anonymous.Anonymous.Anonymous.caul = CAUL {
            cElems: ulong(values.len()),
            pElems: p,
        };
    }))
}

/// Converts a `ULONG` size field. Every list here is a few hundred bytes.
#[allow(clippy::cast_possible_truncation, reason = "small lists")]
const fn ulong(size: usize) -> u32 {
    assert!(size <= u32::MAX as usize);
    size as u32
}

/// A heap buffer with the layout of `SENSOR_COLLECTION_LIST` /
/// `SENSOR_PROPERTY_LIST` (`L`): a header followed by `count` entries of `T`.
struct RawList<L, T> {
    ptr: NonNull<L>,
    count: usize,
    _entries: PhantomData<T>,
}

impl<L, T> RawList<L, T> {
    /// `AllocatedSizeInBytes` and `Count`.
    const HEADER: usize = 2 * size_of::<u32>();

    fn layout(count: usize) -> Layout {
        const {
            // The C structures declare the first entry inline, after the
            // header.
            assert!(size_of::<L>() == Self::HEADER + size_of::<T>());
            assert!(Self::HEADER % align_of::<T>() == 0);
        }
        Layout::from_size_align(Self::HEADER + count * size_of::<T>(), align_of::<L>())
            .expect("list sizes are small")
    }

    /// A zeroed list with the header filled in.
    fn new(count: usize) -> Self {
        let layout = Self::layout(count);
        // SAFETY: The layout is not zero sized.
        let Some(ptr) = NonNull::new(unsafe { alloc_zeroed(layout) }.cast::<L>()) else {
            handle_alloc_error(layout)
        };
        let header = ptr.as_ptr().cast::<u32>();
        // SAFETY: The allocation starts with the two u32 header fields.
        unsafe {
            header.write(ulong(layout.size()));
            header.add(1).write(ulong(count));
        }
        Self {
            ptr,
            count,
            _entries: PhantomData,
        }
    }

    const fn allocated_size(&self) -> u32 {
        // SAFETY: Header written in new().
        unsafe { self.ptr.as_ptr().cast::<u32>().read() }
    }

    const fn entries_mut(&mut self) -> &mut [T] {
        // SAFETY: count entries follow the header, zeroed or written since.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.ptr.as_ptr().cast::<u8>().add(Self::HEADER).cast::<T>(),
                self.count,
            )
        }
    }
}

impl<L, T> Drop for RawList<L, T> {
    fn drop(&mut self) {
        // SAFETY: Allocated in new() with this layout.
        unsafe { dealloc(self.ptr.as_ptr().cast(), Self::layout(self.count)) };
    }
}

/// A `SENSOR_COLLECTION_LIST`, which owns its values.
pub struct CollectionList(RawList<SENSOR_COLLECTION_LIST, SENSOR_VALUE_PAIR>);

impl CollectionList {
    /// Builds the list from `(key, value)` pairs. Values that fail to
    /// build (allocation failure) fail the whole list.
    pub fn new<const N: usize>(
        entries: [(PROPERTYKEY, Result<PROPVARIANT, NTSTATUS>); N],
    ) -> Result<Self, NTSTATUS> {
        let mut list = Self(RawList::new(N));
        // On error, values set so far are freed when `list` is dropped.
        for (pair, (key, value)) in list.0.entries_mut().iter_mut().zip(entries) {
            pair.Key = key;
            pair.Value = value?;
        }
        Ok(list)
    }

    /// Replaces the value at `index`.
    pub fn set(&mut self, index: usize, value: PROPVARIANT) {
        let slot = &mut self.0.entries_mut()[index].Value;
        // SAFETY: slot holds a value built by one of the pv_* functions or
        // copied in by SensorsUtils.
        unsafe { PropVariantClear(slot) };
        *slot = value;
    }

    pub const fn as_ptr(&self) -> *const SENSOR_COLLECTION_LIST {
        self.0.ptr.as_ptr()
    }

    #[allow(
        clippy::needless_pass_by_ref_mut,
        reason = "the list is written through the pointer"
    )]
    pub const fn as_mut_ptr(&mut self) -> *mut SENSOR_COLLECTION_LIST {
        self.0.ptr.as_ptr()
    }

    /// `CollectionsListGetMarshalledSize`
    pub fn marshalled_size(&self) -> u32 {
        // SAFETY: as_ptr is a valid, initialized list.
        unsafe { CollectionsListGetMarshalledSize(self.as_ptr()) }
    }

    /// `PropKeyFindKeyGetFloat`
    pub fn get_f32(&self, key: &PROPERTYKEY) -> Result<f32, NTSTATUS> {
        let mut value = 0.0;
        // SAFETY: The list is valid, value a valid out parameter.
        let status = unsafe { PropKeyFindKeyGetFloat(self.as_ptr(), key, &raw mut value) };
        if nt_success(status) {
            Ok(value)
        } else {
            Err(status)
        }
    }

    /// The `EvtSensorGet*` pattern: with no `target`, only returns the size
    /// needed. Otherwise also marshals the list into `target`.
    ///
    /// # Safety
    ///
    /// `target` is null or points to a list of `AllocatedSizeInBytes` bytes.
    pub unsafe fn copy_to(&self, target: *mut SENSOR_COLLECTION_LIST) -> Result<u32, NTSTATUS> {
        let size = self.marshalled_size();
        if !target.is_null() {
            // SAFETY: Caller guarantees target is valid.
            if unsafe { (*target).AllocatedSizeInBytes } < size {
                return Err(STATUS_INSUFFICIENT_RESOURCES);
            }
            // SAFETY: Both are valid lists, target is large enough.
            let status = unsafe { CollectionsListCopyAndMarshall(target, self.as_ptr()) };
            if !nt_success(status) {
                return Err(status);
            }
        }
        Ok(size)
    }
}

impl Drop for CollectionList {
    fn drop(&mut self) {
        for pair in self.0.entries_mut() {
            // SAFETY: Each value is VT_EMPTY or was set by this module or
            // SensorsUtils.
            unsafe { PropVariantClear(&raw mut pair.Value) };
        }
    }
}

/// A `SENSOR_PROPERTY_LIST`.
pub struct PropertyList(RawList<SENSOR_PROPERTY_LIST, PROPERTYKEY>);

impl PropertyList {
    pub fn new<const N: usize>(keys: [PROPERTYKEY; N]) -> Self {
        let mut list = Self(RawList::new(N));
        list.0.entries_mut().copy_from_slice(&keys);
        list
    }

    pub const fn as_ptr(&self) -> *const SENSOR_PROPERTY_LIST {
        self.0.ptr.as_ptr()
    }

    /// Like [`CollectionList::copy_to`], with `PropertiesListCopy`.
    ///
    /// # Safety
    ///
    /// `target` is null or points to a list of `AllocatedSizeInBytes` bytes.
    pub unsafe fn copy_to(&self, target: *mut SENSOR_PROPERTY_LIST) -> Result<u32, NTSTATUS> {
        let size = self.0.allocated_size();
        if !target.is_null() {
            // SAFETY: Caller guarantees target is valid.
            if unsafe { (*target).AllocatedSizeInBytes } < size {
                return Err(STATUS_INSUFFICIENT_RESOURCES);
            }
            // SAFETY: Both are valid lists, target is large enough.
            let status = unsafe { PropertiesListCopy(target, self.as_ptr()) };
            if !nt_success(status) {
                return Err(status);
            }
        }
        Ok(size)
    }
}
