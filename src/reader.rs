use std::{
    io::{self, Read},
    ops::{Deref, DerefMut},
};

use crate::Process;

/// A reader for a process.
///
/// Reads `length` bytes at a time. Every read
/// will return the same slice of memory. Sequential
/// reads advance through the process' memory by
/// default. To disable this behavior, use
/// `ProcessReader::no_advance`.
///
/// Can be dereferenced to the underlying `Process`.
#[derive(Debug)]
pub struct ProcessReader<'a> {
    proc: &'a mut Process,

    offset: usize,
    length: usize,
    advance: bool,
}

impl<'a> ProcessReader<'a> {
    /// Create a new process reader.
    pub fn new(proc: &'a mut Process, offset: usize, length: usize) -> Self {
        Self {
            proc,
            offset,
            length,
            advance: true,
        }
    }

    /// Disables advancing through memory.
    pub fn no_advance(mut self) -> Self {
        self.advance = false;
        self
    }

    /// Enables advancing through memory.
    pub fn advance(mut self) -> Self {
        self.advance = true;
        self
    }

    /// Jumps to an offset in memory.
    pub fn goto(&mut self, offset: usize) {
        self.offset = offset;
    }

    /// Jumps to an address in memory.
    pub fn goto_addr(&mut self, address: usize) {
        self.offset = address - self.proc.base.unwrap();
    }
}

impl<'a> Read for ProcessReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let length = buf.len().min(self.length);

        for i in (0..length).step_by(8) {
            let word = self.proc.read_word_offset(self.offset + i)?;

            for j in 0..8 {
                buf[i + j] = ((word >> (j * 8)) & 0xff) as u8;
            }
        }

        if self.advance {
            self.offset += length;
        }

        Ok(length)
    }
}

impl<'a> Deref for ProcessReader<'a> {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        self.proc
    }
}

impl<'a> DerefMut for ProcessReader<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.proc
    }
}
