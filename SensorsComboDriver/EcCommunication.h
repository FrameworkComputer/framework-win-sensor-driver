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
#define EC_MEMMAP_ACC_STATUS_BUSY_BIT BIT(4)
#define EC_MEMMAP_ACC_STATUS_PRESENCE_BIT BIT(7)

#define FILE_DEVICE_CROS_EMBEDDED_CONTROLLER 0x80EC

#define IOCTL_CROSEC_XCMD \
	CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x801, METHOD_BUFFERED, FILE_READ_DATA | FILE_WRITE_DATA)
#define IOCTL_CROSEC_RDMEM CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x802, METHOD_BUFFERED, FILE_READ_DATA)

#define CROSEC_CMD_MAX_REQUEST  0x100
#define CROSEC_CMD_MAX_RESPONSE 0x100
#define CROSEC_MEMMAP_SIZE      0xFF

typedef struct _CROSEC_READMEM {
	ULONG offset;
	ULONG bytes;
	UCHAR buffer[CROSEC_MEMMAP_SIZE];
} * PCROSEC_READMEM, CROSEC_READMEM;

int CrosEcReadMemU8(HANDLE Handle, unsigned int offset, UINT8* dest);

#ifdef __cplusplus
}
#endif
