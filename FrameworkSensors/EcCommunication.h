//Copyright (C) Framework Computer Inc
//Copyright (C) 2014 The ChromiumOS Authors
//
//Abstract:
//
//    Definitions for accessing EC

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <handleapi.h>
#include <windows.h>
#include <wdf.h>
#include "Trace.h"

/* Command version mask */
#define EC_VER_MASK(version) (1UL << (version))

#define EC_MEMMAP_ALS 0x80 /* ALS readings in lux (2 X 16 bits) */
/* Unused 0x84 - 0x8f */
#define EC_MEMMAP_ACC_STATUS 0x90 /* Accelerometer status (8 bits )*/
/* Unused 0x91 */
#define EC_MEMMAP_ACC_DATA 0x92 /* Accelerometers data 0x92 - 0x9f */
/* 0x92: Lid Angle if available, LID_ANGLE_UNRELIABLE otherwise */
/* 0x94 - 0x99: 1st Accelerometer */
/* 0x9a - 0x9f: 2nd Accelerometer */

/* Define the format of the accelerometer mapped memory status byte. */
#define EC_MEMMAP_ACC_STATUS_SAMPLE_ID_MASK 0x0f
// BIT(4)
#define EC_MEMMAP_ACC_STATUS_BUSY_BIT (1 << 4)
// BIT(7)
#define EC_MEMMAP_ACC_STATUS_PRESENCE_BIT (1 << 7)

#define FILE_DEVICE_CROS_EMBEDDED_CONTROLLER 0x80EC

#define IOCTL_CROSEC_XCMD \
	CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x801, METHOD_BUFFERED, FILE_READ_DATA | FILE_WRITE_DATA)
#define IOCTL_CROSEC_RDMEM CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x802, METHOD_BUFFERED, FILE_READ_DATA)

#define CROSEC_CMD_MAX_REQUEST  0x100
#define CROSEC_CMD_MAX_RESPONSE 0x100
#define CROSEC_MEMMAP_SIZE      0xFF

NTSTATUS ConnectToEc(
	_Inout_ HANDLE* Handle
);

#define EC_CMD_MOTION_SENSE     0x002B
#define EC_CMD_RGBKBD_SET_COLOR 0x013A
#define EC_CMD_RGBKBD           0x013B

#define EC_RES_SUCCESS 0
#define EC_INVALID_COMMAND 1
#define EC_ERROR 2
#define EC_INVALID_PARAMETER 3
#define EC_ACCESS_DENIED 4
#define EC_INVALID_RESPONSE 5
#define EC_INVALID_VERSION 6
#define EC_INVALID_CHECKSUM 7

#define CROS_EC_CMD_MAX_REQUEST (0x100-8)

typedef struct _CROSEC_COMMAND {
	UINT32 version;
	UINT32 command;
	UINT32 outlen;
	UINT32 inlen;
	UINT32 result;
	UINT8 data[CROS_EC_CMD_MAX_REQUEST];
} * PCROSEC_COMMAND, CROSEC_COMMAND;

typedef struct _CROSEC_READMEM {
	ULONG offset;
	ULONG bytes;
	UCHAR buffer[CROSEC_MEMMAP_SIZE];
} * PCROSEC_READMEM, CROSEC_READMEM;


#include <pshpack1.h>
#define CROS_EC_CMD_MAX_KEY_COUNT 64
typedef struct {
	UINT8 r;
	UINT8 g;
	UINT8 b;
} Rgb;

typedef struct {
	UINT8 StartKey;
	UINT8 Length;
	Rgb Colors[CROS_EC_CMD_MAX_KEY_COUNT];
} EC_REQUEST_RGB_KBD_SET_COLOR;

typedef struct {
	// Dump = 0
	UINT8 Cmd;
	UINT8 MaxSensorCount;
} EC_REQUEST_MOTION_SENSE_DUMP;

typedef struct {
	UINT8 MaxSensorCount;
	UINT8 SensorCount;
	// Need to allocate extra data if you care about this field.
	// Right now I only care about the count.
	// If this field is not there, the EC just truncates the response.
	// UINT8 Sensors[];
} EC_RESPONSE_MOTION_SENSE_DUMP;

#include <poppack.h>

int CrosEcSendCommand(
	HANDLE Handle,
	UINT16 command,
	UINT8 version,
	LPVOID outdata,
	unsigned int outlen,
	LPVOID indata,
	unsigned int inlen
	);
int CrosEcReadMemU8(HANDLE Handle, unsigned int offset, UINT8* dest);

#ifdef __cplusplus
}
#endif
