use std::{
    fs::{read_dir, read_to_string},
    io::{self, ErrorKind, Read},
    os::raw::c_void,
    ptr::null,
};

use nix::sys::{
    ptrace,
    signal::{self, Signal},
    wait::waitpid,
};

pub use nix::{errno::Errno, unistd::Pid};

pub mod reader;
pub mod writer;

pub use reader::ProcessReader;
pub use writer::ProcessWriter;

const POINTER_WIDTH: usize = usize::BITS as usize / 8;

fn get_process_status_name(file: &str) -> io::Result<String> {
    let data = read_to_string(file)?;
    let line = data.lines().next().expect("Bad /proc/*/status format");
    if let Some(name) = line.strip_prefix("Name:\t") {
        return Ok(name.to_string());
    }

    Err(io::Error::new(
        ErrorKind::NotFound,
        format!("Failed to find name in {file}"),
    ))
}

fn check_process_status_file(file: &str, target: &str) -> io::Result<bool> {
    Ok(get_process_status_name(file)?.contains(target))
}

fn check_process_status_file_strict(file: &str, target: &str) -> io::Result<bool> {
    Ok(get_process_status_name(file)? == target)
}

/// An attached process.
///
/// To attach to a process, call `Process::new(pid)`. To find a process by
/// name (just checks for string inclusion), use `Process::find(name)`. To
/// detach from a process, drop this struct.
///
/// Attaching to a process, as well as reading/writing its memory, stops
/// the process. To continue it, use `Process::cont()`, or detach.
#[derive(Debug)]
pub struct Process {
    pid: Pid,
    stopped: bool,

    name: String,
    base: Option<usize>,
}

impl Process {
    /// Attach to a process.
    ///
    /// Also reads its name from `/proc/<pid>/status`. If that fails, so will
    /// the method.
    pub fn new(pid: Pid) -> io::Result<Self> {
        // Call this first in case it fails
        let name = get_process_status_name(&format!("/proc/{pid}/status"))?;

        ptrace::attach(pid)?;
        waitpid(pid, None)?;

        Ok(Self {
            pid,
            stopped: true,

            name,
            base: None,
        })
    }

    /// Finds a process by name, then calls `Process::new`. Simply checks for string inclusion (e.g.
    /// `myapp` will match both `./myapp --gui` and `find / | grep myapp`, whichever has a lower pid).
    pub fn find(target: &str) -> io::Result<Self> {
        let dir = read_dir("/proc")?;

        for entry in dir {
            let entry = entry?;
            if !entry
                .file_name()
                .to_string_lossy()
                .chars()
                .all(char::is_numeric)
            {
                continue;
            }

            if check_process_status_file(
                &format!("/proc/{}/status", entry.file_name().to_string_lossy()),
                target,
            )? {
                return Self::new(Pid::from_raw(
                    entry.file_name().to_string_lossy().parse().unwrap(),
                ));
            }
        }

        Err(io::Error::new(
            ErrorKind::NotFound,
            format!("Failed to find process `{target}`"),
        ))
    }

    /// Finds a process by name, then calls `Process::new`. Only allows strict matches (e.g.
    /// `myapp` won't match `./myapp --gui` and `find / | grep myapp`).
    pub fn find_strict(target: &str) -> io::Result<Self> {
        let dir = read_dir("/proc")?;

        for entry in dir {
            let entry = entry?;
            if !entry
                .file_name()
                .to_string_lossy()
                .chars()
                .all(char::is_numeric)
            {
                continue;
            }

            if check_process_status_file_strict(
                &format!("/proc/{}/status", entry.file_name().to_string_lossy()),
                target,
            )? {
                return Self::new(Pid::from_raw(
                    entry.file_name().to_string_lossy().parse().unwrap(),
                ));
            }
        }

        Err(io::Error::new(
            ErrorKind::NotFound,
            format!("Failed to find process `{target}`"),
        ))
    }

    /// Gets the base address of the process' r/w memory (the first entry in `/proc/<pid>/maps` that
    /// contains both `rw-p` and the process name).
    ///
    /// If it hasn't been called yet, calling `<read/write>_word_offset` will call this first.
    pub fn get_base(&mut self) -> io::Result<()> {
        if self.base.is_some() {
            return Ok(());
        }

        let file = format!("/proc/{}/maps", self.pid);

        let data = read_to_string(file)?;
        for line in data.lines() {
            if line.contains("rw-p") && line.contains(&self.name) {
                let (base, _) = line.split_once('-').ok_or(Errno::ENOKEY)?;

                self.base = Some(usize::from_str_radix(base, 16).map_err(|_| {
                    io::Error::new(
                        ErrorKind::InvalidData,
                        format!("Bad format in /proc/{}/maps", self.pid),
                    )
                })?);
                return Ok(());
            }
        }

        Err(io::Error::new(
            ErrorKind::NotFound,
            format!("No suitable mapping in /proc/{}/maps", self.pid),
        ))
    }

