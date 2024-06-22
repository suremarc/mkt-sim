use std::{collections::BTreeMap, io, mem::ManuallyDrop};

use bbqueue::{BBBuffer, Consumer, GrantW, Producer};
use protocol::{client::RequestKind, zerocopy::IntoBytes, Message};
use slab::Slab;
use tokio_uring::{
    buf::{IoBuf, IoBufMut, Slice},
    fs::File,
    io::{AsyncReadRent, BufReader, PrefixedReadIo},
    net::{unix::SocketAddr, TcpListener, TcpStream},
};

const N: usize = 1 << 20;

pub struct Journaler {}

pub async fn server(handle: File, listener: TcpListener) -> ! {
    let mut buffers: BTreeMap<SocketAddr, ConnectionHandler> = Default::default();

    let mut slab: Slab<BBBuffer<N>> = Slab::with_capacity(1024);

    loop {
        match listener.accept().await {
            Err(e) => eprintln!("error accepting cxn: {e}"),
            Ok((stream, addr)) => {
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

struct IoGrantW<'a> {
    grant: GrantW<'a, N>,
    written: usize,
}

unsafe impl IoBuf for IoGrantW {
    fn stable_ptr(&mut self) -> *mut u8 {
        self.grant.buf().as_mut_ptr()
    }

    fn bytes_init(&self) -> usize {
        self.grant.buf().len()
    }

    fn bytes_total(&mut self) -> usize {
        self.grant.buf().len()
    }
}

unsafe impl IoBufMut for IoGrantW {
    fn stable_mut_ptr(&mut self) -> *mut u8 {
        self.grant.buf().as_mut_ptr()
    }

    unsafe fn set_init(&mut self, pos: usize) {
        self.written = pos;
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
