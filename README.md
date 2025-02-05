# framework-win-sensor-driver
Sensor Driver for Windows on Framework systems

```
# Build
msbuild .\SensorsComboDriver\SensorsComboDriver.sln /property:Platform=x64 /property:Configuration=Debug

# First install
# TODO: Should move to ACPI\FRMWC006
> sudo .\devcon.exe install .\SensorsComboDriver\x64\Debug\SensorsComboDriver\SensorsComboDriver.inf umdf2\SensorsComboDriver

# Subsequent installs
> sudo pnputil /add-driver .\SensorsComboDriver\x64\Debug\SensorsComboDriver\SensorsComboDriver.inf /install

# Distributing
.\SensorsComboDriver\x64\Debug\
    SensorsComboDriver.pdb
    SensorsComboDriver\sensorscombodriver.cat
    SensorsComboDriver\sensorscombodriver.dll
    SensorsComboDriver\sensorscombodriver.inf
# Use signtool to sign .cat file and package all 4 files
```
