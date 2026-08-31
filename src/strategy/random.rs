//! The uniformly random strategy.

use std::time::Duration;

use cozy_chess::{Board, Move};
use rand::seq::IndexedRandom;

use super::{legal_moves, report};

/// A legal move in `board`, drawn uniformly, or `None` when the game is over there.
pub fn pick(board: &Board) -> Option<Move> {
    let chosen = legal_moves(board).choose(&mut rand::rng()).copied()?;
    report(board, 1, 0, 1, Duration::ZERO, chosen);
    Some(chosen)
}
