use std::{
    io::{self, ErrorKind, Read},
    os::raw::c_void,
    ptr::null,
};

use crate::{Process, ProcessReader, ProcessWriter};

use windows::Win32::System::Threading::OpenProcess;

pub type Handle = windows::Win32::Foundation::HANDLE;

pub fn read_process_word(handle: Handle, address: usize) -> io::Result<isize> {
    todo!()
}

pub fn write_process_word(handle: Handle, address: usize, data: isize) -> io::Result<()> {
    todo!()
}

pub fn get_process_name(handle: Handle) -> io::Result<String> {
    todo!()
}

pub fn find_process(name: &str, check: impl Fn(&str, &str) -> bool) -> io::Result<Handle> {
    todo!()
}

pub fn check_process_name(name: &str, target: &str) -> bool {
    name.contains(target)
}

pub fn check_process_name_strict(name: &str, target: &str) -> bool {
    name == target
}

pub fn get_base(handle: Handle) -> io::Result<usize> {
    todo!()
}

pub fn get_process_handle(pid: i32) -> io::Result<Handle> {
    todo!()
}
