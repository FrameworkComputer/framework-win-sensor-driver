// Copyright (C) Framework Computer Inc, All Rights Reserved.
//
// Abstract:
//
//  This module contains the implementation of communication with the embedded controller.
//
//  Only need EC commands to use EC_CMD_MOTION_SENSE_CMD to determine which
//  accel sensors there are and which position they are. ALl the rest can be
//  done using memory map reads.
//
// Environment:
//
//  Windows User-Mode Driver Framework (UMDF)

#include "Clients.h"
#include "EcCommunication.h"
#include <windows.h>
#include <wdf.h>

#include "EcCommunication.tmh"

int CrosEcReadMemU8(HANDLE Handle, unsigned int offset, UINT8* dest)
{
    NTSTATUS Status = STATUS_SUCCESS;
    DWORD retb{};
    CROSEC_READMEM rm{};

    if (Handle == INVALID_HANDLE_VALUE) {
        Status = STATUS_INVALID_HANDLE;
        TraceError("COMBO %!FUNC! Invalid Handle");
        return 0;
    }

    rm.bytes = 0x01;
    rm.offset = offset;
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
        return 0;
    }

    TraceInformation("COMBO %!FUNC! Successfully read %d bytes from EC memory at %02x. First one %02x. retb=%d", rm.bytes, rm.offset, rm.buffer[0], retb);
    *dest = rm.buffer[0];

    return rm.bytes;
}
