//! `\Device\CrosEC`, the user mode interface of the Chrome EC bus driver.

use core::{ffi::c_void, mem::size_of, ptr};

use sensors_core::ec::{
    CrosEcCommand,
    CrosEcReadMem,
    EcDevice,
    IOCTL_CROSEC_RDMEM,
    IOCTL_CROSEC_XCMD,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle,
        GetLastError,
        GENERIC_READ,
        GENERIC_WRITE,
        HANDLE,
        INVALID_HANDLE_VALUE,
    },
    Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    System::IO::DeviceIoControl,
};

const CROSEC_DEVICE_PATH: &str = r"\\.\GLOBALROOT\Device\CrosEC";

/// An open handle to `\Device\CrosEC`, closed on drop.
pub struct CrosEc(HANDLE);

impl CrosEc {
    /// `ConnectToEc`
    ///
    /// The handle is synchronous. The C driver opened it with
    /// `FILE_FLAG_OVERLAPPED` but passed no `OVERLAPPED`, which only works as
    /// long as the EC driver never pends a request.
    pub fn open() -> Result<Self, u32> {
        let path: Vec<u16> = CROSEC_DEVICE_PATH.encode_utf16().chain([0]).collect();
        // SAFETY: path is NUL terminated; everything else is plain values.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: No preconditions.
            return Err(unsafe { GetLastError() });
        }
        Ok(Self(handle))
    }

    /// Sends `code` with `buffer` as both input and output buffer.
    fn ioctl<T>(&mut self, code: u32, buffer: &mut T) -> Result<(), u32> {
        let size = u32::try_from(size_of::<T>()).expect("IOCTL buffers are small");
        let buffer = ptr::from_mut(buffer).cast::<c_void>();
        let mut returned = 0;
        // SAFETY: buffer is valid for size bytes in both directions, and the
        // handle is synchronous so it is not used after this returns.
        let ok = unsafe {
            DeviceIoControl(
                self.0,
                code,
                buffer,
                size,
                buffer,
                size,
                &raw mut returned,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            // SAFETY: No preconditions.
            return Err(unsafe { GetLastError() });
        }
        Ok(())
    }
}

impl EcDevice for CrosEc {
    fn xcmd(&mut self, cmd: &mut CrosEcCommand) -> Result<(), u32> {
        self.ioctl(IOCTL_CROSEC_XCMD, cmd)
    }

    fn rdmem(&mut self, rm: &mut CrosEcReadMem) -> Result<(), u32> {
        self.ioctl(IOCTL_CROSEC_RDMEM, rm)
    }
}

impl Drop for CrosEc {
    fn drop(&mut self) {
        // SAFETY: We own the handle.
        unsafe { CloseHandle(self.0) };
    }
}
