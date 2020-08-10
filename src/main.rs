use futures::{Stream, StreamExt};
use lazy_static::lazy_static;
use serde::Serialize;
use std::convert::Infallible;
use std::fmt::Write as _;
use std::io;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time;
use std::time::Duration;
use structopt::StructOpt;
use thiserror::Error;
use tokio;
use tokio::sync::watch;
use warp;
use warp::sse::ServerSentEvent;
use warp::Filter;

#[derive(Debug, Copy, Clone, Serialize)]
pub enum Player {
    Red,
    Blue,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(untagged)]
pub enum Occupancy {
    Occupied(Player),
    Free,
}

impl Occupancy {
    pub fn occupied(self) -> bool {
        use Occupancy::*;
        match self {
            Free => false,
            Occupied(_) => true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderData {
    width: usize,
    height: usize,
    data: Vec<Occupancy>,
}

impl RenderData {
    pub fn game_start() -> Self {
        let mut data = Vec::new();
        for i in 0..(BOARD_SIZE * BOARD_SIZE) {
            data.push(
                [
                    Occupancy::Occupied(Player::Red),
                    Occupancy::Occupied(Player::Blue),
                    Occupancy::Free,
                ][i % 3],
            )
        }
        Self {
            width: BOARD_SIZE,
            height: BOARD_SIZE,
            data,
        }
    }
}

fn receive_updates(
    recv: watch::Receiver<RenderData>,
) -> impl Stream<Item = Result<impl ServerSentEvent, Infallible>> {
    recv.map(|v| Ok((warp::sse::json(v), warp::sse::event("render"))))
}

fn start_webserver(recv: watch::Receiver<RenderData>, bind_addr: std::net::SocketAddr) {
    thread::spawn(move || {
        let mut rt = tokio::runtime::Builder::new()
            .basic_scheduler()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let index = warp::path!("index.html")
                .or(warp::path::end())
                .map(|_| warp::reply::html(include_str!("public/index.html")));
            let js = warp::path!("script.js").map(|| include_str!("public/script.js"));

            let clone_state_watcher = warp::any().map(move || recv.clone());
            let sse_watcher = warp::path("watch").and(clone_state_watcher).map(|recv| {
                let stream = receive_updates(recv);
                warp::sse::reply(warp::sse::keep_alive().stream(stream))
            });

            let routes = warp::get().and(index.or(js).or(sse_watcher));
            println!("Running visualizer on http://{}/", bind_addr);
            warp::serve(routes).run(bind_addr).await;
        });
    });
}

#[derive(Debug, Copy, Clone)]
pub enum WinState {
    Win,
    Loss,
    Tie,
}

impl WinState {
    pub fn inverse(self) -> Self {
        use WinState::*;
        match self {
            Win => Loss,
            Loss => Win,
            Tie => Tie,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ToClientMessage {
    End(WinState),
    Update(usize, usize),
}

#[derive(Debug, Copy, Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Error, Debug)]
pub enum ClientRecvFailure {
    #[error("client took too long to respond")]
    ClientTimeoutReached,
    #[error("the underlying socket errored")]
    ParseError,
    #[error("the client closed the connection")]
    Eof,
}

#[derive(Debug)]
struct Client {
    stream: io::BufReader<TcpStream>,
    name: String,
    read_line: String,
    write_buffer: String,
}

type ClientResult<T> = Result<Result<T, ClientRecvFailure>, io::Error>;
macro_rules! double_try {
    ($e:expr) => {
        let e = $e;
        match (e) {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => return Ok(Err(e)),
            Err(e) => return Err(e),
        }
    };
}

impl Client {
    pub fn new(stream: TcpStream) -> Result<Self, io::Error> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream: io::BufReader::new(stream),
            name: String::new(),
            read_line: String::new(),
            write_buffer: String::new(),
        })
    }

    pub fn recv_name(&mut self, deadline: time::Instant) -> ClientResult<()> {
        double_try!(self.read_line_deadline(deadline));
        dbg!(&self.read_line);
        self.name = self.read_line.trim().to_owned();
        Ok(Ok(()))
    }

