//! The ways Anglerfry picks a move, and the UCI option that selects one.

mod random;
mod two_ply;

use std::time::Duration;

use cozy_chess::util::display_uci_move;
use cozy_chess::{Board, Move};

use crate::search::Limits;
use crate::uci;

/// A way of picking a move.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Strategy {
    /// Uniformly random among the legal moves.
    #[default]
    Random,
    /// Shallow negamax over material.
    TwoPly,
}

impl Strategy {
    /// The name of the UCI option selecting a strategy.
    pub const OPTION: &'static str = "Strategy";

    /// Every strategy, in the order the option offers them.
    const ALL: [Strategy; 2] = [Strategy::Random, Strategy::TwoPly];

    /// The option value naming this strategy.
    pub fn name(self) -> &'static str {
        match self {
            Strategy::Random => "random",
            Strategy::TwoPly => "two-ply",
        }
    }

    /// The strategy that `name` selects, if any.
    pub fn from_name(name: &str) -> Option<Strategy> {
        Strategy::ALL
            .into_iter()
            .find(|strategy| strategy.name().eq_ignore_ascii_case(name))
    }

    /// The `option` line offering the choice of strategy.
    pub fn option() -> String {
        let mut line = format!(
            "option name {} type combo default {}",
            Strategy::OPTION,
            Strategy::default().name()
        );
        for strategy in Strategy::ALL {
            line.push_str(" var ");
            line.push_str(strategy.name());
        }
        line
    }

    /// The move to play in `board` within `limits`, or `None` when the game is over there.
    /// Emits at least one `info` line whenever it returns a move.
    pub fn pick(self, board: &Board, limits: &Limits) -> Option<Move> {
        match self {
            Strategy::Random => random::pick(board, limits),
            Strategy::TwoPly => two_ply::pick(board, limits),
        }
    }
}

/// The moves in `board` a search may answer with: those `limits` name, else every legal one.
fn root_moves(board: &Board, limits: &Limits) -> Vec<Move> {
    if !limits.search_moves.is_empty() {
        return limits.search_moves.clone();
    }
    let mut moves = Vec::new();
    board.generate_moves(|piece_moves| {
        moves.extend(piece_moves);
        false
    });
    moves
}

/// Emits the `info` line of a search that settled on `best`, scored in centipawns.
fn report(board: &Board, depth: u8, score: i32, nodes: u64, elapsed: Duration, best: Move) {
    uci::send(&format!(
        "info depth {depth} time {} nodes {nodes} score cp {score} pv {}",
        elapsed.as_millis(),
        display_uci_move(board, best)
    ));
}

#[cfg(test)]
mod tests {
    use cozy_chess::GameStatus;

    use super::*;
    use crate::uci::Go;

    #[test]
    fn names_round_trip() {
        for strategy in Strategy::ALL {
            assert_eq!(Strategy::from_name(strategy.name()), Some(strategy));
        }
        assert_eq!(Strategy::from_name("nonsense"), None);
    }

    #[test]
    fn offers_every_strategy() {
        assert_eq!(
            Strategy::option(),
            "option name Strategy type combo default random var random var two-ply"
        );
    }

    /// Picks with every strategy, checking that the answer is one of the named moves.
    #[test]
    fn every_strategy_answers_within_searchmoves() {
        let board = Board::startpos();
        let limits = Limits::new(
            &Go {
                depth: Some(2),
                search_moves: ["a2a3", "h2h3"].map(str::to_owned).into(),
                ..Go::default()
            },
            &board,
        );
        for strategy in Strategy::ALL {
            let played = strategy.pick(&board, &limits).expect("a legal move");
            let played = display_uci_move(&board, played).to_string();
            assert!(["a2a3", "h2h3"].contains(&played.as_str()), "{played}");
        }
    }

    /// Plays both strategies against each other, checking that every move they pick is legal.
    #[test]
    fn self_play_stays_legal() {
        let mut board = Board::startpos();
        let limits = Limits::new(
            &Go {
                depth: Some(2),
                ..Go::default()
            },
            &board,
        );
        for ply in 0..40 {
            if board.status() != GameStatus::Ongoing {
                break;
            }
            let strategy = if ply % 2 == 0 {
                Strategy::Random
            } else {
                Strategy::TwoPly
            };
            let played = strategy.pick(&board, &limits).expect("a legal move");
            assert!(board.is_legal(played));
            board.play_unchecked(played);
        }
    }
}
