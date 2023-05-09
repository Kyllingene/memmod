use std::{
    io::{self, Write},
    ops::{Deref, DerefMut},
};

use crate::Process;

/// A writer for a process.
///
/// Subsequent writes advance the reader through the process'
/// memory by default. To disable this behavior, call
/// `ProcessWriter::no_advance`.
///
/// Can be dereferenced to the underlying `Process`.
#[derive(Debug)]
pub struct ProcessWriter<'a> {
    proc: &'a mut Process,

    address: usize,
    data: Vec<u8>,
    advance: bool,
}

impl<'a> ProcessWriter<'a> {
    /// Create a new process writer. Advances by default.
    pub fn new(proc: &'a mut Process, address: usize) -> Self {
        Self {
            proc,
            address,
            data: Vec::new(),
            advance: true,
        }
    }

    /// Create a new process writer. Advances by default.
    pub fn offset(proc: &'a mut Process, offset: isize) -> Self {
        let base = proc.base.unwrap();
        let address = if offset >= 0 {
            base + offset as usize
        } else {
            base - offset as usize
        };
        Self {
            proc,
            address,
            data: Vec::new(),
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

    /// Jumps to an address in memory.
    pub fn goto(&mut self, address: usize) {
        self.address = address;
    }
    
    /// Jumps to an offset in memory.
    pub fn goto_offset(&mut self, offset: isize) {
        self.address = if offset >= 0 {
            self.proc.base().unwrap() + offset as usize
        } else {
            self.proc.base().unwrap() - offset as usize
        };
    }
}

impl<'a> Write for ProcessWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for byte in buf {
            self.data.push(*byte);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut word = 0;
        let mut wordi = 0;
        for mut i in 0..self.data.len() {
            if i % 8 == 0 {
                word = 0;
            }

            word |= (self.data[i] as i64) << ((i % 8) * 8);

            if self.data.len() % 8 != 0 && i / 8 == self.data.len() / 8 {
                let difference = self.data.len() - i;
                i += 1;

                for i in i..self.data.len() {
                    word |= (self.data[i] as i64) << ((i % 8) * 8);
                }

                let mut source = self.proc.read_word(self.address + wordi * 8)?;
                source &= i64::MAX << (difference * 8);
                word |= source;

                self.proc.write_word(self.address + wordi * 8, word)?;

                break;
            }

            if (i + 1) % 8 == 0 {
                self.proc.write_word(self.address + wordi * 8, word)?;
                wordi += 1;
            }
        }

        if self.advance {
            self.address += self.data.len();
        }

        self.data.clear();

        Ok(())
    }
}

impl<'a> Deref for ProcessWriter<'a> {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        self.proc
    }
}

impl<'a> DerefMut for ProcessWriter<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.proc
    }
}

impl<'a> Drop for ProcessWriter<'a> {
    fn drop(&mut self) {
        if !self.data.is_empty() {
            if let Err(e) = self.flush() {
                panic!("Writer for process {} (at 0x{:x}) dropped without flushing, but an error occurred while flushing: {e}", self.pid, self.address);
            }
        }
    }
}
