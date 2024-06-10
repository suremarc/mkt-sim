use std::{
    fs::File,
    io,
    net::{SocketAddr, TcpListener, TcpStream},
};

pub struct JournalerOptions {
    pub addr: SocketAddr,
}

pub fn serve(_handle: File, opts: JournalerOptions) -> io::Result<()> {
    let listener = TcpListener::bind(opts.addr)?;
    for result in listener.incoming() {
        match result {
            Ok(stream) => handle_connection(stream)?,
            Err(e) => return Err(e),
        }
    }

    unreachable!()
}

fn handle_connection(_stream: TcpStream) -> io::Result<()> {
    todo!()
}
