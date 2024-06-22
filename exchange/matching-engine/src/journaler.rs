use std::{borrow::Borrow, collections::BTreeMap, io, mem::ManuallyDrop};

use bbqueue::{BBBuffer, Consumer, GrantW, Producer};
use glommio::{
    io::DmaStreamWriter,
    net::{TcpListener, TcpStream},
};
use protocol::{client::RequestKind, zerocopy::IntoBytes, Message};
use slab::Slab;

const N: usize = 1 << 20;

pub struct Journaler {}

pub async fn server(handle: DmaStreamWriter, listener: TcpListener) -> ! {
    let mut slab: Slab<BBBuffer<N>> = Slab::with_capacity(1024);

    loop {
        match listener.accept().await {
            Err(e) => eprintln!("error accepting cxn: {e}"),
            Ok(stream) => {
                // let buf: &BBBuffer<N> = slab.g;
                let handler = ConnectionHandler::new(stream, todo!()).unwrap();
                // let (tx, rx) = buf.try_split().unwrap();
                // buffers.insert(addr,)
                // monoio::spawn(handle_connection(stream, tx));
            }
        }
    }
}

struct ConnectionHandler {
    stream: TcpStream,
    buf: &'static BBBuffer<N>,
    tx: ManuallyDrop<Producer<'static, N>>,
    rx: ManuallyDrop<Consumer<'static, N>>,
    scratch: Vec<u8>,
}

struct RBuf {
    buf: &'static BBBuffer<N>,
    tx: ManuallyDrop<Producer<'static, N>>,
    rx: ManuallyDrop<Consumer<'static, N>>,
}

impl Drop for RBuf {
    fn drop(&mut self) {
        let _ = self
            .buf
            .try_release(unsafe { ManuallyDrop::take(&mut self.tx) }, unsafe {
                ManuallyDrop::take(&mut self.rx)
            });
    }
}

struct ConnectionState {}

impl ConnectionHandler {
    fn new(stream: TcpStream, buf: &'static BBBuffer<N>) -> Result<Self, bbqueue::Error> {
        let (tx, rx) = buf.try_split()?;
        Ok(Self {
            stream,
            buf,
            tx: ManuallyDrop::new(tx),
            rx: Some(ManuallyDrop::new(rx)),
            scratch: Vec::with_capacity(N),
        })
    }
}

impl Drop for ConnectionHandler {
    fn drop(&mut self) {
        let _ = self
            .buf
            .try_release(unsafe { ManuallyDrop::take(&mut self.tx) }, unsafe {
                ManuallyDrop::take(&mut self.rx)
            });
    }
}

async fn handle_connection(mut stream: TcpStream, mut tx: Producer<'static, N>) -> io::Result<()> {
    let mut scratch = Vec::<u8>::with_capacity(1 << 16);
    let mut result;

    loop {
        let mut grant = IoGrantW {
            grant: tx.grant_max_remaining(1 << 16).unwrap(),
            written: 0,
        };

        (result, grant) = PrefixedReadIo::new(&mut stream, scratch.as_slice())
            .read(grant)
            .await;
        match result {
            Err(e) => {
                eprintln!("error deserializing message: {e:?}");
            }
            Ok(n) => {
                let n_segment = find_boundary(&grant.grant.buf()[..n]);
                scratch.clear();
                scratch.extend_from_slice(&grant.grant.buf()[n_segment..n]);

                grant.grant.commit(n_segment);
            }
        }
    }
}

fn find_boundary(mut bytes: &[u8]) -> usize {
    let original = bytes;
    loop {
        let len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if len > bytes.len() {
            break;
        }
        bytes = &bytes[len..];
    }

    bytes.as_ptr() as usize - original.as_ptr() as usize
}
