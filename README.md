# framework-win-sensor-driver
Sensor Driver for Windows on Framework systems

```
# Build
msbuild .\SensorsComboDriver\SensorsComboDriver.sln /property:Platform=x64 /property:Configuration=Debug

# Distributing
.\SensorsComboDriver\x64\Debug\
    SensorsComboDriver.pdb
    SensorsComboDriver\sensorscombodriver.cat
    SensorsComboDriver\sensorscombodriver.dll
    SensorsComboDriver\sensorscombodriver.inf
# Use signtool to sign .cat file and package all 4 files

# Install
> sudo pnputil /add-driver SensorsComboDriver.inf /install

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
Device Description:         Sensor Driver - ALS, Accelerometer, Orientation
Class Name:                 Sensor
Class GUID:                 {5175d334-c371-4806-b3ba-71fd53c9258d}
Manufacturer Name:          Framework Computer Inc
Status:                     Started
Driver Name:                oem194.inf
```
