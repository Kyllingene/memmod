use std::io::{Write, self};
use crate::Process;


#[derive(Debug)]
pub struct ProcessWriter<'a> {
    proc: &'a mut Process,

    offset: usize,
    data: Vec<u8>,
}

impl<'a> ProcessWriter<'a> {
    pub fn new(proc: &'a mut Process, offset: usize) -> Self {
        Self {
            proc,
            offset,
            data: Vec::new(),
        }
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

        self.offset += self.data.len();
        self.data.clear();

        Ok(())
    }
}
