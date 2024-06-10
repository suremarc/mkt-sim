use std::{convert::Infallible, fs::File, io, net::SocketAddr};

use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token,
};
use slab::Slab;

pub struct JournalerOptions {
    pub addr: SocketAddr,
}

pub struct Journaler {}

pub fn server(_handle: File, mut listener: TcpListener) -> io::Result<impl FnMut() -> Infallible> {
    let LISTENER: Token = Token(usize::MAX);

    let mut poll = Poll::new()?;
    poll.registry()
        .register(&mut listener, LISTENER, Interest::READABLE)?;

    let mut cxns: Slab<TcpStream> = Default::default();

    let mut events = Events::with_capacity(1024);

    Ok(move || loop {
        match poll.poll(&mut events, None) {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    })
}

fn handle_connection(_stream: TcpStream) -> io::Result<()> {
    todo!()
}
