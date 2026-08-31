//! Reading UCI command lines.

use std::time::Duration;

use cozy_chess::Board;
use cozy_chess::util::parse_uci_move;
use log::warn;

/// A command from the GUI.
#[derive(Debug, PartialEq)]
pub enum Command {
    Uci,
    IsReady,
    SetOption {
        name: String,
        value: Option<String>,
    },
    NewGame,
    /// The position to search, with the moves already played onto it.
    Position(Board),
    Go(Go),
    Stop,
    Quit,
    /// A command that is understood and calls for nothing: `debug`, `ponderhit`, `register`.
    Nothing,
}

/// The limits named by a `go`.
#[derive(Debug, Default, PartialEq)]
pub struct Go {
    pub movetime: Option<Duration>,
    pub white_time: Option<Duration>,
    pub black_time: Option<Duration>,
    pub white_increment: Option<Duration>,
    pub black_increment: Option<Duration>,
    pub moves_to_go: Option<u32>,
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    /// Moves to mate in, as `mate` asks.
    pub mate: Option<u8>,
    /// The moves the answer must come from, in UCI notation; empty allows every legal move.
    pub search_moves: Vec<String>,
    /// Whether the search runs until stopped, as `infinite` and `ponder` ask.
    pub infinite: bool,
}

/// Reads a command line, dropping unrecognised leading tokens as the UCI specification requires.
pub fn parse(line: &str) -> Option<Command> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for (index, keyword) in tokens.iter().enumerate() {
        let rest = &tokens[index + 1..];
        let command = match *keyword {
            "uci" => Some(Command::Uci),
            "isready" => Some(Command::IsReady),
            "ucinewgame" => Some(Command::NewGame),
            "stop" => Some(Command::Stop),
            "quit" => Some(Command::Quit),
            "debug" | "ponderhit" | "register" => Some(Command::Nothing),
            "setoption" => set_option(rest),
            "position" => position(rest),
            "go" => Some(Command::Go(go(rest))),
            _ => None,
        };
        if command.is_some() {
            return command;
        }
    }
    None
}

/// Reads the tail of `setoption name <name> [value <value>]`.
fn set_option(tokens: &[&str]) -> Option<Command> {
    let (keyword, rest) = tokens.split_first()?;
    if *keyword != "name" {
        return None;
    }
    let (name, value) = match rest.iter().position(|token| *token == "value") {
        Some(at) => (&rest[..at], Some(rest[at + 1..].join(" "))),
        None => (rest, None),
    };
    if name.is_empty() {
        return None;
    }
    Some(Command::SetOption {
        name: name.join(" "),
        value,
    })
}

/// Reads the tail of `position startpos|fen <6 fields> [moves <move>...]`.
fn position(tokens: &[&str]) -> Option<Command> {
    let (keyword, rest) = tokens.split_first()?;
    let (mut board, rest) = match *keyword {
        "startpos" => (Board::startpos(), rest),
        "fen" => {
            let fen = rest.get(..6)?.join(" ");
            match Board::from_fen(&fen, false) {
                Ok(board) => (board, &rest[6..]),
                Err(error) => {
                    warn!("Ignoring position {fen:?}: {error}");
                    return None;
                }
            }
        }
        _ => return None,
    };
    let moves = match rest.split_first() {
        Some((keyword, moves)) if *keyword == "moves" => moves,
        Some(_) => return None,
        None => &[],
    };
    for text in moves {
        let played = parse_uci_move(&board, text).ok();
        if played.is_none_or(|played| board.try_play(played).is_err()) {
            warn!("Ignoring position: {text} is unplayable");
            return None;
        }
    }
    Some(Command::Position(board))
}

/// The keywords a `go` may name, each ending the argument list of the one before.
const GO_KEYWORDS: [&str; 12] = [
    "searchmoves",
    "ponder",
    "wtime",
    "btime",
    "winc",
    "binc",
    "movestogo",
    "depth",
    "nodes",
    "mate",
    "movetime",
    "infinite",
];

/// Reads the tail of a `go`, keeping the limits it names.
fn go(tokens: &[&str]) -> Go {
    let mut go = Go::default();
    for (index, keyword) in tokens.iter().enumerate() {
        let rest = &tokens[index + 1..];
        let argument = rest.first().copied().unwrap_or_default();
        match *keyword {
            "movetime" => go.movetime = millis(argument),
            "wtime" => go.white_time = millis(argument),
            "btime" => go.black_time = millis(argument),
            "winc" => go.white_increment = millis(argument),
            "binc" => go.black_increment = millis(argument),
            "movestogo" => go.moves_to_go = argument.parse().ok(),
            "depth" => go.depth = argument.parse().ok(),
            "nodes" => go.nodes = argument.parse().ok(),
            "mate" => go.mate = argument.parse().ok(),
            "searchmoves" => go.search_moves = search_moves(rest),
            "infinite" | "ponder" => go.infinite = true,
            _ => {}
        }
    }
    go
}

