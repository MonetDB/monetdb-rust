pub mod blockstate;
pub mod reading;
pub mod writing;

use std::net::TcpStream;

pub const BLOCKSIZE: usize = 8190;

pub struct Mapi {
    pub sock: TcpStream,
}