    pub fn send_update(&mut self, upd: ToClientMessage) -> Result<(), io::Error> {
        self.write_buffer.clear();
        match upd {
            ToClientMessage::End(state) => {
                writeln!(&mut self.write_buffer, "{:?}", state).unwrap();
                self.write_buffer.make_ascii_uppercase();
            }
            ToClientMessage::Update(this, theirs) => {
                writeln!(&mut self.write_buffer, "{} {}", this, theirs).unwrap();
            }
        }
        // this could theoretically error with WoudBlock, if that ever happens we will deal with it
        self.stream
            .get_mut()
            .write_all(self.write_buffer.as_bytes())
    }

    pub fn read_direction(&mut self, deadline: std::time::Instant) -> ClientResult<Direction> {
        double_try!(self.read_line_deadline(deadline));
        dbg!(&self.read_line);
        if self.read_line.len() != 2 || !self.read_line.is_ascii() {
            return Ok(Err(ClientRecvFailure::ParseError));
        }
        Ok(Ok(match self.read_line.chars().next().unwrap() {
            'u' => Direction::Up,
            'd' => Direction::Down,
            'l' => Direction::Left,
            'r' => Direction::Right,
            _ => return Ok(Err(ClientRecvFailure::ParseError)),
        }))
    }

    fn read_line_deadline(&mut self, deadline: time::Instant) -> ClientResult<()> {
        self.read_line.clear();
        loop {
            match self.stream.read_line(&mut self.read_line) {
                Ok(0) => return Ok(Err(ClientRecvFailure::Eof)),
                Ok(_) => return Ok(Ok(())),
                Err(err) => match err.kind() {
                    io::ErrorKind::WouldBlock => (),
                    _ => return Err(err),
                },
            }
            if time::Instant::now() > deadline {
                return Ok(Err(ClientRecvFailure::ClientTimeoutReached));
            }
        }
    }
}

const BOARD_SIZE: usize = 32;

fn invert_pos(idx: usize) -> usize {
    BOARD_SIZE * BOARD_SIZE - idx - 1
}
fn invert_direction(d: Direction) -> Direction {
    use Direction::*;
    match d {
        Up => Down,
        Down => Up,
        Left => Right,
        Right => Left,
    }
}
fn invert_update(u: ToClientMessage) -> ToClientMessage {
    match u {
        ToClientMessage::End(x) => ToClientMessage::End(x.inverse()),
        ToClientMessage::Update(mypos, theirpos) => {
            ToClientMessage::Update(invert_pos(theirpos), invert_pos(mypos))
        }
    }
}
#[test]
fn inversions() {
    for i in 0..(BOARD_SIZE * BOARD_SIZE) {
        assert_eq!(i, invert_pos(invert_pos(i)));
    }
    assert_eq!(1023, invert_pos(0));
    assert_eq!(992, invert_pos(31));
    assert_eq!(34, invert_pos(989));
    assert_eq!(0, invert_pos(484));
}

#[derive(Debug, Copy, Clone)]
struct RedBlue<T> {
    pub red: T,
    pub blue: T,
}

impl<T> RedBlue<T> {
    pub fn map<U, F>(self, mut op: F) -> RedBlue<U>
    where
        F: FnMut(T) -> U,
    {
        RedBlue {
            red: op(self.red),
            blue: op(self.blue),
        }
    }

    pub fn as_ref(&self) -> RedBlue<&T> {
        RedBlue {
            red: &self.red,
            blue: &self.blue,
        }
    }
}

struct TronGame {
    board: Vec<Occupancy>,
    pos: RedBlue<usize>,
    endgame: Option<WinState>,
}

// Red is always the "main" player
impl TronGame {
    pub fn new() -> Self {
        let mut board = vec![Occupancy::Free; BOARD_SIZE * BOARD_SIZE];
        let redpos = 15 * 32 + 4;
        let bluepos = invert_pos(redpos);
        board[redpos] = Occupancy::Occupied(Player::Red);
        board[bluepos] = Occupancy::Occupied(Player::Blue);
        Self {
            pos: RedBlue {
                red: redpos,
                blue: bluepos,
            },
            endgame: None,
            board,
        }
    }

