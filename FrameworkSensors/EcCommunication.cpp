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

#include "EcCommunication.h"
#include <windows.h>
#include <wdf.h>

#include "EcCommunication.tmh"

NTSTATUS ConnectToEc(
    _Inout_ HANDLE* Handle
) {
    NTSTATUS Status = STATUS_SUCCESS;

    *Handle = CreateFileW(
        LR"(\\.\GLOBALROOT\Device\CrosEC)",
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        NULL);

    if (*Handle == INVALID_HANDLE_VALUE) {
        TraceError("%!FUNC! CreateFileW failed %!STATUS!", Status);
        return STATUS_INVALID_HANDLE;
    }

    return Status;
}

int CrosEcSendCommand(
    HANDLE Handle,
    UINT16 command,
    UINT8 version,
    LPVOID outdata,
    unsigned int outlen,
    LPVOID indata,
    unsigned int inlen
)
{
    NTSTATUS Status = STATUS_SUCCESS;
    DWORD retb{};
    CROSEC_COMMAND cmd{};

    if (Handle == INVALID_HANDLE_VALUE) {
        Status = STATUS_INVALID_HANDLE;
        TraceError("%!FUNC! Invalid Handle");
        return 0;
    }

    if (outlen > CROS_EC_CMD_MAX_REQUEST || inlen > CROS_EC_CMD_MAX_REQUEST) {
        TraceError("%!FUNC! outlen %d or inlen %d too large", outlen, inlen);
        return 0;
    }
    if (outlen == 0) {
        TraceError("%!FUNC! outlen is 0");
        return 0;
    }
    if (outdata == nullptr) {
        TraceError("%!FUNC! Invalid outdata - NULL");
        return 0;
    }

    cmd.command = command;
    cmd.version = version;
    cmd.result = 0xFF;
    cmd.outlen = outlen;
    cmd.inlen = CROS_EC_CMD_MAX_REQUEST - 8; // 8 is the header length

    RtlCopyMemory(cmd.data, outdata, outlen);

    Status = DeviceIoControl(Handle,
        (DWORD) IOCTL_CROSEC_XCMD,
        &cmd,
        sizeof(cmd),
        &cmd,
        sizeof(cmd),
        &retb,
        nullptr);
    if (!NT_SUCCESS(Status)) {
        TraceError("%!FUNC! ConnectToEc failed %!STATUS!", Status);
        return 0;
    }

    if (cmd.result != EC_RES_SUCCESS) {
        TraceError("%!FUNC! Host command failed - EC result %d", cmd.result);
        return 0;
    }

    if (inlen > 0) {
        if (indata == nullptr) {
            TraceError("%!FUNC! inlen is %d. But indata is NULL", inlen);
            return 0;
        }
        RtlCopyMemory(indata, cmd.data, inlen);
    }

    return cmd.inlen;
}


int CrosEcReadMemU8(HANDLE Handle, unsigned int offset, UINT8* dest)
{
    NTSTATUS Status = STATUS_SUCCESS;
    DWORD retb{};
    CROSEC_READMEM rm{};

    if (Handle == INVALID_HANDLE_VALUE) {
        Status = STATUS_INVALID_HANDLE;
        TraceError("%!FUNC! Invalid Handle");
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
        TraceError("%!FUNC! ConnectToEc failed %!STATUS!", Status);
        return 0;
    }

    TraceInformation("%!FUNC! Successfully read %d bytes from EC memory at %02x. First one %02x. retb=%d", rm.bytes, rm.offset, rm.buffer[0], retb);
    *dest = rm.buffer[0];

    return rm.bytes;
}
