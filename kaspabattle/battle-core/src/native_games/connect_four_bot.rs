//! Connect Four bot. Pure domain logic: it only *proposes* a column; the caller applies it through
//! the same [`ConnectFour::execute`] every human move goes through, so the bot can never play an
//! illegal move or act out of turn.
//!
//! * `Easy`   – takes an immediate win, blocks an immediate loss, avoids handing the opponent a win,
//!              otherwise picks a (seeded) random column, mildly preferring the centre.
//! * `Medium` – iterative-deepening negamax with alpha-beta pruning (depth ≤ 5) + positional heuristic.
//! * `Hard`   – same search, depth ≤ 8.
//!
//! Determinism: for a fixed `seed` and a search that finishes inside `budget`, the result is always
//! the same. The time budget only ever *stops deepening*, it never returns a half-searched depth.

use super::connect_four::{ConnectFour, GameStatus, COLUMNS, ROWS};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "easy" => Some(Difficulty::Easy),
            "medium" => Some(Difficulty::Medium),
            "hard" => Some(Difficulty::Hard),
            _ => None,
        }
    }

    fn max_depth(self) -> u32 {
        match self {
            Difficulty::Easy => 0,
            Difficulty::Medium => 5,
            Difficulty::Hard => 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BotParams {
    pub difficulty: Difficulty,
    /// Seeds all randomness (tie-breaks, Easy-mode choices).
    pub seed: u64,
    /// Wall-clock budget for the whole decision.
    pub budget: Duration,
}

const WIN: i32 = 1_000_000;
/// Centre-first column order: better pruning and a sensible tie-break.
const ORDER: [usize; COLUMNS] = [3, 2, 4, 1, 5, 0, 6];

/// Legal columns (not full), centre first.
fn legal_columns(game: &ConnectFour) -> Vec<usize> {
    ORDER
        .iter()
        .copied()
        .filter(|c| game.cell(ROWS - 1, *c) == 0)
        .collect()
}

/// Would playing `column` as `who` win immediately? (simulates and rolls back)
fn wins_now(game: &mut ConnectFour, who: &str, column: usize) -> bool {
    match game.execute(who, column) {
        Ok(m) => {
            let won = matches!(m.status, GameStatus::Won { .. });
            // `who` may not be on move in the real game; execute() enforces turns, so callers pass
            // the side to move. Always undo.
            let _ = game.rollback(&m.rollback);
            won
        }
        Err(_) => false,
    }
}

/// Picks a column for `bot` (its player id inside `game`). `None` if the game is over, it is not the
/// bot's turn or there is no legal move.
pub fn choose_column(game: &ConnectFour, bot: &str, params: BotParams) -> Option<usize> {
    if game.current_player() != Some(bot) {
        return None;
    }
    let legal = legal_columns(game);
    if legal.is_empty() {
        return None;
    }
    let started = Instant::now();
    let mut rng = StdRng::seed_from_u64(params.seed);
    let mut sim = game.clone();
    let slot = sim.slot_of(bot)?;
    let opp = sim.player_at(slot.other()).to_string();
    let me = bot.to_string();

    // 1. Immediate win.
    for &c in &legal {
        if wins_now(&mut sim, &me, c) {
            return Some(c);
        }
    }
    // 2. Block the opponent's immediate win. The engine only lets the side to move play, so a
    //    "what if the opponent were to move" probe uses a copy with the turn handed over.
    let mut probe = pass_turn(game, slot.other());
    for &c in &legal {
        if wins_now(&mut probe, &opp, c) {
            return Some(c);
        }
    }

    match params.difficulty {
        Difficulty::Easy => {
            // Avoid moves after which the opponent wins at once, if any safe move exists.
            let safe: Vec<usize> = legal
                .iter()
                .copied()
                .filter(|&c| !gives_opponent_win(&mut sim, &me, &opp, c))
                .collect();
            let pool = if safe.is_empty() { &legal } else { &safe };
            // Centre-weighted pick: earlier entries of ORDER are more likely.
            let total: u32 = (1..=pool.len() as u32).sum();
            let mut roll = rng.gen_range(0..total);
            for (i, &c) in pool.iter().enumerate() {
                let w = (pool.len() - i) as u32;
                if roll < w {
                    return Some(c);
                }
                roll -= w;
            }
            pool.first().copied()
        }
        d => {
            let mut best = legal[0];
            // Iterative deepening; keep the result of the last *completed* depth.
            for depth in 1..=d.max_depth() {
                let mut best_this: Option<(i32, usize)> = None;
                let mut alpha = -WIN - 1;
                let mut aborted = false;
                for &c in &legal {
                    if started.elapsed() > params.budget {
                        aborted = true;
                        break;
                    }
                    let Ok(m) = sim.execute(&me, c) else { continue };
                    let score = match &m.status {
                        GameStatus::Won { .. } => WIN,
                        GameStatus::Draw => 0,
                        GameStatus::InProgress => {
                            -negamax(&mut sim, &opp, &me, depth - 1, -WIN - 1, -alpha, started, params.budget, &mut aborted)
                        }
                    };
                    let _ = sim.rollback(&m.rollback);
                    if aborted {
                        break;
                    }
                    // Tie-break randomly but reproducibly.
                    let jitter = rng.gen_range(0..2);
                    let better = match best_this {
                        None => true,
                        Some((s, _)) => score > s || (score == s && jitter == 0 && score > -WIN),
                    };
                    if better {
                        best_this = Some((score, c));
                    }
                    alpha = alpha.max(score);
                }
                if aborted {
                    break;
                }
                if let Some((score, c)) = best_this {
                    best = c;
                    if score >= WIN - 100 {
                        break; // forced win found
                    }
                }
            }
            Some(best)
        }
    }
}

/// A copy of `game` in which `slot` is to move (used only to probe threats).
fn pass_turn(game: &ConnectFour, slot: super::connect_four::Slot) -> ConnectFour {
    let mut state = game.state().clone();
    state.next = slot;
    ConnectFour::from_state(
        game.players()[0].clone(),
        game.players()[1].clone(),
        state,
    )
    .unwrap_or_else(|_| game.clone())
}

fn gives_opponent_win(sim: &mut ConnectFour, me: &str, opp: &str, column: usize) -> bool {
    let Ok(m) = sim.execute(me, column) else { return false };
    let mut danger = false;
    if matches!(m.status, GameStatus::InProgress) {
        for &c in &legal_columns(sim) {
            if wins_now(sim, opp, c) {
                danger = true;
                break;
            }
        }
    }
    let _ = sim.rollback(&m.rollback);
    danger
}

#[allow(clippy::too_many_arguments)]
fn negamax(
    game: &mut ConnectFour,
    mover: &str,
    other: &str,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    started: Instant,
    budget: Duration,
    aborted: &mut bool,
) -> i32 {
    if *aborted || started.elapsed() > budget {
        *aborted = true;
        return 0;
    }
    if depth == 0 {
        return evaluate(game, mover, other);
    }
    let legal = legal_columns(game);
    if legal.is_empty() {
        return 0;
    }
    let mut best = -WIN - 1;
    for c in legal {
        let Ok(m) = game.execute(mover, c) else { continue };
        let score = match &m.status {
            GameStatus::Won { .. } => WIN + depth as i32, // faster wins score higher
            GameStatus::Draw => 0,
            GameStatus::InProgress => {
                -negamax(game, other, mover, depth - 1, -beta, -alpha, started, budget, aborted)
            }
        };
        let _ = game.rollback(&m.rollback);
        if *aborted {
            return 0;
        }
        best = best.max(score);
        alpha = alpha.max(score);
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Static evaluation from `mover`'s point of view: window counting + centre control.
fn evaluate(game: &ConnectFour, mover: &str, other: &str) -> i32 {
    let (Some(ms), Some(os)) = (game.slot_of(mover), game.slot_of(other)) else { return 0 };
    let (mv, ov) = (ms.cell_value(), os.cell_value());
    let mut score = 0i32;
    for r in 0..ROWS {
        if game.cell(r, 3) == mv {
            score += 3;
        } else if game.cell(r, 3) == ov {
            score -= 3;
        }
    }
    let mut window = |cells: [u8; 4]| {
        let m = cells.iter().filter(|c| **c == mv).count();
        let o = cells.iter().filter(|c| **c == ov).count();
        if m > 0 && o > 0 {
            return;
        }
        score += match (m, o) {
            (3, 0) => 5,
            (2, 0) => 2,
            (0, 3) => -6,
            (0, 2) => -2,
            _ => 0,
        };
    };
    for r in 0..ROWS {
        for c in 0..COLUMNS {
            if c + 3 < COLUMNS {
                window([game.cell(r, c), game.cell(r, c + 1), game.cell(r, c + 2), game.cell(r, c + 3)]);
            }
            if r + 3 < ROWS {
                window([game.cell(r, c), game.cell(r + 1, c), game.cell(r + 2, c), game.cell(r + 3, c)]);
            }
            if r + 3 < ROWS && c + 3 < COLUMNS {
                window([game.cell(r, c), game.cell(r + 1, c + 1), game.cell(r + 2, c + 2), game.cell(r + 3, c + 3)]);
            }
            if r >= 3 && c + 3 < COLUMNS {
                window([game.cell(r, c), game.cell(r - 1, c + 1), game.cell(r - 2, c + 2), game.cell(r - 3, c + 3)]);
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "human";
    const B: &str = "bot";

    fn params(d: Difficulty, seed: u64) -> BotParams {
        BotParams { difficulty: d, seed, budget: Duration::from_secs(5) }
    }

    /// Game with the bot as second player; plays `moves` alternately starting with the human.
    fn game_after(moves: &[usize]) -> ConnectFour {
        let mut g = ConnectFour::new(H, B);
        for (i, c) in moves.iter().enumerate() {
            let p = if i % 2 == 0 { H } else { B };
            g.execute(p, *c).unwrap();
        }
        g
    }

    #[test]
    fn always_returns_a_legal_column_for_every_difficulty() {
        for d in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            for seed in 0..5 {
                let g = game_after(&[3]);
                let c = choose_column(&g, B, params(d, seed)).unwrap();
                assert!(c < COLUMNS);
                let mut g2 = g.clone();
                g2.execute(B, c).expect("bot move must be legal");
            }
        }
    }

    #[test]
    fn takes_an_immediate_win() {
        // bot has 0,0,0 stacked? Build: bot discs in column 4 rows 0..2, human elsewhere.
        // H:0, B:4, H:1, B:4, H:0, B:4, H:1 -> bot to move can win with column 4.
        let g = game_after(&[0, 4, 1, 4, 0, 4, 1]);
        for d in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            assert_eq!(choose_column(&g, B, params(d, 7)), Some(4), "{d:?}");
        }
    }

    #[test]
    fn blocks_an_immediate_loss() {
        // Human has 0,0,0 in a column (rows 0-2); bot discs in column 1; human threatens col 0.
        // H:0, B:1, H:0, B:1, H:0 -> bot to move must block column 0.
        let g = game_after(&[0, 1, 0, 1, 0]);
        for d in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            assert_eq!(choose_column(&g, B, params(d, 3)), Some(0), "{d:?}");
        }
    }

    #[test]
    fn refuses_when_it_is_not_its_turn_or_the_game_is_over() {
        let g = game_after(&[]); // human to move
        assert_eq!(choose_column(&g, B, params(Difficulty::Medium, 1)), None);
        let mut won = game_after(&[0, 1, 0, 1, 0, 1]);
        won.execute(H, 0).unwrap(); // human wins
        assert!(won.status().is_finished());
        assert_eq!(choose_column(&won, B, params(Difficulty::Medium, 1)), None);
        assert_eq!(choose_column(&won, H, params(Difficulty::Medium, 1)), None);
    }

    #[test]
    fn is_deterministic_for_a_fixed_seed() {
        let g = game_after(&[3, 3, 2]);
        for d in [Difficulty::Easy, Difficulty::Medium] {
            let a = choose_column(&g, B, params(d, 42));
            let b = choose_column(&g, B, params(d, 42));
            assert_eq!(a, b, "{d:?}");
        }
    }

    #[test]
    fn respects_its_time_budget() {
        let g = game_after(&[3, 3, 2, 4, 2]);
        let started = Instant::now();
        let p = BotParams { difficulty: Difficulty::Hard, seed: 1, budget: Duration::from_millis(60) };
        let c = choose_column(&g, B, p);
        assert!(c.is_some());
        assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
    }

    #[test]
    fn plays_complete_games_against_a_random_player_without_illegal_moves() {
        use rand::seq::SliceRandom;
        let mut rng = StdRng::seed_from_u64(99);
        for game_no in 0..6u64 {
            let mut g = ConnectFour::new(H, B);
            let mut guard = 0;
            while !g.status().is_finished() {
                guard += 1;
                assert!(guard <= 42);
                if g.current_player() == Some(H) {
                    let legal = legal_columns(&g);
                    let c = *legal.choose(&mut rng).unwrap();
                    g.execute(H, c).unwrap();
                } else {
                    let c = choose_column(&g, B, BotParams {
                        difficulty: if game_no % 2 == 0 { Difficulty::Medium } else { Difficulty::Easy },
                        seed: game_no,
                        budget: Duration::from_secs(2),
                    })
                    .expect("bot must move when it is its turn");
                    g.execute(B, c).expect("legal");
                }
            }
            // After the end, the bot must not produce another move.
            assert_eq!(choose_column(&g, B, params(Difficulty::Medium, 0)), None);
        }
    }

    #[test]
    fn medium_beats_a_random_opponent_most_of_the_time() {
        use rand::seq::SliceRandom;
        let mut rng = StdRng::seed_from_u64(5);
        let mut bot_wins = 0;
        for i in 0..8u64 {
            let mut g = ConnectFour::new(H, B);
            while !g.status().is_finished() {
                if g.current_player() == Some(H) {
                    let c = *legal_columns(&g).choose(&mut rng).unwrap();
                    g.execute(H, c).unwrap();
                } else {
                    let c = choose_column(&g, B, BotParams { difficulty: Difficulty::Medium, seed: i, budget: Duration::from_secs(2) }).unwrap();
                    g.execute(B, c).unwrap();
                }
            }
            if g.winner() == Some(B) {
                bot_wins += 1;
            }
        }
        assert!(bot_wins >= 7, "medium bot won only {bot_wins}/8 against random");
    }
}
