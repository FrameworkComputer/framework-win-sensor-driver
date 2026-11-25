use anyhow::Context;

fn main() -> anyhow::Result<()> {
    wdk_build::configure_wdk_binary_build()?;

    // The Sensor Class Extension stub lives in the SDK's um\<arch>\sensors\1.1,
    // which wdk-build doesn't add to the search path. Linking it binds the
    // driver to SensorsCx (see src/sensorscx.rs).
    let stub_dir = wdk_build::Config::from_env_auto()?
        .library_paths()?
        .map(|path| path.join(r"sensors\1.1"))
        .find(|path| path.join("sensorscxstub.lib").is_file())
        .context(r"sensors\1.1\sensorscxstub.lib not found in the WDK library paths")?;
    println!("cargo::rustc-link-search=native={}", stub_dir.display());
    println!("cargo::rustc-link-lib=static=sensorscxstub");

    Ok(())
}
