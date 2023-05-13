use std::{
    fs::{read_dir, read_to_string},
    io::{self, ErrorKind, Read},
    os::raw::c_void,
    ptr::null,
};

#[cfg(unix)]
pub mod unix;

#[cfg(unix)]
use unix::{
    read_process_word,
    write_process_word,
    check_process_name,
    check_process_name_strict,
    get_process_name,
    get_process_handle,
    Handle
};

#[cfg(unix)]
pub use nix::unistd::Pid;

#[cfg(windows)]
pub mod windows_utils;

use windows_utils::{find_process, get_base};
#[cfg(windows)]
use windows_utils::{
    read_process_word,
    write_process_word,
    check_process_name,
    check_process_name_strict,
    get_process_name,
    get_process_handle,
    Handle
};

pub mod reader;
pub mod writer;

pub use reader::ProcessReader;
pub use writer::ProcessWriter;

const POINTER_WIDTH: usize = usize::BITS as usize / 8;

/// An attached process.
///
/// To attach to a process, call `Process::new(pid)`. To find a process by
/// name (just checks for string inclusion), use `Process::find(name)`. To
/// detach from a process, drop this struct (or call `Process::detach()` for
/// proper error handling).
///
/// Modifying a process' memory stops the process. To continue it, use `Process::cont()`,
/// or detach. Reading does not stop the process; you must stop it yourself.
#[derive(Debug)]
pub struct Process {
    handle: Handle,
    stopped: bool,

    name: String,
    base: Option<usize>,
}

impl Process {
    /// Attach to a process.
    // ///
    // /// Also reads its name from `/proc/<pid>/status`. If that fails, so will
    // /// the method.
    pub fn new(pid: i32) -> io::Result<Self> {
        let handle = get_process_handle(pid)?;
        Self::from_handle(handle)
    }

    fn from_handle(handle: Handle) -> io::Result<Self> {
        let name = get_process_name(handle)?;

        Ok(Self {
            handle,
            stopped: false,

            name,
            base: None,
        })
    }

    /// Finds a process by name, then calls `Process::new`. Simply checks for string inclusion (e.g.
    /// `myapp` will match both `./myapp --gui` and `find / | grep myapp`, whichever has a lower pid).
    pub fn find(target: &str) -> io::Result<Self> {
        Self::from_handle(find_process(target, check_process_name)?)
    }

    /// Finds a process by name, then calls `Process::new`. Only allows strict matches (e.g.
    /// `myapp` won't match `./myapp --gui` or `find / | grep myapp`).
    pub fn find_strict(target: &str) -> io::Result<Self> {
        Self::from_handle(find_process(target, check_process_name_strict)?)
    }

    /// Gets the base address of the process' memory (the first mapping in /proc/pid/maps).
    ///
    /// If it hasn't been called yet, calling `<read/write>_word_offset` will call this first.
    pub fn get_base(&mut self) -> io::Result<()> {
        if self.base.is_some() {
            return Ok(());
        }

        self.base = Some(get_base(self.handle)?);
        
        Ok(())
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

    /// Detaches from the process.
    /// 
    /// This consumes the struct.
    pub fn detach(mut self) -> io::Result<()> {
        self.detach_without_consuming()
    }

    fn detach_without_consuming(&mut self) -> io::Result<()> {
        let sig = if self.stopped {
            Some(Signal::SIGCONT)
        } else {
            None
        };

        ptrace::detach(self.pid, sig).map_err(Errno::into)
    }

    /// Reads a single word from the process' memory.
    pub fn read_word(&mut self, address: usize) -> io::Result<isize> {
        let addr = unsafe { null::<c_void>().add(address) as *mut c_void };

        let data = ptrace::read(self.pid, addr)? as isize;
        Ok(data)
    }

    /// Reads a single word from the process' memory, using `offset`.
    ///
    /// If `Process::get_base()` hasn't been called yet, calls that first.
    pub fn read_word_offset(&mut self, offset: usize) -> io::Result<isize> {
        self.get_base()?;
        self.read_word(self.base.unwrap() + offset)
    }

    /// Writes a single word into the process' memory.
    pub fn write_word(&mut self, address: usize, data: isize) -> io::Result<()> {
        self.stop()?;

        let addr = unsafe { null::<c_void>().add(address) as *mut c_void };

        let data = unsafe { null::<c_void>().offset(data) as *mut c_void };

        unsafe {
            ptrace::write(self.pid, addr, data)?;
        }

        Ok(())
    }

    /// Writes a single word into the process' memory, using `offset`.
    ///
    /// If `Process::get_base()` hasn't been called yet, calls that first.
    pub fn write_word_offset(&mut self, offset: usize, data: isize) -> io::Result<()> {
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

    /// Returns a `ProcessReader` for this process, good for `length` bytes, starting at `address`.
    pub fn reader(&mut self, address: usize, length: usize) -> io::Result<ProcessReader> {
        self.get_base()?;
        Ok(ProcessReader::new(self, address, length))
    }

    /// Returns a `ProcessWriter` for this process, starting at `address`.
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
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Err(e) = self.detach_without_consuming() {
            panic!(
                "Failed to detach from process {}: {e}",
                self.pid
            );
        }
    }
}
