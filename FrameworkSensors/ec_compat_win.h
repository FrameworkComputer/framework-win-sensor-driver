/*++

SPDX-License-Identifier: MS-PL

Copyright (C) Framework Computer Inc, All Rights Reserved.

Module Name:

    ec_compat_win.h

Abstract:

    Windows compatibility layer for ec_commands.h from ChromiumOS EC.
    Include this header BEFORE ec_commands.h to provide the necessary
    type definitions and compiler attributes for MSVC.

Environment:

    User-mode Driver Framework 2

--*/

#pragma once

#include <windows.h>

/*
 * Suppress MSVC warnings that are unavoidable in ec_commands.h:
 * C4200: nonstandard extension - zero-sized array in struct/union
 *        (used for flexible array members)
 */
#pragma warning(push)
#pragma warning(disable: 4200)

/*
 * Provide stdint.h-compatible type definitions using Windows types.
 * These must be defined before ec_commands.h is included.
 */
#ifndef _STDINT_WIN_COMPAT
#define _STDINT_WIN_COMPAT

typedef UINT8  uint8_t;
typedef UINT16 uint16_t;
typedef UINT32 uint32_t;
typedef UINT64 uint64_t;
typedef INT8   int8_t;
typedef INT16  int16_t;
typedef INT32  int32_t;
typedef INT64  int64_t;

#ifndef UINT16_MAX
#define UINT16_MAX 0xFFFF
#endif

#endif /* _STDINT_WIN_COMPAT */

/*
 * MSVC doesn't support __attribute__((packed)) or __attribute__((aligned(n))).
 * We define __packed as empty and rely on #pragma pack() in the including file.
 * For __aligned, MSVC uses __declspec(align(n)).
 */
#ifndef __packed
#define __packed
#endif

#ifndef __aligned
#define __aligned(x) __declspec(align(x))
#endif

/*
 * Suppress the BUILD_ASSERT macro from ec_commands.h since it may use
 * constructs not available in MSVC.
 */
#ifndef BUILD_ASSERT
#define BUILD_ASSERT(_cond)
#endif
