use wdk::nt_success;
use wdk_sys::{
    call_unsafe_wdf_function_binding,
    NTSTATUS,
    PCUNICODE_STRING,
    PDRIVER_OBJECT,
    PWDFDEVICE_INIT,
    WDFDRIVER,
    WDF_DRIVER_CONFIG,
    WDF_NO_HANDLE,
    WDF_NO_OBJECT_ATTRIBUTES,
};

use crate::{device, trace, wdf_struct_size};

/// `DriverEntry` initializes the driver and is the first routine called by the
/// framework after the driver is loaded.
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
extern "system" fn driver_entry(
    driver: PDRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    wdk_panic::install_panic_hook();

    let mut driver_config = WDF_DRIVER_CONFIG {
        Size: wdf_struct_size!(WDF_DRIVER_CONFIG),
        EvtDriverDeviceAdd: Some(evt_driver_device_add),
        ..WDF_DRIVER_CONFIG::default()
    };

    // SAFETY: Arguments come from the framework or are valid locals.
    let status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &raw mut driver_config,
            WDF_NO_HANDLE.cast::<WDFDRIVER>(),
        )
    };
    if !nt_success(status) {
        trace!("WdfDriverCreate failed {status:#010X}");
    }
    status
}

/// Called by the framework in response to `AddDevice` from the `PnP`
/// manager.
extern "C" fn evt_driver_device_add(_driver: WDFDRIVER, device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    device::create(device_init)
}
