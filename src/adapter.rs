use anyhow;
use std::fmt::Write as _;
use std::io::{self, BufRead as _, Write as _};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::mpsc::*;
use std::thread;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "client-adapter")]
struct Opt {
    /// Game host address and port number
    #[structopt(name = "HOST", default_value = "127.0.0.1:4040")]
    host: SocketAddr,
}

// Forwards this processes STDIN over TCP to a server.
// Forwards TCP traffic from the server to STDOUT.
// Line-buffers both.
fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::from_args();
    eprintln!("Adapter Connecting...");
    let stream = TcpStream::connect(opt.host)?;
    stream.set_nonblocking(true)?;

    let (stdin, tcpout) = channel();

    let stdin_listener_handle = thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            stdin.send(line.unwrap()).unwrap();
        }
    });

    // line buffer TCP
    let mut stream = io::BufReader::new(stream);
    let mut write_buffer = String::new();
    let mut read_buffer = String::new();
    let stdout_handle = io::stdout();
    let mut stdout = stdout_handle.lock();
    loop {
        // print from stdin if we have it.
        match tcpout.try_recv() {
            Ok(val) => {
                // have to manually append newline byte
                write_buffer.clear();
                writeln!(&mut write_buffer, "{}", val)?;
                stream.get_mut().write(write_buffer.as_bytes()).unwrap();
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                // Reader finished and dropped channel, meaning stdin was
                // closed, meaning the client finished.
                // Verify nothing went wrong:
                stdin_listener_handle.join().unwrap();
                return Ok(());
            }
        };
        read_buffer.clear();
        match stream.read_line(&mut read_buffer) {
            Ok(0) => {
                eprintln!("TCP connection closed");
                return Ok(());
            }
            Ok(_) => {
                stdout.write(read_buffer.as_bytes())?;
            }
            Err(err) => match err.kind() {
                io::ErrorKind::WouldBlock => (),
                _ => return Err(err.into()),
            },
        }
    }
}
