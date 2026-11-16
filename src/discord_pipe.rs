//! The connection to Discord's local IPC: a Unix socket on Linux, a named pipe on Windows.

/// Where Discord's socket can be: the runtime and temporary folders, and the folders of the
/// Flatpak and Snap builds and of Vesktop.
#[cfg(unix)]
fn socket_candidates() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;

    let mut roots: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    roots.push(PathBuf::from("/tmp"));
    let nested = [
        "",
        "app/com.discordapp.Discord",
        "app/com.discordapp.DiscordCanary",
        "snap.discord",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
    ];
    roots
        .iter()
        .flat_map(|root| nested.iter().map(move |inner| root.join(inner)))
        .flat_map(|dir| (0..10).map(move |index| dir.join(format!("discord-ipc-{index}"))))
        .collect()
}

fn not_open() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "Discord is not open")
}

#[cfg(unix)]
pub use std::os::unix::net::UnixStream as Pipe;

/// Connects to the first Discord socket that answers.
#[cfg(unix)]
pub fn open() -> std::io::Result<Pipe> {
    socket_candidates()
        .into_iter()
        .find_map(|path| Pipe::connect(path).ok())
        .ok_or_else(not_open)
}

#[cfg(windows)]
pub use windows::{Pipe, open};

#[cfg(windows)]
mod windows {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::windows::io::AsRawHandle;
    use std::sync::Arc;
    use std::time::Duration;

    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    /// How often a reader looks for what Discord sent.
    const POLL: Duration = Duration::from_millis(50);

    /// One of Discord's named pipes, shared by the thread that writes and the one that reads.
    pub struct Pipe(Arc<File>);

    impl Pipe {
        /// Another handle on the same connection, for the reading thread.
        pub fn try_clone(&self) -> std::io::Result<Self> {
            Ok(Self(Arc::clone(&self.0)))
        }
    }

    impl Read for Pipe {
        /// A read waiting on a pipe opened for blocking I/O holds up every write to it, and
        /// Encore writes while it waits for Discord's events: it only reads what has arrived.
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            loop {
                let mut available = 0u32;
                // SAFETY: the handle is the open pipe; only the byte count is asked for.
                let peeked = unsafe {
                    PeekNamedPipe(
                        self.0.as_raw_handle(),
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut available,
                        std::ptr::null_mut(),
                    )
                };
                if peeked == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if available > 0 {
                    let wanted = buf.len().min(available as usize);
                    return (&*self.0).read(&mut buf[..wanted]);
                }
                std::thread::sleep(POLL);
            }
        }
    }

    impl Write for Pipe {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            (&*self.0).write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            (&*self.0).flush()
        }
    }

    /// Connects to the first of Discord's pipes that answers.
    pub fn open() -> std::io::Result<Pipe> {
        (0..10)
            .find_map(|index| {
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!(r"\\.\pipe\discord-ipc-{index}"))
                    .ok()
            })
            .map(|file| Pipe(Arc::new(file)))
            .ok_or_else(super::not_open)
    }
}
