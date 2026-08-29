//! 外部BMSエディタが別プロセスで起動する `-S` を、実行中ビューワーへ中継する。

use std::io;

use anyhow::{Context, Result};

#[cfg(windows)]
const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\bmz-player-viewer-v1";
#[cfg(not(windows))]
const LOOPBACK_ADDRESS: &str = "127.0.0.1:39076";

/// 実行中のビューワーへ停止要求を送る。未起動なら `false`。
pub fn request_stop() -> Result<bool> {
    platform::request_stop()
}

/// 停止要求を1回受け取るlistenerを開始する。
pub fn start_stop_listener(on_stop: impl FnOnce() + Send + 'static) -> Result<()> {
    platform::start_stop_listener(on_stop)
}

#[cfg(windows)]
mod platform {
    use std::fs::OpenOptions;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::mpsc;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    use super::*;

    pub(super) fn request_stop() -> Result<bool> {
        match OpenOptions::new().write(true).open(WINDOWS_PIPE_NAME) {
            Ok(_) => Ok(true),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error).context("failed to connect to the BMZ viewer pipe"),
        }
    }

    pub(super) fn start_stop_listener(on_stop: impl FnOnce() + Send + 'static) -> Result<()> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("viewer-stop-listener".to_string())
            .spawn(move || {
                let name: Vec<u16> = std::ffi::OsStr::new(WINDOWS_PIPE_NAME)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let handle = unsafe {
                    CreateNamedPipeW(
                        name.as_ptr(),
                        PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                        1,
                        0,
                        0,
                        0,
                        std::ptr::null(),
                    )
                };
                if handle == INVALID_HANDLE_VALUE {
                    let _ = ready_tx.send(Err(io::Error::last_os_error()));
                    return;
                }
                let _ = ready_tx.send(Ok(()));
                let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0;
                let connected = connected || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                if connected {
                    on_stop();
                }
                unsafe {
                    CloseHandle(handle);
                }
            })
            .context("failed to spawn the viewer stop listener")?;
        ready_rx
            .recv()
            .context("viewer stop listener exited before initialization")?
            .context("failed to create the BMZ viewer pipe")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn stop_request_round_trips_to_listener() {
        let (tx, rx) = mpsc::sync_channel(1);
        start_stop_listener(move || tx.send(()).unwrap()).unwrap();

        assert!(request_stop().unwrap());
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}

#[cfg(not(windows))]
mod platform {
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};

    use super::*;

    pub(super) fn request_stop() -> Result<bool> {
        match TcpStream::connect(LOOPBACK_ADDRESS) {
            Ok(mut stream) => {
                stream.write_all(b"STOP")?;
                Ok(true)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error).context("failed to connect to the BMZ viewer listener"),
        }
    }

    pub(super) fn start_stop_listener(on_stop: impl FnOnce() + Send + 'static) -> Result<()> {
        let listener = TcpListener::bind(LOOPBACK_ADDRESS)
            .context("failed to bind the BMZ viewer listener")?;
        std::thread::Builder::new()
            .name("viewer-stop-listener".to_string())
            .spawn(move || {
                if listener.accept().is_ok() {
                    on_stop();
                }
            })
            .context("failed to spawn the viewer stop listener")?;
        Ok(())
    }
}
