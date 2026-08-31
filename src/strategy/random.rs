//! The uniformly random strategy.

use std::time::Duration;

use cozy_chess::{Board, Move};
use rand::seq::IndexedRandom;

use super::{report, root_moves};
use crate::search::Limits;

/// A move `limits` allow in `board`, drawn uniformly, or `None` when there is none.
pub fn pick(board: &Board, limits: &Limits) -> Option<Move> {
    let chosen = root_moves(board, limits)
        .choose(&mut rand::rng())
        .copied()?;
    report(board, 1, 0, 1, Duration::ZERO, chosen);
    Some(chosen)
}
