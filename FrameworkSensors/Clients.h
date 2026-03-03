// SPDX-License-Identifier: MS-PL
//
// Copyright (C) Microsoft Corporation, All Rights Reserved
// Copyright (C) Framework Computer Inc, All Rights Reserved.
//
// Abstract:
//
//  This module contains the type definitions for the client
//  driver's device callback class.
//
// Environment:
//
//  Windows User-Mode Driver Framework (WUDF)

#pragma once

#include <windows.h>
#include <wdf.h>
#include <cmath>
#include <timeapi.h>

#include "Trace.h"
#include <SensorsCx.h>
#include <SensorsUtils.h>

//
// CLX Callbacks
//
EVT_SENSOR_DRIVER_START_SENSOR               OnStart;
EVT_SENSOR_DRIVER_STOP_SENSOR                OnStop;
EVT_SENSOR_DRIVER_GET_SUPPORTED_DATA_FIELDS  OnGetSupportedDataFields;
EVT_SENSOR_DRIVER_GET_PROPERTIES             OnGetProperties;
EVT_SENSOR_DRIVER_GET_DATA_FIELD_PROPERTIES  OnGetDataFieldProperties;
EVT_SENSOR_DRIVER_GET_DATA_INTERVAL          OnGetDataInterval;
EVT_SENSOR_DRIVER_SET_DATA_INTERVAL          OnSetDataInterval;
EVT_SENSOR_DRIVER_GET_DATA_THRESHOLDS        OnGetDataThresholds;
EVT_SENSOR_DRIVER_SET_DATA_THRESHOLDS        OnSetDataThresholds;
EVT_SENSOR_DRIVER_DEVICE_IO_CONTROL          OnIoControl;
EVT_SENSOR_DRIVER_START_SENSOR_HISTORY       OnStartHistory;
EVT_SENSOR_DRIVER_STOP_SENSOR_HISTORY        OnStopHistory;
EVT_SENSOR_DRIVER_CLEAR_SENSOR_HISTORY       OnClearHistory;
EVT_SENSOR_DRIVER_START_HISTORY_RETRIEVAL    OnStartHistoryRetrieval;
EVT_SENSOR_DRIVER_CANCEL_HISTORY_RETRIEVAL   OnCancelHistoryRetrieval;
EVT_SENSOR_DRIVER_ENABLE_WAKE                OnEnableWake;
EVT_SENSOR_DRIVER_DISABLE_WAKE               OnDisableWake;

EVT_WDF_TIMER                                OnTimerExpire;


/*++
Routine Description:
    Critical section lock/unlock to protect shared context
Return Value:
    None
--*/
#define Lock(lock)      { WdfWaitLockAcquire(lock, NULL); }
#define Unlock(lock)    { WdfWaitLockRelease(lock); }

#define SENSORV2_POOL_TAG_COMBO           '2bmC'

//
// Sensor Enumeration Properties
//
typedef enum
{
    SENSOR_TYPE_GUID = 0,
    SENSOR_MANUFACTURER,
    SENSOR_MODEL,
    SENSOR_CONNECTION_TYPE,
    SENSOR_PERSISTENT_UNIQUEID,
    SENSOR_ISPRIMARY,
    SENSOR_ENUMERATION_PROPERTIES_COUNT
} SENSOR_ENUMERATION_PROPERTIES_INDEX;

enum class SensorConnectionType : ULONG
{
    Integrated = 0,
    Attached = 1,
    External = 2
};

//
// Data-field Properties
//
typedef enum
{
    SENSOR_RESOLUTION = 0,
    SENSOR_MIN_RANGE,
    SENSOR_MAX_RANGE,
    SENSOR_DATA_FIELD_PROPERTY_COUNT
} SENSOR_DATA_FIELD_PROPERTY_INDEX;

//
// Sensor Common Properties
//
typedef enum
{
    SENSOR_COMMON_PROPERTY_STATE = 0,
    SENSOR_COMMON_PROPERTY_MIN_INTERVAL,
    SENSOR_COMMON_PROPERTY_MAX_DATAFIELDSIZE,
    SENSOR_COMMON_PROPERTY_TYPE,
    SENSOR_COMMON_PROPERTY_COUNT
} SENSOR_COMMON_PROPERTIES_INDEX;



