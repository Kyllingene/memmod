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

    offset: usize,
    data: Vec<u8>,
    advance: bool,
}

impl<'a> ProcessWriter<'a> {
    /// Create a new process writer. Advances by default.
    pub fn new(proc: &'a mut Process, offset: usize) -> Self {
        Self {
            proc,
            offset,
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

    /// Jumps to an offset in memory.
    pub fn goto(&mut self, offset: usize) {
        self.offset = offset;
    }

    /// Jumps to an address in memory.
    pub fn goto_addr(&mut self, address: usize) {
        self.offset = address - self.proc.base.unwrap();
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

                let mut source = self.proc.read_word_offset(self.offset + wordi * 8)?;
                source &= i64::MAX >> (difference * 8);
                word |= source;

                self.proc.write_word_offset(self.offset + wordi * 8, word)?;

                break;
            }

            if (i + 1) % 8 == 0 {
                self.proc.write_word_offset(self.offset + wordi * 8, word)?;
                wordi += 1;
            }
        }

        if self.advance {
            self.offset += self.data.len();
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
