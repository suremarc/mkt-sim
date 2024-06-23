use std::{
    cell::RefCell,
    io::{self},
    net::SocketAddr,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use bbqueue::{BBBuffer, GrantR, GrantW, Producer};

use monoio::{
    buf::{IoBuf, IoBufMut},
    fs::File,
    io::{AsyncReadRent, AsyncWriteRentExt},
    net::{TcpListener, TcpStream},
    IoUringDriver,
};
use protocol::{
    client::{ErrorKind, RequestKind},
    zerocopy::{IntoBytes, TryFromBytes},
    Message,
};

pub struct Journaler {}

const N: usize = 1 << 20;
static BB: BBBuffer<N> = BBBuffer::new();

pub async fn server(handle: File, addr: SocketAddr) -> ! {
    let (tx, mut rx) = BB.try_split().unwrap();

    std::thread::spawn(move || monoio::start::<IoUringDriver, _>(accept_loop(addr, tx)));

    loop {
        // todo: async backoff
        let grant = match rx.read() {
            Err(bbqueue::Error::InsufficientSize) => continue,
            Err(e) => panic!("{e:?}"),
            Ok(grant) => grant,
        };

        if let Err(e) = handle.write_all_at(IoGrantR(grant), 0).await.0 {
            eprintln!("{e}");
        }
    }
}

struct IoGrantR(GrantR<'static, N>);

unsafe impl IoBuf for IoGrantR {
    fn read_ptr(&self) -> *const u8 {
        self.0.buf().as_ptr()
    }

    fn bytes_init(&self) -> usize {
        self.0.buf().len()
    }
}

async fn accept_loop(addr: SocketAddr, tx: Producer<'static, N>) -> ! {
    let tx = Rc::new(RefCell::new(tx));
    let listener = TcpListener::bind(addr).unwrap();
    loop {
        match listener.accept().await {
            Err(e) => eprintln!("error accepting cxn: {e}"),
            Ok((stream, _addr)) => {
                monoio::spawn(handle_connection(stream, tx.clone()));
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    tx: Rc<RefCell<Producer<'static, N>>>,
) -> io::Result<()> {
    let mut scratch = Vec::<u8>::with_capacity(1 << 16);
    let mut write_buf = Vec::<u8>::with_capacity(1 << 16);

    let mut seq = 0;

    let mut result;

    loop {
        // todo: async backoff
        let mut grant = tx
            .as_ref()
            .borrow_mut()
            .grant_max_remaining(1 << 16)
            .unwrap();

        grant.buf().copy_from_slice(&scratch);
        let n_scratch = scratch.len();
        scratch.clear();

        let mut grant = IoGrantW {
            grant,
            written: n_scratch,
        };

        (result, grant) = stream.read(grant).await;
        match result {
            Err(e) => {
                eprintln!("error deserializing message: {e:?}");
            }
            Ok(n) => {
                let n_segment = validate(&grant.buf()[..n], &mut write_buf, &mut seq);
                scratch.extend_from_slice(&grant.buf()[n_segment..n]);

                grant.commit(n_segment);
            }
        }

        // todo: handle reading in another task
        (result, write_buf) = stream.write_all(write_buf).await;
        result.unwrap();
    }
}

fn validate(mut rem: &[u8], write_buf: &mut Vec<u8>, seq: &mut u32) -> usize {
    let original = rem;
    loop {
        let len = u16::from_be_bytes([rem[0], rem[1]]) as usize;
        if len > rem.len() {
            break;
        }

        let msg_bytes;
        (msg_bytes, rem) = rem.split_at(len);

        match Message::<RequestKind>::try_ref_from(msg_bytes) {
            Err(_e) => {
                write_buf.extend_from_slice(Message::new(*seq, ErrorKind::Invalid).as_bytes())
            }
            Ok(msg) => {
                if msg.sequence_number.get() != *seq {
                    write_buf.extend_from_slice(
                        Message::new(*seq, ErrorKind::UnexpectedSequenceNumber).as_bytes(),
                    )
                }
            }
        };

        *seq += 1;
    }

    rem.as_ptr() as usize - original.as_ptr() as usize
}

struct IoGrantW {
    grant: GrantW<'static, N>,
    written: usize,
}

impl IoGrantW {
    fn commit(self, used: usize) {
        self.grant.commit(used)
    }
}

unsafe impl IoBufMut for IoGrantW {
    fn write_ptr(&mut self) -> *mut u8 {
        self.grant.buf().as_mut_ptr()
    }

    fn bytes_total(&mut self) -> usize {
        self.grant.buf().len()
    }

    unsafe fn set_init(&mut self, pos: usize) {
        self.written = pos;
    }
}

impl Deref for IoGrantW {
    type Target = GrantW<'static, N>;

    fn deref(&self) -> &Self::Target {
        &self.grant
    }
}

impl DerefMut for IoGrantW {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.grant
    }
}