//
// Base ---------------------------------------------------------------------
//
typedef class _ComboDevice
{
public:
    //
    // WDF
    //
    WDFDEVICE                   m_Device;
    SENSOROBJECT                m_SensorInstance;
    WDFWAITLOCK                 m_Lock;
    WDFTIMER                    m_Timer;

    //
    // Sensor Operation
    //
    BOOLEAN                     m_PoweredOn;
    BOOLEAN                     m_Started;
    ULONG                       m_IntervalMs;
    ULONG                       m_MinimumIntervalMs;
    BOOLEAN                     m_FirstSample;
    ULONG                       m_StartTime;
    ULONGLONG                   m_SampleCount;
    BOOLEAN                     m_WakeEnabled;
    HANDLE                      m_CrosEcHandle;

    //
    // Sensor Specific Properties
    //
    PSENSOR_COLLECTION_LIST     m_pEnumerationProperties;
    PSENSOR_COLLECTION_LIST     m_pProperties;
    PSENSOR_PROPERTY_LIST       m_pSupportedDataFields;
    PSENSOR_COLLECTION_LIST     m_pDataFieldProperties;
    PSENSOR_COLLECTION_LIST     m_pThresholds;
    PSENSOR_COLLECTION_LIST     m_pData;

public:
    //
    // Sensor specific functions
    //
    virtual NTSTATUS            Initialize(_In_ WDFDEVICE Device, _In_ SENSOROBJECT SensorObj) = NULL;
    virtual NTSTATUS            GetData(_In_ HANDLE Handle)                                    = NULL;
    virtual NTSTATUS            UpdateCachedThreshold()                                        = NULL;
    virtual NTSTATUS            EnableWake() { return STATUS_NOT_SUPPORTED; }
    virtual NTSTATUS            DisableWake() { return STATUS_NOT_SUPPORTED; }

    //
    // History functions - none of the sensors in this driver actually support history yet, this is for testing purpose now.
    //
    virtual NTSTATUS            StartHistory() { return STATUS_NOT_SUPPORTED; }
    virtual NTSTATUS            StopHistory() { return STATUS_NOT_SUPPORTED; }
    virtual NTSTATUS            ClearHistory() { return STATUS_NOT_SUPPORTED; }
    virtual NTSTATUS            StartHistoryRetrieval(_Inout_ PSENSOR_COLLECTION_LIST /*pHistoryBuffer*/, _In_ ULONG /*HistorySizeInBytes*/) { return STATUS_NOT_SUPPORTED; }
    virtual NTSTATUS            CancelHistoryRetrieval(_Out_ PULONG /*pBytesWritten*/) { return STATUS_NOT_SUPPORTED; }

} ComboDevice, *PComboDevice;

// Set up accessor function to retrieve device context
WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(ComboDevice, GetContextFromSensorInstance);


//
// Ambient Light --------------------------------------------------------------
//
typedef class _AlsDevice : public _ComboDevice
{
private:
    // Internal struct used to store readings
    typedef struct _AlsData
    {
        FLOAT Lux;
        FLOAT Kelvins;
        FLOAT ChromaticityX;
        FLOAT ChromaticityY;
        BOOL IsValid;
    } AlsData, *PAlsData;

    // Internal struct used to store thresholds
    typedef struct _AlsThresholdData
    {
        FLOAT LuxPct;
        FLOAT LuxAbs;
        FLOAT KelvinsAbs;
        FLOAT ChromaticityXAbs;
        FLOAT ChromaticityYAbs;
    } AlsThresholdData;

private:

    AlsThresholdData            m_CachedThresholds;
    AlsData                     m_CachedData;
    AlsData                     m_LastSample;

public:

    NTSTATUS                    Initialize(_In_ WDFDEVICE Device, _In_ SENSOROBJECT SensorObj);
    NTSTATUS                    GetData(_In_ HANDLE Device);
    NTSTATUS                    UpdateCachedThreshold();

} AlsDevice, *PAlsDevice;



//
// Accelerometer --------------------------------------------------------------
//
typedef class _AccelerometerDevice : public _ComboDevice
{
private:

    typedef struct _AccelerometerSample
    {
        VEC3D   Axis;
        BOOL    Shake;
    } AccelerometerSample, *PAccelerometerSample;

    AccelerometerSample                       m_CachedThresholds;
    AccelerometerSample                       m_CachedData;
    AccelerometerSample                       m_LastSample;
    UINT8                                     m_LidSensorIndex;
    UINT8                                     m_BaseSensor;
    BOOLEAN                                   m_SensorIndicesInitialized;

public:

    NTSTATUS                    Initialize(_In_ WDFDEVICE Device, _In_ SENSOROBJECT SensorObj);
    NTSTATUS                    GetData(_In_ HANDLE Device);
    NTSTATUS                    UpdateCachedThreshold();

} AccelerometerDevice, *PAccelerometerDevice;



//
// Simple Device Orientation --------------------------------------------------
//
typedef class _SimpleDeviceOrientationDevice : public _ComboDevice
{
private:

    typedef struct _SimpleDeviceOrientationSample
    {
        FLOAT    X;
    } SimpleDeviceOrientationSample, *PSimpleDeviceOrientationSample;

    SimpleDeviceOrientationSample                       m_CachedThresholds;
    SimpleDeviceOrientationSample                       m_CachedData;
    SimpleDeviceOrientationSample                       m_LastSample;

public:

    NTSTATUS                    Initialize(_In_ WDFDEVICE Device, _In_ SENSOROBJECT SensorObj);
    NTSTATUS                    GetData(_In_ HANDLE Device);
    NTSTATUS                    UpdateCachedThreshold();

} SimpleDeviceOrientationDevice, *PSimpleDeviceOrientationDevice;
