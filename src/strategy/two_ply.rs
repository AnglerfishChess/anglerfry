//! The shallow negamax strategy.

use std::time::Instant;

use cozy_chess::{Board, Color, GameStatus, Move, Piece};

use super::{legal_moves, report};
use crate::search::Limits;

/// The deepest this strategy searches, whatever depth is asked of it.
const MAX_DEPTH: u8 = 4;

/// A score below every reachable one.
const WORST: i32 = -1_000_000;

/// Being checkmated, before the bonus that prefers being mated later.
const MATED: i32 = -100_000;

/// The centipawn value of a piece.
fn value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 0,
    }
}

/// The material balance of `board` in centipawns, from the side to move's point of view.
fn evaluate(board: &Board) -> i32 {
    let balance: i32 = Piece::ALL
        .into_iter()
        .map(|piece| {
            let squares = board.pieces(piece);
            let white = (squares & board.colors(Color::White)).len() as i32;
            let black = (squares & board.colors(Color::Black)).len() as i32;
            value(piece) * (white - black)
        })
        .sum();
    match board.side_to_move() {
        Color::White => balance,
        Color::Black => -balance,
    }
}

/// The score of `board` for the side to move, over `depth` plies, adding to `nodes` the positions
/// visited. Returns early, and then meaninglessly, once `limits` are spent.
fn negamax(board: &Board, depth: u8, limits: &Limits, nodes: &mut u64) -> i32 {
    *nodes += 1;
    match board.status() {
        GameStatus::Won => return MATED + i32::from(depth),
        GameStatus::Drawn => return 0,
        GameStatus::Ongoing => {}
    }
    if depth == 0 {
        return evaluate(board);
    }
    let mut best = WORST;
    board.generate_moves(|piece_moves| {
        for played in piece_moves {
            if limits.spent(*nodes) {
                return true;
            }
            let mut child = board.clone();
            child.play_unchecked(played);
            best = best.max(-negamax(&child, depth - 1, limits, nodes));
        }
        false
    });
    best
}

/// The move to play in `board`, searched as deeply as `limits` allow, or `None` when the game is
/// over there.
pub fn pick(board: &Board, limits: &Limits) -> Option<Move> {
    let started = Instant::now();
    let moves = legal_moves(board);
    let mut best = *moves.first()?;
    let max_depth = limits.depth.unwrap_or(MAX_DEPTH).clamp(1, MAX_DEPTH);
    let mut nodes = 0;
    for depth in 1..=max_depth {
        let mut candidate = None;
        let mut best_score = WORST;
        for played in &moves {
            if limits.spent(nodes) {
                break;
            }
            let mut child = board.clone();
            child.play_unchecked(*played);
            let score = -negamax(&child, depth - 1, limits, &mut nodes);
            if score > best_score {
                best_score = score;
                candidate = Some(*played);
            }
        }
        // An iteration cut short by the limits carries no usable score.
        if limits.spent(nodes) {
            break;
        }
        if let Some(candidate) = candidate {
            best = candidate;
            report(board, depth, best_score, nodes, started.elapsed(), best);
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use cozy_chess::util::display_uci_move;

    use super::*;
    use crate::uci::Go;

    /// The move picked in the position `fen`, searched `depth` plies.
    fn pick_in(fen: &str, depth: u8) -> Option<String> {
        let board = Board::from_fen(fen, false).unwrap();
        let limits = Limits::new(
            &Go {
                depth: Some(depth),
                ..Go::default()
            },
            board.side_to_move(),
        );
        let played = pick(&board, &limits)?;
        Some(display_uci_move(&board, played).to_string())
    }

    #[test]
    fn counts_material_for_the_side_to_move() {
        let board = Board::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1", false).unwrap();
        assert_eq!(evaluate(&board), -900);
    }

    #[test]
    fn takes_the_free_piece() {
        assert_eq!(
            pick_in("4k3/8/8/3q4/4B3/8/8/4K3 w - - 0 1", 2).as_deref(),
            Some("e4d5")
        );
    }

    #[test]
    fn finds_mate_in_one() {
        assert_eq!(
            pick_in(
                "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 1",
                2
            )
            .as_deref(),
            Some("h5f7")
        );
    }

    #[test]
    fn has_no_move_when_the_game_is_over() {
        assert_eq!(pick_in("7k/5KQ1/8/8/8/8/8/8 b - - 0 1", 2), None);
    }
}
