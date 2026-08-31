//! The uniformly random strategy.

use cozy_chess::{Board, Move};
use rand::seq::IndexedRandom;

use super::legal_moves;

/// A legal move in `board`, drawn uniformly, or `None` when the game is over there.
pub fn pick(board: &Board) -> Option<Move> {
    legal_moves(board).choose(&mut rand::rng()).copied()
}
