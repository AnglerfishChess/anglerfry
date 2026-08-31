//! Limits of a single `go`, and the thread that runs it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cozy_chess::util::display_uci_move;
use cozy_chess::{Board, Color, Move};
use log::error;

use crate::strategy::Strategy;
use crate::uci::{self, Go};

/// Moves the clock is expected to have to cover.
const MOVES_AHEAD: u32 = 30;

/// Longest a single move may take when a clock is given.
const MAX_BUDGET: Duration = Duration::from_secs(10);

/// Shortest a single move may take when a clock is given.
const MIN_BUDGET: Duration = Duration::from_millis(10);

/// Clock time held back for the moves that follow.
const RESERVE: Duration = Duration::from_millis(50);

/// How often a finished but withheld search rechecks its limits.
const POLL: Duration = Duration::from_millis(1);

/// When a search must give up, and how deep it may go before then.
pub struct Limits {
    stop: Arc<AtomicBool>,
    deadline: Option<Instant>,
    /// Plies to search; a strategy may cap this further.
    pub depth: Option<u8>,
    /// Positions to visit at most.
    pub nodes: Option<u64>,
    /// Whether the move must be withheld until `stop`.
    pub infinite: bool,
}

impl Limits {
    /// Turns the limits of a `go` into a deadline for a search by `turn`.
    pub fn new(go: &Go, turn: Color) -> Limits {
        let (remaining, increment) = match turn {
            Color::White => (go.white_time, go.white_increment),
            Color::Black => (go.black_time, go.black_increment),
        };
        let deadline = match (go.movetime, remaining) {
            (Some(movetime), _) => Some(Instant::now() + movetime),
            (None, Some(remaining)) => Some(
                Instant::now() + budget(remaining, increment.unwrap_or_default(), go.moves_to_go),
            ),
            (None, None) => None,
        };
        Limits {
            stop: Arc::new(AtomicBool::new(false)),
            deadline,
            depth: go.depth,
            nodes: go.nodes,
            // A `go` that names no bound at all also runs until stopped.
            infinite: go.infinite
                || (deadline.is_none() && go.depth.is_none() && go.nodes.is_none()),
        }
    }

    /// Whether the search must yield the best move it has.
    pub fn expired(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    /// Whether the search, having visited `nodes` positions, must yield the best move it has.
    pub fn spent(&self, nodes: u64) -> bool {
        self.nodes.is_some_and(|limit| nodes >= limit) || self.expired()
    }
}

/// Time to spend on one move, given the clock and the increment of the side to move.
fn budget(remaining: Duration, increment: Duration, moves_to_go: Option<u32>) -> Duration {
    let share = moves_to_go.unwrap_or(MOVES_AHEAD).clamp(1, MOVES_AHEAD);
    let planned = (remaining / share + increment / 2).min(MAX_BUDGET);
    planned
        .min(remaining.saturating_sub(RESERVE))
        .max(MIN_BUDGET)
}

/// A search in flight.
pub struct Handle {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Handle {
    /// Asks the search to report its best move now.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Asks the search to stop, and waits until it has reported.
    pub fn finish(self) {
        self.stop();
        if self.thread.join().is_err() {
            error!("Search thread panicked");
        }
    }
}

/// Starts a search that emits its `info` lines and exactly one `bestmove`.
pub fn spawn(strategy: Strategy, board: Board, limits: Limits) -> Handle {
    let stop = Arc::clone(&limits.stop);
    let thread = thread::spawn(move || {
        let best = strategy.pick(&board, &limits);
        while limits.infinite && !limits.expired() {
            thread::sleep(POLL);
        }
        uci::send(&best_move(&board, best));
    });
    Handle { stop, thread }
}

/// The `bestmove` line reporting `played` in `board`, or the null move when there is none.
fn best_move(board: &Board, played: Option<Move>) -> String {
    match played {
        Some(played) => format!("bestmove {}", display_uci_move(board, played)),
        None => "bestmove 0000".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use cozy_chess::util::parse_uci_move;

    use super::*;

    /// The limits of a `go` with `depth` plies asked for and nothing else.
    fn to_depth(depth: u8) -> Limits {
        Limits::new(
            &Go {
                depth: Some(depth),
                ..Go::default()
            },
            Color::White,
        )
    }

    #[test]
    fn spends_a_slice_of_the_clock() {
        assert_eq!(
            budget(Duration::from_secs(120), Duration::from_secs(2), None),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn spends_more_when_the_moves_ahead_are_counted() {
        assert_eq!(
            budget(Duration::from_secs(60), Duration::ZERO, Some(10)),
            Duration::from_secs(6)
        );
    }

    #[test]
    fn caps_the_time_spent_on_one_move() {
        assert_eq!(
            budget(Duration::from_secs(3600), Duration::ZERO, None),
            MAX_BUDGET
        );
    }

    #[test]
    fn stays_within_the_clock() {
        assert_eq!(
            budget(Duration::from_millis(60), Duration::from_secs(10), None),
            MIN_BUDGET
        );
    }

    #[test]
    fn runs_until_stopped_only_without_any_bound() {
        assert!(Limits::new(&Go::default(), Color::White).infinite);
        assert!(
            Limits::new(
                &Go {
                    infinite: true,
                    ..Go::default()
                },
                Color::White
            )
            .infinite
        );
        assert!(!to_depth(3).infinite);
    }

    #[test]
    fn expires_at_the_deadline() {
        let limits = Limits::new(
            &Go {
                movetime: Some(Duration::ZERO),
                ..Go::default()
            },
            Color::White,
        );
        assert!(limits.expired());
    }

    #[test]
    fn expires_when_stopped() {
        let limits = to_depth(3);
        assert!(!limits.expired());
        limits.stop.store(true, Ordering::Relaxed);
        assert!(limits.expired());
    }

    #[test]
    fn reports_castling_the_way_guis_write_it() {
        let board = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1", false).unwrap();
        let castle = parse_uci_move(&board, "e1g1").unwrap();
        assert_eq!(best_move(&board, Some(castle)), "bestmove e1g1");
        assert_eq!(best_move(&board, None), "bestmove 0000");
    }

    #[test]
    fn expires_when_the_nodes_run_out() {
        let limits = Limits::new(
            &Go {
                nodes: Some(100),
                ..Go::default()
            },
            Color::White,
        );
        assert!(!limits.spent(99));
        assert!(limits.spent(100));
    }
}
