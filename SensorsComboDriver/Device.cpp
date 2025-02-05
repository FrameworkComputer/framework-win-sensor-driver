// Copyright (C) Microsoft Corporation, All Rights Reserved.
//
// Abstract:
//
//  This module contains the implementation of WDF callback functions
//  for combo driver.
//
// Environment:
//
//  Windows User-Mode Driver Framework (WUDF)

#include "Clients.h"
#include "Driver.h"

#include <new.h>
#include <winnt.h>

#include "Device.tmh"

#define EC_LPC_ADDR_MEMMAP       0xE00
#define EC_MEMMAP_SIZE         255 /* ACPI IO buffer max is 255 bytes */
#define ENABLE_ALS_SENSOR 1
#define ENABLE_ORIENTATION_SENSOR 0
#define ENABLE_ACCEL_SENSOR 0

//---------------------------------------
// Declare and map devices below
//---------------------------------------
enum Device
{
#if ENABLE_ALS_SENSOR
    Device_Als,
#endif
#if ENABLE_ORIENTATION_SENSOR
    Device_SimpleDeviceOrientation,
#endif
#if ENABLE_ACCEL_SENSOR
    Device_LinearAccelerometer,
#endif
    // Keep this last
    Device_Count
};

static const ULONG SensorInstanceCount = Device_Count;
static SENSOROBJECT SensorInstancesBuffer[SensorInstanceCount];    // Global buffer to avoid allocate and free

inline size_t GetDeviceSizeAtIndex(
    _In_ ULONG Index)
{
    size_t result = 0;
    switch (static_cast<Device>(Index))
    {
#if ENABLE_ALS_SENSOR
        case Device_Als:                    result = sizeof(AlsDevice); break;
#endif
#if ENABLE_ORIENTATION_SENSOR
        case Device_SimpleDeviceOrientation:result = sizeof(SimpleDeviceOrientationDevice); break;
#endif
#if ENABLE_ACCEL_SENSOR
        case Device_LinearAccelerometer:    result = sizeof(LinearAccelerometerDevice); break;
#endif
        default: break; // invalid
    }
    return result;
}

void AllocateDeviceAtIndex(
    _In_ ULONG Index,
    _Inout_ PComboDevice* ppDevice
    )
{
    switch (static_cast<Device>(Index))
    {
#if ENABLE_ALS_SENSOR
        case Device_Als:                    *ppDevice = new(*ppDevice) AlsDevice; break;
#endif
#if ENABLE_ORIENTATIONACCEL_SENSOR
        case Device_SimpleDeviceOrientation:*ppDevice = new(*ppDevice) SimpleDeviceOrientationDevice; break;
#endif
#if ENABLE_ACCEL_SENSOR
        case Device_LinearAccelerometer:    *ppDevice = new(*ppDevice) LinearAccelerometerDevice; break;
#endif

        default: break; // invalid (let driver fail)
    }
}

NTSTATUS ConnectToEc(
	_In_ WDFDEVICE FxDevice,
    _Inout_ HANDLE *Handle
) {
    SENSOR_FunctionEnter();
    NTSTATUS Status = STATUS_SUCCESS;

    UNREFERENCED_PARAMETER(FxDevice);

    *Handle = CreateFileW(
        LR"(\\.\GLOBALROOT\Device\CrosEC)",
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        NULL);

    if (*Handle == INVALID_HANDLE_VALUE) {
        Status = STATUS_INVALID_HANDLE;
        TraceError("COMBO %!FUNC! CreateFileW failed %!STATUS!", Status);
        goto Exit;
    }

Exit:
    SENSOR_FunctionExit(Status);

	return Status;
}