    // gives the message for red
    // takes moves that have already been inverted
    pub fn observe(&mut self, moves: RedBlue<Direction>) -> ToClientMessage {
        if let Some(win) = self.endgame {
            return ToClientMessage::End(win);
        }

        let red_boundary = Self::boundary_collision(self.pos.red, moves.red);
        let blue_boundary = Self::boundary_collision(self.pos.blue, moves.blue);

        self.pos.red = Self::advance(self.pos.red, moves.red);
        self.pos.blue = Self::advance(self.pos.blue, moves.blue);

        let mut red_collides = false;
        let mut blue_collides = false;

        // if we didnt check this, who won would depend on update order
        if self.pos.red == self.pos.blue {
            red_collides = true;
            blue_collides = true;
        }

        if !red_boundary {
            red_collides |= self.board[self.pos.red].occupied();
            self.board[self.pos.red] = Occupancy::Occupied(Player::Red);
        }
        if !blue_boundary {
            blue_collides |= self.board[self.pos.blue].occupied();
            self.board[self.pos.blue] = Occupancy::Occupied(Player::Blue);
        }

        self.endgame = match (red_collides || red_boundary, blue_collides || blue_boundary) {
            (true, true) => Some(WinState::Tie),
            (false, false) => None,
            (true, false) => Some(WinState::Loss),
            (false, true) => Some(WinState::Win),
        };
        if let Some(win) = self.endgame {
            ToClientMessage::End(win)
        } else {
            ToClientMessage::Update(self.pos.red, self.pos.blue)
        }
    }

    pub fn position_update(&self) -> ToClientMessage {
        ToClientMessage::Update(self.pos.red, self.pos.blue)
    }

    fn advance(pos: usize, d: Direction) -> usize {
        let pos = pos as isize;
        let bsize = BOARD_SIZE as isize;
        use Direction::*;
        (pos + match d {
            Up => -bsize,
            Down => bsize,
            Left => -1,
            Right => 1,
        }) as usize
    }

    fn boundary_collision(pos: usize, d: Direction) -> bool {
        use Direction::*;
        match d {
            Up => pos < BOARD_SIZE,
            Down => pos > BOARD_SIZE * BOARD_SIZE - BOARD_SIZE,
            Left => pos % BOARD_SIZE == 0,
            Right => pos % BOARD_SIZE == BOARD_SIZE - 1,
        }
    }

    pub fn render_data(&self) -> RenderData {
        RenderData {
            width: BOARD_SIZE,
            height: BOARD_SIZE,
            data: self.board.clone(),
        }
    }

    pub fn set_win_state(&mut self, w: WinState) {
        self.endgame = Some(w);
    }

    pub fn game_over(&self) -> bool {
        self.endgame.is_some()
    }
}

fn create_deadline() -> time::Instant {
    let timeout = time::Duration::from_millis(CLI_OPTIONS.timeout);
    time::Instant::now() + timeout
}

// Removes losing failures
fn handle_recv_failures<T>(
    errs: RedBlue<Result<T, ClientRecvFailure>>,
    game: &mut TronGame,
) -> Result<RedBlue<T>, ClientRecvFailure> {
    let losses = errs.as_ref().map(|res| res.is_err());
    match (losses.red, losses.blue) {
        (true, true) => game.set_win_state(WinState::Tie),
        (false, false) => (),
        (true, false) => game.set_win_state(WinState::Loss),
        (false, true) => game.set_win_state(WinState::Win),
    };
    match errs {
        RedBlue {
            red: Ok(red),
            blue: Ok(blue),
        } => Ok(RedBlue { red, blue }),
        RedBlue {
            red: Err(e),
            blue: _,
        } => Err(e),
        RedBlue {
            red: _,
            blue: Err(e),
        } => Err(e),
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "server")]
struct Opt {
    /// Game listen address and port number.
    #[structopt(name = "BIND_ADDRESS", default_value = "127.0.0.1:4040")]
    host: std::net::SocketAddr,