    /// Halts the process.
    ///
    /// Called before all read/write operations.
    pub fn stop(&mut self) -> io::Result<()> {
        if !self.stopped {
            signal::kill(self.pid, Signal::SIGSTOP)?;
            waitpid(self.pid, None)?;
            self.stopped = true;
        }

        Ok(())
    }

    /// Continues the process.
    ///
    /// This is never called automatically.
    pub fn cont(&mut self) -> io::Result<()> {
        if self.stopped {
            signal::kill(self.pid, Signal::SIGCONT)?;
            self.stopped = false;
        }

        Ok(())
    }

    /// Reads a single i64 from the process' memory.
    pub fn read_word(&mut self, address: usize) -> io::Result<i64> {
        self.stop()?;

        let addr = unsafe { null::<c_void>().add(address) as *mut c_void };

        let data = ptrace::read(self.pid, addr)?;
        Ok(data)
    }

    /// Reads a single i64 from the process' memory, using `offset`.
    ///
    /// If `Process::get_base()` hasn't been called yet, calls that first.
    pub fn read_word_offset(&mut self, offset: usize) -> io::Result<i64> {
        self.get_base()?;
        self.read_word(self.base.unwrap() + offset)
    }

    /// Writes a single i64 into the process' memory.
    pub fn write_word(&mut self, usize: usize, data: i64) -> io::Result<()> {
        self.stop()?;

        let addr = unsafe { null::<c_void>().add(usize) as *mut c_void };

        let data = unsafe { null::<c_void>().offset(data as isize) as *mut c_void };

        unsafe {
            ptrace::write(self.pid, addr, data)?;
        }

        Ok(())
    }

    /// Writes a single i64 into the process' memory, using `offset`.
    ///
    /// If `Process::get_base()` hasn't been called yet, calls that first.
    pub fn write_word_offset(&mut self, offset: usize, data: i64) -> io::Result<()> {
        self.get_base()?;
        self.write_word(self.base.unwrap() + offset, data)
    }

    /// Resolves a chain of pointer offsets.
    pub fn pointer_chain(&mut self, mut address: usize, offsets: Vec<isize>) -> io::Result<usize> {        
        let mut reader = self.reader(address, POINTER_WIDTH)?.no_advance();

        let mut address_bytes = [0; POINTER_WIDTH];
        for offset in offsets.iter() {
            reader.goto(address);
            reader.read_exact(&mut address_bytes)?;
            address = usize::from_le_bytes(address_bytes);

            if *offset >= 0 {
       			address += *offset as usize;
       		} else {
       			address -= offset.unsigned_abs();
       		}

        }

        Ok(address)
    }

    /// Returns the pid of the attached process.
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// Returns the full name of the attached process.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Returns the base address of the attached process.
    pub fn base(&mut self) -> io::Result<usize> {
        self.get_base()?;
        Ok(self.base.unwrap())
    }

    /// Returns a `ProcessReader` for this process, good for `length` bytes.
    pub fn reader(&mut self, address: usize, length: usize) -> io::Result<ProcessReader> {
        self.get_base()?;
        Ok(ProcessReader::new(self, address, length))
    }

    /// Returns a `ProcessWriter` for this process.
    pub fn writer(&mut self, address: usize) -> io::Result<ProcessWriter> {
        self.get_base()?;
        Ok(ProcessWriter::new(self, address))
    }

    /// Returns a `ProcessReader` for this process, good for `length` bytes, starting at `offset`.
    pub fn reader_offset(&mut self, offset: isize, length: usize) -> io::Result<ProcessReader> {
        self.get_base()?;
        Ok(ProcessReader::offset(self, offset, length))
    }

    /// Returns a `ProcessWriter` for this process, starting at `offset`.
    pub fn writer_offset(&mut self, offset: isize) -> io::Result<ProcessWriter> {
        self.get_base()?;
        Ok(ProcessWriter::offset(self, offset))
    }

    /// Returns a `ProcessReader` for this process, good for `length` bytes, starting at `address`.
    pub fn reader_addr(&mut self, address: usize, length: usize) -> io::Result<ProcessReader> {
        let base = self.base()?;
        Ok(ProcessReader::new(self, address - base, length))
    }

    /// Returns a `ProcessWriter` for this process, starting at `address`.
    pub fn writer_addr(&mut self, address: usize) -> io::Result<ProcessWriter> {
        let base = self.base()?;
        Ok(ProcessWriter::new(self, address - base))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let sig = if self.stopped {
            Some(Signal::SIGCONT)
        } else {
            None
        };

        if let Err(e) = ptrace::detach(self.pid, sig) {
            panic!(
                "Failed to detach from process {} (tried to send signal {sig:?}): {e}",
                self.pid
            );
        }
    }
}
