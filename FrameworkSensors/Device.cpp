// SPDX-License-Identifier: MS-PL
//
// Copyright (C) Microsoft Corporation, All Rights Reserved.
// Copyright (C) Framework Computer Inc, All Rights Reserved.
//
// Abstract:
//
//  This module contains the implementation of WDF callback functions
//  for Framework Sensors driver.
//
// Environment:
//
//  Windows User-Mode Driver Framework (WUDF)

#include "Clients.h"
#include "Driver.h"
#include "EcCommunication.h"

#include <new.h>
#include <winnt.h>

#include "Device.tmh"

//---------------------------------------
// Dynamic sensor detection
//---------------------------------------
#define MAX_SENSOR_COUNT 4

typedef enum {
    SENSOR_KIND_ACCELEROMETER,
    SENSOR_KIND_HINGEANGLE,
    SENSOR_KIND_ALS,
} SensorKind;

static ULONG SensorInstanceCount = 0;
static SENSOROBJECT SensorInstancesBuffer[MAX_SENSOR_COUNT];

static size_t SensorSizes[MAX_SENSOR_COUNT];
static SensorKind SensorKinds[MAX_SENSOR_COUNT];

inline size_t GetDeviceSizeAtIndex(
    _In_ ULONG Index)
{
    if (Index < SensorInstanceCount)
    {
        return SensorSizes[Index];
    }
    return 0;
}

void AllocateDeviceAtIndex(
    _In_ ULONG Index,
    _Inout_ PComboDevice* ppDevice
    )
{
    if (Index >= SensorInstanceCount)
    {
        return;
    }

    switch (SensorKinds[Index])
    {
        case SENSOR_KIND_ACCELEROMETER: *ppDevice = new(*ppDevice) AccelerometerDevice; break;
        case SENSOR_KIND_HINGEANGLE:    *ppDevice = new(*ppDevice) HingeAngleDevice; break;
        case SENSOR_KIND_ALS:           *ppDevice = new(*ppDevice) AlsDevice; break;
        default: break;
    }
}

//------------------------------------------------------------------------------
//
// Function: OnDeviceAdd
//
// This routine is the AddDevice entry point for the  Framework Sensors client
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

    SENSOR_FunctionEnter();

    //
    // Dynamic sensor detection
    //
    SensorInstanceCount = 0;

    // Detect accelerometer + hinge angle via motion sensor presence
    {
        UINT8 acc_status = 0;
        CrosEcReadMemU8(Handle, EC_MEMMAP_ACC_STATUS, &acc_status);
        if (acc_status & EC_MEMMAP_ACC_STATUS_PRESENCE_BIT)
        {
            UINT8 base = 0, lid = 0;
            if (NT_SUCCESS(CrosEcGetAccelIndeces(Handle, &base, &lid)))
            {
                TraceInformation("COMBO %!FUNC! Detected accelerometer (base=%d, lid=%d)", base, lid);
                SensorSizes[SensorInstanceCount] = sizeof(AccelerometerDevice);
                SensorKinds[SensorInstanceCount] = SENSOR_KIND_ACCELEROMETER;
                SensorInstanceCount++;

                TraceInformation("COMBO %!FUNC! Detected hinge angle (base+lid accels present)");
                SensorSizes[SensorInstanceCount] = sizeof(HingeAngleDevice);
                SensorKinds[SensorInstanceCount] = SENSOR_KIND_HINGEANGLE;
                SensorInstanceCount++;
            }
        }
    }

    // Detect ALS via motion sensor enumeration, then fallback to memmap
    {
        BOOLEAN alsFound = FALSE;
        UINT8 sensorCount = CrosEcGetMotionSensorCount(Handle);
        for (UINT8 idx = 0; idx < sensorCount && !alsFound; idx++)
        {
            EC_REQUEST_MOTION_SENSE_INFO infoReq{};
            EC_RESPONSE_MOTION_SENSE_INFO infoRes{};

            infoReq.Cmd = 1;
            infoReq.SensorNum = idx;
            if (0 != CrosEcSendCommand(
                Handle,
                EC_CMD_MOTION_SENSE_CMD,
                1,
                &infoReq,
                sizeof(infoReq),
                &infoRes,
                sizeof(infoRes)))
            {
                if (infoRes.SensorType == MOTIONSENSE_TYPE_LIGHT ||
                    infoRes.SensorType == MOTIONSENSE_TYPE_LIGHT_RGB)
                {
                    TraceInformation("COMBO %!FUNC! Detected ALS via motion sensor index %d", idx);
                    alsFound = TRUE;
                }
            }
        }

        // Fallback: check EC_MEMMAP_ALS for non-zero value
        if (!alsFound)
        {
            UINT8 alsVal[2] = {0};
            CrosEcReadMemU8(Handle, EC_MEMMAP_ALS + 0, &alsVal[0]);
            CrosEcReadMemU8(Handle, EC_MEMMAP_ALS + 1, &alsVal[1]);
            UINT16 alsReading = alsVal[0] + (alsVal[1] << 8);
            if (alsReading != 0)
            {
                TraceInformation("COMBO %!FUNC! Detected ALS via memmap (reading=%d)", alsReading);
                alsFound = TRUE;
            }
        }

        if (alsFound && SensorInstanceCount < MAX_SENSOR_COUNT)
        {
            SensorSizes[SensorInstanceCount] = sizeof(AlsDevice);
            SensorKinds[SensorInstanceCount] = SENSOR_KIND_ALS;
            SensorInstanceCount++;
        }
    }

    TraceInformation("COMBO %!FUNC! Detected %d sensor(s) total", SensorInstanceCount);

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

        pDevice->m_CrosEcHandle = INVALID_HANDLE_VALUE;

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

        // Close handle to EC driver
        if (pDevice->m_CrosEcHandle && pDevice->m_CrosEcHandle != INVALID_HANDLE_VALUE) {
            CloseHandle(pDevice->m_CrosEcHandle);
            pDevice->m_CrosEcHandle = INVALID_HANDLE_VALUE;
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

        // Close EC handle before sleep to avoid stale handles after wake.
        // The handle will be re-acquired in OnTimerExpire when the device wakes.
        if (pDevice->m_CrosEcHandle != INVALID_HANDLE_VALUE) {
            CloseHandle(pDevice->m_CrosEcHandle);
            pDevice->m_CrosEcHandle = INVALID_HANDLE_VALUE;
        }

        pDevice->m_PoweredOn = FALSE;
        InitPropVariantFromUInt32(SensorState_Idle,
                                  &(pDevice->m_pProperties->List[SENSOR_COMMON_PROPERTY_STATE].Value));
    }

Exit:
    SENSOR_FunctionExit(Status);
    return Status;
}
