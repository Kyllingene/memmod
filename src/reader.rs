use std::io::{Read, self, ErrorKind};

use crate::Process;

#[derive(Debug)]
pub struct ProcessReader<'a> {
    proc: &'a mut Process,

    offset: usize,
    length: usize,
}

impl<'a> ProcessReader<'a> {
    pub fn new(proc: &'a mut Process, offset: usize, length: usize) -> Self {
        Self {
            proc,
            offset,
            length
        }
    }
}

impl<'a> Read for ProcessReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.len() < self.length {
            return Err(io::Error::new(ErrorKind::OutOfMemory, format!("Expected at least {} bytes of space, found {}", self.length, buf.len())));
        }

        for i in (0..self.length).step_by(8) {
            let word = self.proc.read_word_offset(self.offset + i)?;

            for j in 0..8 {
                buf[i+j] = ((word >> (j * 8)) & 0xff) as u8;
            }
        }

        Ok(self.length)
    }
}