/// Reads the moves of a `searchmoves`, which run until the next keyword of the `go`.
fn search_moves(tokens: &[&str]) -> Vec<String> {
    tokens
        .iter()
        .take_while(|token| !GO_KEYWORDS.contains(*token))
        .map(|token| (*token).to_owned())
        .collect()
}

/// Reads a count of milliseconds.
fn millis(argument: &str) -> Option<Duration> {
    argument.parse().ok().map(Duration::from_millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The board of a `position` command line.
    fn board_of(line: &str) -> Option<Board> {
        match parse(line) {
            Some(Command::Position(board)) => Some(board),
            _ => None,
        }
    }

    /// The limits of a `go` command line.
    fn go_of(line: &str) -> Go {
        match parse(line) {
            Some(Command::Go(go)) => go,
            other => panic!("expected a go, got {other:?}"),
        }
    }

    #[test]
    fn reads_a_plain_command() {
        assert_eq!(parse("  isready \n"), Some(Command::IsReady));
    }

    #[test]
    fn drops_unrecognised_leading_tokens() {
        assert_eq!(parse("joho debug on"), Some(Command::Nothing));
    }

    #[test]
    fn rejects_a_line_without_a_command() {
        assert_eq!(parse("what is this"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn reads_an_option_name_and_value_of_several_words() {
        assert_eq!(
            parse("setoption name Clear Hash value two words"),
            Some(Command::SetOption {
                name: "Clear Hash".to_owned(),
                value: Some("two words".to_owned()),
            })
        );
        assert_eq!(
            parse("setoption name Clear Hash"),
            Some(Command::SetOption {
                name: "Clear Hash".to_owned(),
                value: None,
            })
        );
        assert_eq!(parse("setoption value 1"), None);
    }

    #[test]
    fn plays_the_moves_onto_the_position() {
        let board = board_of("position startpos moves e2e4 e7e5").unwrap();
        assert_eq!(
            format!("{board}"),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
        );
    }

    #[test]
    fn reads_castling_in_the_notation_guis_use() {
        let board = board_of("position fen 4k3/8/8/8/8/8/8/4K2R w K - 0 1 moves e1g1").unwrap();
        assert_eq!(format!("{board}"), "4k3/8/8/8/8/8/8/5RK1 b - - 1 1");
    }

    #[test]
    fn rejects_a_position_it_cannot_use() {
        assert_eq!(board_of("position fen 8/8/8/8/8/8/8/8 w - - 0 1"), None);
        assert_eq!(board_of("position fen rubbish"), None);
        assert_eq!(board_of("position startpos moves e2e5"), None);
        assert_eq!(board_of("position elsewhere"), None);
    }

    #[test]
    fn reads_the_clock_of_a_go() {
        assert_eq!(
            go_of("go wtime 300000 btime 299000 winc 2000 binc 2000 movestogo 40"),
            Go {
                white_time: Some(Duration::from_secs(300)),
                black_time: Some(Duration::from_secs(299)),
                white_increment: Some(Duration::from_secs(2)),
                black_increment: Some(Duration::from_secs(2)),
                moves_to_go: Some(40),
                ..Go::default()
            }
        );
    }

    #[test]
    fn reads_the_other_limits_of_a_go() {
        assert_eq!(
            go_of("go movetime 500").movetime,
            Some(Duration::from_millis(500))
        );
        assert_eq!(go_of("go depth 7 nodes 900").depth, Some(7));
        assert_eq!(go_of("go depth 7 nodes 900").nodes, Some(900));
        assert_eq!(go_of("go mate 2").mate, Some(2));
        assert!(go_of("go infinite").infinite);
        assert!(go_of("go ponder").infinite);
        assert_eq!(go_of("go"), Go::default());
    }

    #[test]
    fn reads_the_moves_a_go_restricts_itself_to() {
        assert_eq!(
            go_of("go searchmoves e2e4 d2d4 depth 3"),
            Go {
                search_moves: vec!["e2e4".to_owned(), "d2d4".to_owned()],
                depth: Some(3),
                ..Go::default()
            }
        );
        assert!(go_of("go searchmoves").search_moves.is_empty());
    }

    #[test]
    fn keeps_the_limits_it_understands_out_of_a_go() {
        assert_eq!(go_of("go movetime nonsense mate nonsense"), Go::default());
    }
}