    /// Specifies the time limit for clients to respond to server messages,
    /// in milliseconds.
    #[structopt(long, default_value = "200")]
    timeout: u64,

    /// Add this many milliseconds of extra delay each game loop. Useful for
    /// slowing down the visualizer with fast bots.
    #[structopt(long, default_value = "0")]
    extra_delay: u64,

    /// Visualizer listen address and port number
    #[structopt(long, default_value = "127.0.0.1:3030")]
    visualizer_addr: std::net::SocketAddr,
}

lazy_static! {
    static ref CLI_OPTIONS: Opt = Opt::from_args();
}

fn main() -> Result<(), anyhow::Error> {
    let (render_send, render_recv) = watch::channel(RenderData::game_start());
    start_webserver(render_recv, CLI_OPTIONS.visualizer_addr);
    thread::sleep(Duration::from_millis(10));
    let bind_addr: std::net::SocketAddr = ([127, 0, 0, 1], 4040).into();
    println!("Listening for player connections on {}", bind_addr);
    let listener = TcpListener::bind(bind_addr)?;
    println!("Waiting for player 1");
    let (p1, _addr) = listener.accept()?;
    println!("Waiting for player 2");
    let (p2, _addr) = listener.accept()?;

    let red_player = Client::new(p1)?;
    let blue_player = Client::new(p2)?;

    let game = TronGame::new();

    play_game(red_player, blue_player, game, render_send)?;
    println!("Game ended normally");
    Ok(())
}

fn play_game(
    mut red_player: Client,
    mut blue_player: Client,
    mut game: TronGame,
    renderer: watch::Sender<RenderData>,
) -> Result<(), anyhow::Error> {
    println!("Reading names");
    // start by getting names
    let name_deadline = create_deadline();
    let res = handle_recv_failures(
        RedBlue {
            red: red_player.recv_name(name_deadline)?,
            blue: blue_player.recv_name(name_deadline)?,
        },
        &mut game,
    );
    if let Err(e) = res {
        // game ends due to client failure of some kind, just inform the clients of that
        let dummy_move = RedBlue {
            red: Direction::Up,
            blue: Direction::Up,
        };
        let msg = game.observe(dummy_move);
        red_player.send_update(msg)?;
        blue_player.send_update(invert_update(msg))?;
        println!("Game ended due to {:?} while getting names", e);
        return Ok(());
    }

    // initialize the game by sending initial positions
    let red_update = game.position_update();
    let blue_update = invert_update(red_update);
    red_player
        .send_update(red_update)
        .and(blue_player.send_update(blue_update))?;

    // init renderer
    renderer.broadcast(game.render_data())?;

    // main game loop
    while !game.game_over() {
        println!("Begin loop iter");
        // get client moves
        let move_deadline = create_deadline();
        let res = handle_recv_failures(
            RedBlue {
                red: red_player.read_direction(move_deadline)?,
                blue: blue_player.read_direction(move_deadline)?,
            },
            &mut game,
        );
        let moves = match res {
            Ok(mut rb) => {
                rb.blue = invert_direction(rb.blue);
                rb
            }
            Err(e) => {
                // game is already over, clients will be notified on the next
                // update. Give a dummy move to the already-ended game.
                println!("Game ended due to {:?} while getting moves", e);
                let dummy_move = RedBlue {
                    red: Direction::Up,
                    blue: Direction::Up,
                };
                dummy_move
            }
        };
        // update game state and send client
        let red_update = game.observe(moves);
        let blue_update = invert_update(red_update);
        red_player
            .send_update(red_update)
            .and(blue_player.send_update(blue_update))?;

        // update render state
        renderer.broadcast(game.render_data())?;

        // sleep if applicable
        if CLI_OPTIONS.extra_delay > 0 {
            std::thread::sleep(time::Duration::from_millis(CLI_OPTIONS.extra_delay));
        }
    }
    // finalize render state
    renderer.broadcast(game.render_data())?;
    // hacky but whatever
    std::thread::sleep(time::Duration::from_millis(10));
    Ok(())
}
