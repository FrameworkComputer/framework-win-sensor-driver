# Framework Sensor Driver

Sensor Driver for Windows on Framework Computer systems. It reports the lid
accelerometer to Windows for screen rotation.

The driver is a UMDF 2 client of the Sensor Class Extension (SensorsCx),
written in Rust on
[windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs) in the
style of the
[Windows Rust driver samples](https://github.com/microsoft/Windows-rust-driver-samples).
The accelerometer is behind the EC: the driver polls it through the Chrome EC
bus driver's `\Device\CrosEC` (`ACPI\FRMWC004`, see
[crosecbus](https://github.com/FrameworkComputer/crosecbus)).

## Layout

| Path | What |
| --- | --- |
| `sensors-core/` | `no_std`, no WDK dependency: the `\Device\CrosEC` IOCTL ABI, motion sense queries, sample decoding and thresholding, timer scheduling. Unit tested on the host against a simulated EC. |
| `FrameworkSensors/` | The UMDF driver: WDF and SensorsCx callbacks, property lists, the EC handle. |
| `FrameworkSensors/FrameworkSensors.inx` | INF template, unchanged from the C++ driver. |

Mapping from the C++ sources (removed, see git history):

| C++ | Rust |
| --- | --- |
| `Driver.cpp` | `FrameworkSensors/src/driver.rs` |
| `Device.cpp` | `FrameworkSensors/src/device.rs` |
| `Clients.cpp`, `Clients.h` | `FrameworkSensors/src/clients.rs` (callbacks), `sensors-core/src/schedule.rs` (timer) |
| `AccelerometerClient.cpp` | `FrameworkSensors/src/accelerometer.rs`, `sensors-core/src/accel.rs` |
| `AlsClient.cpp` | `FrameworkSensors/src/als.rs`, `sensors-core/src/als.rs` |
| `ComboDevice` (shared sensor state, timer) | `FrameworkSensors/src/sensor.rs` |
| `EcCommunication.cpp`, `ec_commands.h` (parts used) | `sensors-core/src/ec.rs`, `FrameworkSensors/src/ec.rs` |
| `SensorsCx.h` (WDK), `SensorsUtils` macros | `FrameworkSensors/src/sensorscx.rs`, `FrameworkSensors/src/collections.rs` |

`windows-sys` provides the user mode Win32 and SensorsUtils APIs. SensorsCx is
bound through the WDK's `sensorscxstub.lib`, the same way as from C.

## Compatibility

Same hardware ID (`ACPI\FRMWC006`), INF, DLL name, sensor type, persistent
unique ID, properties, data fields, default thresholds (1 g), minimum
interval (4 ms), sample scaling, and EC protocol (one byte memory map reads,
so it also works with C `crosecbus` builds that overrun unaligned multi-byte
reads).

Intentional differences from the C++ driver:

* **Failed EC reads skip the sample** instead of reporting zeros for the
  bytes that couldn't be read (the first half of the unmerged
  `check-busy-bit` branch; the busy bit / sample ID retry is not included).
* **Synchronous EC handle:** `\Device\CrosEC` was opened with
  `FILE_FLAG_OVERLAPPED` but used without an `OVERLAPPED`, which breaks as
  soon as the EC driver pends a request.
* **Sensor state is locked:** the timer callback and SensorsCx callbacks
  shared state without synchronization, and a timer callback racing with
  `EvtSensorStop` could re-arm the timer after it was stopped.
* The polling schedule survives the 32-bit millisecond counter wrapping.
* `EvtSensorStop` no longer calls `timeEndPeriod(1)` without a matching
  `timeBeginPeriod`.
* `EvtDeviceReleaseHardware` only touches the sensors SensorsCx returned,
  null `pSize` arguments are not written to, and `EvtSensorSetDataThresholds`
  checks the list's count against its size.
* `PROPVARIANT` strings and GUIDs are freed with the sensor.
* Tracing uses `OutputDebugString` (`wdk::println!`) instead of WPP.
  Per-sample traces, and the accelerometer status and lid angle reads that
  only fed them, are in debug builds only.
* The ambient light sensor, compiled out of the C++ driver, is behind the
  `als` Cargo feature (off by default, like before). It reports lux only: the
  C++ version also reported constant color temperature and chromaticity and
  claimed to be color capable, and had a -4..4 lux range. The simple device
  orientation sensor, which never read any hardware, was not ported.
* DriverVer is 0.1.0.0.

## Building

Requirements are the same as for the Rust driver samples: the WDK (or EWDK),
LLVM/Clang, and `cargo-make`.

```shell
cargo build --release                 # target/release/FrameworkSensors.dll
cargo test -p sensors-core            # tests against the simulated EC
cargo clippy --all-targets -- -D warnings
cargo +nightly fmt --all              # rustfmt.toml uses unstable options
cargo make                            # stamp INF, infverif, catalog, test-sign into target/*/FrameworkSensors_package
```

The DriverVer stamped by `cargo make` is set in `Makefile.toml`.

The sensors are Cargo features: `accelerometer` (default) and `als`, for
example `cargo build --release --features als` for both, or
`--no-default-features --features als` for just the ambient light sensor.

`cargo make` symlinks its makefiles into `target/`, which on Windows needs
Developer Mode (or an elevated shell). Its signing step creates a self-signed
test certificate in the current user's `WDRTestCertStore`.

CI (`.github/workflows/ci.yml`) runs the formatting check, tests, clippy and
the driver build on stable and nightly, and uploads the test-signed release
package as the `framework_win_sensors_release` artifact.

## Installing

Release builds need to be signed. To try a test-signed build, run
`install-testsigned.ps1` from the package folder as Administrator. It trusts
the package's signing certificate, stages the driver, and force-installs it
on `ACPI\FRMWC006`. The previous driver stays in the driver store, so Device
Manager -> Roll Back Driver switches back.

Otherwise:

```
> sudo pnputil /add-driver FrameworkSensors.inf /install
```

Check if the driver is installed and loaded:

```
# Framework EC driver
> pnputil /enum-devices /deviceid "acpi\frmwc004"
Microsoft PnP Utility

Instance ID:                ACPI\FRMWC004\1
Device Description:         Framework EC
Class Name:                 System
Class GUID:                 {4d36e97d-e325-11ce-bfc1-08002be10318}
Manufacturer Name:          Framework
Status:                     Started
Driver Name:                oem2.inf

# Sensor driver
> pnputil /enum-devices /deviceid "acpi\frmwc006"
Microsoft PnP Utility

Instance ID:                ACPI\FRMWC006\1
Device Description:         Framework Computer Sensor Driver
Class Name:                 Sensor
Class GUID:                 {5175d334-c371-4806-b3ba-71fd53c9258d}
Manufacturer Name:          Framework Computer Inc
Status:                     Started
Driver Name:                oem194.inf
```

Debug output (`FrameworkSensors: ...`) shows up in a debugger attached to
`WUDFHost.exe`, or in DebugView.
