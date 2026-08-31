//! Anglerfry, a UCI chess engine.

mod search;
mod strategy;
mod uci;

fn main() {
    env_logger::init();
    uci::run();
}
