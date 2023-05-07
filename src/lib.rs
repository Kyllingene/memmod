use std::{
    io::{self, ErrorKind, Read},
    fs::{read_dir, read_to_string},
    os::raw::c_void,
    ptr::null,
};

use nix::{
    sys::{
        ptrace,
        signal::{self, Signal},
        wait::waitpid,
    },
};

pub use nix::{
    errno::Errno,
    unistd::Pid,
};

pub mod reader;
pub mod writer;
pub use reader::ProcessReader;
pub use writer::ProcessWriter;

pub type Address = usize;

fn get_process_status_name(file: &str) -> io::Result<String> {
    let data = read_to_string(file)?;
    let line = data.lines().next().expect("Bad /proc/*/status format");
    if let Some(name) = line.strip_prefix("Name:\t") {
        return Ok(name.to_string());
    }

    Err(io::Error::new(ErrorKind::NotFound, format!("Failed to find name in {file}")))
}

fn check_process_status_file(file: &str, target: &str) -> io::Result<bool> {
    Ok(get_process_status_name(file)?.contains(target))
}

#[derive(Debug)]
pub struct Process {
    pid: Pid,
    stopped: bool,

    name: String,
    base: Option<Address>,
}

impl Process {
    pub fn new(pid: Pid) -> io::Result<Self> {
        ptrace::attach(pid)?;
        waitpid(pid, None)?;

        let name = get_process_status_name(&format!("/proc/{pid}/status"))?;

        Ok(Self {
            pid,
            stopped: true,
            
            name,
            base: None,
        })
    }

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
                return Ok(Self::new(Pid::from_raw(
                    entry.file_name().to_string_lossy().parse().unwrap(),
                ))?);
            }
        }

        Err(io::Error::new(ErrorKind::NotFound, format!("Failed to find process `{target}`")))
    }

    pub fn get_base(&mut self) -> io::Result<()> {
        if self.base.is_some() {
            return Ok(());
        }

        let file = format!("/proc/{}/maps", self.pid);

        let data = read_to_string(file)?;
        for line in data.lines() {
            if line.contains("rw-p") && line.contains(&self.name) {
                let (base, _) = line.split_once('-')
                    .ok_or(Errno::ENOKEY)?;

                self.base = Some(Address::from_str_radix(base, 16).map_err(|_| io::Error::new(ErrorKind::InvalidData, format!("Bad format in /proc/{}/maps", self.pid)))?);
                return Ok(());
            }
        }

        Err(io::Error::new(ErrorKind::NotFound, format!("No suitable mapping in /proc/{}/maps", self.pid)))
    }

    pub fn stop(&mut self) -> io::Result<()> {
        if !self.stopped {
            signal::kill(self.pid, Signal::SIGSTOP)?;
            waitpid(self.pid, None)?;
            self.stopped = true;
        }

        Ok(())
    }

    pub fn cont(&mut self) -> io::Result<()> {
        if self.stopped {
            signal::kill(self.pid, Signal::SIGCONT)?;
            self.stopped = false;
        }

        Ok(())
    }

    pub fn read_word(&mut self, address: Address) -> io::Result<i64> {
        self.stop()?;

        let addr = unsafe {
            null::<c_void>().offset(address as isize) as *mut c_void
        };

        let data = ptrace::read(self.pid, addr)?;
        Ok(data)
    }

    pub fn read_word_offset(&mut self, offset: Address) -> io::Result<i64> {
        self.get_base()?;
        self.read_word(self.base.unwrap() + offset)
    }

    pub fn write_word(&mut self, address: Address, data: i64) -> io::Result<()> {
        self.stop()?;

        let addr = unsafe {
            null::<c_void>().offset(address as isize) as *mut c_void
        };

        let data = unsafe {
            null::<c_void>().offset(data as isize) as *mut c_void
        };

        unsafe {
            ptrace::write(self.pid, addr, data)?;
        }

        Ok(())
    }

    pub fn write_word_offset(&mut self, offset: Address, data: i64) -> io::Result<()> {
        self.get_base()?;
        self.write_word(self.base.unwrap() + offset, data)
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn reader<'a>(&'a mut self, offset: Address, length: usize) -> ProcessReader<'a> {
        ProcessReader::new(
            self,
            offset,
            length
        )
    }

    pub fn writer<'a>(&'a mut self, offset: Address) -> ProcessWriter<'a> {
        ProcessWriter::new(
            self,
            offset
        )
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

impl Read for Process {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let data = self.read_word_offset(0)?;

        for i in 0..4.min(buf.len()) {
            buf[i] =  ((data >> (i * 8)) & 0xff) as u8;
        }

        Ok(4.min(buf.len()))
    }
}
