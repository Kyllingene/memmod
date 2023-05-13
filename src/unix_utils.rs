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
    errno::Errno,
    unistd::Pid,
};

pub type Handle = Pid;

pub fn read_process_word() {
    todo!()
}

pub fn write_process_word() {
    todo!()
}

pub fn get_process_name(file: &str) -> io::Result<String> {
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

pub fn check_process_name(file: &str, target: &str) -> io::Result<bool> {
    Ok(get_process_status_name(file)?.contains(target))
}

pub fn check_process_name_strict(file: &str, target: &str) -> io::Result<bool> {
    Ok(get_process_status_name(file)? == target)
}

pub fn get_base(handle: Handle) -> io::Result<usize> {
    let file = format!("/proc/{}/maps", self.pid);

    let data = read_to_string(file)?;
    let line = data.lines().next().ok_or(Errno::ENOKEY)?;
    let (base, _) = line.split_once('-').ok_or(Errno::ENOKEY)?;
    usize::from_str_radix(base, 16).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("Bad format in /proc/{}/maps", self.pid),
        )
    })?
}

pub fn get_process_handle(pid: i32) -> io::Result<Handle> {
    ptrace::attach(pid)?;
    waitpid(pid, None)?;
    ptrace::cont(pid, None)?;
    waitpid(pid, None)?;
}
