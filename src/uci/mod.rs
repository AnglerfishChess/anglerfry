//! The UCI protocol loop.
//!
//! This loop never blocks on a search: `go` hands the position to a search thread and comes
//! straight back to stdin, so `isready`, `stop` and `quit` stay answerable while thinking.

mod parse;

use std::io::{self, BufRead};
use std::ops::ControlFlow;

use cozy_chess::Board;
use log::debug;

pub use parse::Go;
use parse::{Command, parse};

use crate::search::{self, Limits};
use crate::strategy::Strategy;

/// The `id name` of this engine.
const NAME: &str = concat!("Anglerfry ", env!("CARGO_PKG_VERSION"));

/// The `id author` of this engine.
const AUTHOR: &str = "Alexander Myodov";

/// Writes one GUI-bound message as a single line on stdout.
pub fn send(message: &str) {
    println!("{message}");
}

/// Serves the UCI protocol on stdin/stdout until `quit` or end of input.
pub fn run() {
    let mut session = Session::new();
    for line in io::stdin().lock().lines().map_while(Result::ok) {
        match parse(&line) {
            Some(command) => {
                if session.handle(command).is_break() {
                    break;
                }
            }
            None => debug!("Unrecognised command: {line:?}"),
        }
    }
    session.abandon_search();
}

/// The state a UCI session carries between commands.
struct Session {
    board: Board,
    strategy: Strategy,
    search: Option<search::Handle>,
}

impl Session {
    /// A session at the initial position, with no search in flight.
    fn new() -> Session {
        Session {
            board: Board::startpos(),
            strategy: Strategy::default(),
            search: None,
        }
    }

    /// Acts on one command, breaking once the session is over.
    fn handle(&mut self, command: Command) -> ControlFlow<()> {
        match command {
            Command::Uci => {
                send(&format!("id name {NAME}"));
                send(&format!("id author {AUTHOR}"));
                send(&Strategy::option());
                send("uciok");
            }
            Command::IsReady => send("readyok"),
            Command::SetOption { name, value } => self.set_option(&name, value.as_deref()),
            Command::NewGame => {
                self.finish_search();
                self.board = Board::startpos();
            }
            Command::Position(board) => self.board = board,
            Command::Go(go) => self.go(&go),
            Command::Stop => {
                if let Some(handle) = &self.search {
                    handle.stop();
                }
            }
            Command::Quit => {
                self.abandon_search();
                return ControlFlow::Break(());
            }
            Command::Nothing => {}
        }
        ControlFlow::Continue(())
    }

    /// Applies a `setoption`; unknown names and values are dropped.
    fn set_option(&mut self, name: &str, value: Option<&str>) {
        if !name.eq_ignore_ascii_case(Strategy::OPTION) {
            debug!("Ignoring unknown option {name:?}");
        } else if let Some(strategy) = value.and_then(Strategy::from_name) {
            self.strategy = strategy;
        } else {
            debug!("Ignoring unknown {} value {value:?}", Strategy::OPTION);
        }
    }

    /// Starts a search for the current position, after any search in flight has reported.
    fn go(&mut self, go: &Go) {
        self.finish_search();
        let limits = Limits::new(go, self.board.side_to_move());
        self.search = Some(search::spawn(self.strategy, self.board.clone(), limits));
    }

    /// Stops the search in flight and waits for its `bestmove`.
    fn finish_search(&mut self) {
        if let Some(handle) = self.search.take() {
            handle.finish();
        }
    }

    /// Stops the search in flight without waiting for it.
    fn abandon_search(&mut self) {
        if let Some(handle) = self.search.take() {
            handle.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_a_strategy_by_option() {
        let mut session = Session::new();
        session.set_option("Strategy", Some("two-ply"));
        assert_eq!(session.strategy, Strategy::TwoPly);

        session.set_option("Strategy", Some("nonsense"));
        assert_eq!(session.strategy, Strategy::TwoPly);

        session.set_option("Nonsense", Some("random"));
        assert_eq!(session.strategy, Strategy::TwoPly);
    }

    #[test]
    fn takes_the_position_from_the_gui() {
        let mut session = Session::new();
        let Some(command) = parse("position startpos moves d2d4") else {
            panic!("expected a position");
        };
        session.handle(command);
        assert_ne!(session.board, Board::startpos());

        session.handle(Command::NewGame);
        assert_eq!(session.board, Board::startpos());
    }
}