//------------------------------------------------------------------------------
//
// Function: OnDeviceAdd
//
// This routine is the AddDevice entry point for the  combo client
// driver. This routine is called by the framework in response to AddDevice
// call from the PnP manager. It will create and initialize the device object
// to represent a new instance of the sensor client.
//
// Arguments:
//      Driver: IN: Supplies a handle to the driver object created in DriverEntry
//      DeviceInit: IN: Supplies a pointer to a framework-allocated WDFDEVICE_INIT structure
//
// Return Value:
//      NTSTATUS code
//------------------------------------------------------------------------------
NTSTATUS
OnDeviceAdd(
    _In_ WDFDRIVER /*Driver*/,
    _Inout_ PWDFDEVICE_INIT pDeviceInit
    )
{
    WDF_PNPPOWER_EVENT_CALLBACKS Callbacks;
    WDFDEVICE Device;
    WDF_OBJECT_ATTRIBUTES FdoAttributes;
    ULONG Flag = 0;
    SENSOR_CONTROLLER_CONFIG SensorConfig;
    NTSTATUS Status = STATUS_SUCCESS;

    SENSOR_FunctionEnter();

    WDF_OBJECT_ATTRIBUTES_INIT(&FdoAttributes);

    //
    // Initialize FDO attributes and set up file object with sensor extension
    //
    Status = SensorsCxDeviceInitConfig(pDeviceInit, &FdoAttributes, Flag);
    if (!NT_SUCCESS(Status))
    {
        TraceError("COMBO %!FUNC! SensorsCxDeviceInitConfig failed %!STATUS!", Status);
        goto Exit;
    }

    //
    // Register the PnP callbacks with the framework.
    //
    WDF_PNPPOWER_EVENT_CALLBACKS_INIT(&Callbacks);
    Callbacks.EvtDevicePrepareHardware = OnPrepareHardware;
    Callbacks.EvtDeviceReleaseHardware = OnReleaseHardware;
    Callbacks.EvtDeviceD0Entry = OnD0Entry;
    Callbacks.EvtDeviceD0Exit = OnD0Exit;

    WdfDeviceInitSetPnpPowerEventCallbacks(pDeviceInit, &Callbacks);

    //
    // Call the framework to create the device
    //
    Status = WdfDeviceCreate(&pDeviceInit, &FdoAttributes, &Device);
    if (!NT_SUCCESS(Status))
    {
        TraceError("COMBO %!FUNC! WdfDeviceCreate failed %!STATUS!", Status);
        goto Exit;
    }

    //
    // Register CLX callback function pointers
    //
    SENSOR_CONTROLLER_CONFIG_INIT(&SensorConfig);
    SensorConfig.DriverIsPowerPolicyOwner = WdfUseDefault;

    SensorConfig.EvtSensorStart                     = OnStart;
    SensorConfig.EvtSensorStop                      = OnStop;
    SensorConfig.EvtSensorGetSupportedDataFields    = OnGetSupportedDataFields;
    SensorConfig.EvtSensorGetDataInterval           = OnGetDataInterval;
    SensorConfig.EvtSensorSetDataInterval           = OnSetDataInterval;
    SensorConfig.EvtSensorGetDataFieldProperties    = OnGetDataFieldProperties;
    SensorConfig.EvtSensorGetDataThresholds         = OnGetDataThresholds;
    SensorConfig.EvtSensorSetDataThresholds         = OnSetDataThresholds;
    SensorConfig.EvtSensorGetProperties             = OnGetProperties;
    SensorConfig.EvtSensorDeviceIoControl           = OnIoControl;
    SensorConfig.EvtSensorStartHistory              = OnStartHistory;
    SensorConfig.EvtSensorStopHistory               = OnStopHistory;
    SensorConfig.EvtSensorClearHistory              = OnClearHistory;
    SensorConfig.EvtSensorStartHistoryRetrieval     = OnStartHistoryRetrieval;
    SensorConfig.EvtSensorCancelHistoryRetrieval    = OnCancelHistoryRetrieval;
    SensorConfig.EvtSensorEnableWake                = OnEnableWake;
    SensorConfig.EvtSensorDisableWake               = OnDisableWake;

    //
    // Set up power capabilities and IO queues
    //
    Status = SensorsCxDeviceInitialize(Device, &SensorConfig);
    if (!NT_SUCCESS(Status))
    {
        TraceError("COMBO %!FUNC! SensorDeviceInitialize failed %!STATUS!", Status);
        goto Exit;
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}


//------------------------------------------------------------------------------
//
// Function: OnPrepareHardware
//
// This routine is called by the framework when the PnP manager sends an
// IRP_MN_START_DEVICE request to the driver stack. This routine is
// responsible for performing operations that are necessary to make the
// driver's device operational (for e.g. mapping the hardware resources
// into memory).
//
// Argument:
//      Device: IN: Supplies a handle to the framework device object
//      ResourcesRaw: IN: Supplies a handle to a collection of framework resource
//          objects. This collection identifies the raw (bus-relative) hardware
//          resources that have been assigned to the device.
//      ResourcesTranslated: IN: Supplies a handle to a collection of framework
//          resource objects. This collection identifies the translated
//          (system-physical) hardware resources that have been assigned to the
//          device. The resources appear from the CPU's point of view.
//
// Return Value:
//      NTSTATUS code
//------------------------------------------------------------------------------
NTSTATUS
OnPrepareHardware(
    _In_ WDFDEVICE Device,
    _In_ WDFCMRESLIST /*ResourcesRaw*/,
    _In_ WDFCMRESLIST /*ResourcesTranslated*/
    )
{
    NTSTATUS Status = STATUS_SUCCESS;
    ULONG  i;
    HANDLE Handle;
    DWORD retb{};
    CROSEC_READMEM rm{};

    SENSOR_FunctionEnter();

    Status = ConnectToEc(Device, &Handle);
    if (!NT_SUCCESS(Status)) {
        TraceError("COMBO %!FUNC! ConnectToEc failed %!STATUS!", Status);
        goto Exit;
    }

    rm.bytes = 0xfe;
    rm.offset = 0;
    Status = DeviceIoControl(Handle,
        (DWORD) IOCTL_CROSEC_RDMEM,
        &rm,
        sizeof(rm),
        &rm,
        sizeof(rm),
        &retb,
        nullptr);
    if (!NT_SUCCESS(Status)) {
        TraceError("COMBO %!FUNC! ConnectToEc failed %!STATUS!", Status);
        goto Exit;
    }

    UINT8 *EcMem = rm.buffer;
    for (i = 0; i < 0xfe-16; i+=16) {
        TraceInformation(
            "%02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X\n",
            EcMem[i], EcMem[i+1], EcMem[i+2], EcMem[i+3], EcMem[i+4], EcMem[i + 5], EcMem[i + 6], EcMem[i + 7],
            EcMem[i + 8], EcMem[i+9], EcMem[i+10], EcMem[i+11], EcMem[i+12], EcMem[i + 13], EcMem[i + 14], EcMem[i + 15]
        );
    }

    for (ULONG Count = 0; Count < SensorInstanceCount; Count++)
    {
        PComboDevice pDevice = nullptr;
        WDF_OBJECT_ATTRIBUTES SensorAttr;
        SENSOR_CONFIG SensorConfig;
        SENSOROBJECT SensorInstance;

        // Create WDFOBJECT for the sensor
        WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&SensorAttr, ComboDevice);
        SensorAttr.ContextSizeOverride = GetDeviceSizeAtIndex(Count);

        // Register sensor instance with clx
        Status = SensorsCxSensorCreate(Device, &SensorAttr, &SensorInstance);
        if (!NT_SUCCESS(Status))
        {
            TraceError("COMBO %!FUNC! SensorsCxSensorCreate failed %!STATUS!", Status);
            goto Exit;
        }

        pDevice = GetContextFromSensorInstance(SensorInstance);
        if (nullptr == pDevice)
        {
            Status = STATUS_INSUFFICIENT_RESOURCES;
            TraceError("COMBO %!FUNC! GetContextFromSensorInstance failed %!STATUS!", Status);
            goto Exit;
        }

        AllocateDeviceAtIndex(Count, &pDevice);

        pDevice->m_CrosEcHandle = Handle;

        // Fill out properties
        Status = pDevice->Initialize(Device, SensorInstance);
        if (!NT_SUCCESS(Status))
        {
            TraceError("COMBO %!FUNC! Initialize device object failed %!STATUS!", Status);
            goto Exit;
        }

        SENSOR_CONFIG_INIT(&SensorConfig);
        SensorConfig.pEnumerationList = pDevice->m_pEnumerationProperties;
        Status = SensorsCxSensorInitialize(SensorInstance, &SensorConfig);
        if (!NT_SUCCESS(Status))
        {
            TraceError("COMBO %!FUNC! SensorsCxSensorInitialize failed %!STATUS!", Status);
            goto Exit;
        }
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}



//------------------------------------------------------------------------------
//
// Function: OnReleaseHardware
//
// This routine is called by the framework when the PnP manager is revoking
// ownership of our resources. This may be in response to either
// IRP_MN_STOP_DEVICE or IRP_MN_REMOVE_DEVICE. This routine is responsible for
// performing cleanup of resources allocated in PrepareHardware callback.
// This callback is invoked before passing  the request down to the lower driver.
// This routine will also be invoked by the framework if the prepare hardware
// callback returns a failure.
//
// Argument:
//      Device: IN: Supplies a handle to the framework device object
//      ResourcesTranslated: IN: Supplies a handle to a collection of framework
//          resource objects. This collection identifies the translated
//          (system-physical) hardware resources that have been assigned to the
//          device. The resources appear from the CPU's point of view.
//
// Return Value:
//      NTSTATUS code
//------------------------------------------------------------------------------
NTSTATUS
OnReleaseHardware(
    _In_ WDFDEVICE Device,
    _In_ WDFCMRESLIST /*ResourcesTranslated*/
    )
{
    ULONG Count = SensorInstanceCount;
    PComboDevice pDevice = nullptr;
    NTSTATUS Status = STATUS_SUCCESS;

    SENSOR_FunctionEnter();

    Status = SensorsCxDeviceGetSensorList(Device, SensorInstancesBuffer, &Count);
    if (!NT_SUCCESS(Status))
    {
        Status = STATUS_INVALID_PARAMETER;
        TraceError("COMBO %!FUNC! SensorsCxDeviceGetSensorList failed %!STATUS!", Status);
        goto Exit;
    }

    for (Count = 0; Count < SensorInstanceCount; Count++)
    {
        pDevice = GetContextFromSensorInstance(SensorInstancesBuffer[Count]);
        if (nullptr == pDevice)
        {
            Status = STATUS_INVALID_PARAMETER;
            TraceError("COMBO %!FUNC! GetContextFromSensorInstance failed %!STATUS!", Status);
            goto Exit;
        }

        // Delete lock
        if (NULL != pDevice->m_Lock)
        {
            WdfObjectDelete(pDevice->m_Lock);
            pDevice->m_Lock = NULL;
        }

        // Delete sensor instance
        if (NULL != pDevice->m_SensorInstance)
        {
            WdfObjectDelete(pDevice->m_SensorInstance);
            // The pDevice context created using WdfMemoryCreate and parented to m_SensorInstance is automatically
            // destroyed when m_SensorInstance is deleted. pDevice is therefore no longer accessible beyond the above call to WdfObjectDelete.
            // We can therefore not set the m_SensorInstance member back to NULL. We instead set pDevice to nullptr.
            pDevice = nullptr;
        }
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}

//------------------------------------------------------------------------------
//
// Function: OnD0Entry
//
// This routine is invoked by the framework to program the device to goto
// D0, which is the working state. The framework invokes callback every
// time the hardware needs to be (re-)initialized.  This includes after
// IRP_MN_START_DEVICE, IRP_MN_CANCEL_STOP_DEVICE, IRP_MN_CANCEL_REMOVE_DEVICE,
// and IRP_MN_SET_POWER-D0.
//
// Argument:
//      Device: IN: Supplies a handle to the framework device object
//      PreviousState: IN: WDF_POWER_DEVICE_STATE-typed enumerator that identifies
//          the device power state that the device was in before this transition to D0
//
// Return Value:
//      NTSTATUS code
//------------------------------------------------------------------------------
NTSTATUS
OnD0Entry(
    _In_ WDFDEVICE Device,
    _In_ WDF_POWER_DEVICE_STATE /*PreviousState*/
    )
{
    ULONG Count = SensorInstanceCount;
    PComboDevice pDevice = nullptr;
    NTSTATUS Status = STATUS_SUCCESS;

    SENSOR_FunctionEnter();


    //
    // Get sensor instances
    //
    Status = SensorsCxDeviceGetSensorList(Device, SensorInstancesBuffer, &Count);
    if (!NT_SUCCESS(Status))
    {
        Status = STATUS_INVALID_PARAMETER;
        TraceError("COMBO %!FUNC! SensorsCxDeviceGetSensorList failed %!STATUS!", Status);
        goto Exit;
    }

    //
    // Powering on all sensor instances
    //
    for (Count = 0; Count < SensorInstanceCount; Count++)
    {
        pDevice = GetContextFromSensorInstance(SensorInstancesBuffer[Count]);
        if (nullptr == pDevice)
        {
            Status = STATUS_INVALID_PARAMETER;
            TraceError("COMBO %!FUNC! GetContextFromSensorInstance failed %!STATUS!", Status);
            goto Exit;
        }

        pDevice->m_PoweredOn = TRUE;
        InitPropVariantFromUInt32(SensorState_Idle,
                                  &(pDevice->m_pProperties->List[SENSOR_COMMON_PROPERTY_STATE].Value));
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}



//------------------------------------------------------------------------------
//
// Function: OnD0Exit
//
// This routine is invoked by the framework to program the device to go into
// a certain Dx state. The framework invokes callback every the the device is
// leaving the D0 state, which happens when the device is stopped, when it is
// removed, and when it is powered off.
//
// Argument:
//      Device: IN: Supplies a handle to the framework device object
//      TargetState: IN: Supplies the device power state which the device will be put
//          in once the callback is complete
//
// Return Value:
//      NTSTATUS code
//------------------------------------------------------------------------------
NTSTATUS
OnD0Exit(
    _In_ WDFDEVICE Device,
    _In_ WDF_POWER_DEVICE_STATE /*TargetState*/
    )
{
    ULONG Count = SensorInstanceCount;
    PComboDevice pDevice = nullptr;
    NTSTATUS Status = STATUS_SUCCESS;

    SENSOR_FunctionEnter();

    //
    // Get sensor instances
    //
    Status = SensorsCxDeviceGetSensorList(Device, SensorInstancesBuffer, &Count);
    if (!NT_SUCCESS(Status))
    {
        Status = STATUS_INVALID_PARAMETER;
        TraceError("COMBO %!FUNC! SensorsCxDeviceGetSensorList failed %!STATUS!", Status);
        goto Exit;
    }

    //
    // Powering off all sensor instances
    //
    for (Count = 0; Count < SensorInstanceCount; Count++)
    {
        pDevice = GetContextFromSensorInstance(SensorInstancesBuffer[Count]);
        if (nullptr == pDevice)
        {
            Status = STATUS_INVALID_PARAMETER;
            TraceError("COMBO %!FUNC! GetContextFromSensorInstance failed %!STATUS!", Status);
            goto Exit;
        }

        pDevice->m_PoweredOn = FALSE;
        InitPropVariantFromUInt32(SensorState_Idle,
                                  &(pDevice->m_pProperties->List[SENSOR_COMMON_PROPERTY_STATE].Value));
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}
