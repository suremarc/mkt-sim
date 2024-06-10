use std::{
    io::{self, Read},
    marker::PhantomData,
};

use zerocopy::{IntoBytes, TryFromBytes};

use crate::{CastResult, Message, Tag};

pub struct MessageStream<T: Tag, R: Read> {
    read: R,
    len: u16,
    buf: Vec<u8>,
    _phantom: PhantomData<T>,
}

impl<T: Tag, R: Read> MessageStream<T, R> {
    pub fn new(read: R) -> Self {
        Self {
            read,
            len: 0,
            buf: vec![0; 65536],
            _phantom: PhantomData,
        }
    }

    pub fn message(&self) -> CastResult<Message<T, [u8]>> {
        Message::try_ref_from(&self.buf)
    }

    pub fn read_next(&mut self) -> io::Result<()> {
        self.read.read_exact(self.len.as_mut_bytes())?;
        self.buf.truncate(self.len as usize);
        self.read.read_exact(&mut self.buf)?;
        Ok(())
    }
}
