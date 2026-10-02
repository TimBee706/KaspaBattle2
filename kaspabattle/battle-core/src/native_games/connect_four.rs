//! Connect Four (Vier Gewinnt) — 7 columns × 6 rows, exactly two players.
//!
//! Coordinates: `row` 0 is the **bottom** row, `column` 0 is the leftmost column. A client only ever
//! sends a column; the engine finds the lowest free row (gravity).

use super::hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const COLUMNS: usize = 7;
pub const ROWS: usize = 6;
pub const CELLS: usize = COLUMNS * ROWS;

/// Seat at the table. `One` is the match creator and moves first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    One,
    Two,
}

impl Slot {
    pub fn other(self) -> Slot {
        match self {
            Slot::One => Slot::Two,
            Slot::Two => Slot::One,
        }
    }

    /// Cell value used in the board (0 is empty).
    pub fn cell_value(self) -> u8 {
        match self {
            Slot::One => 1,
            Slot::Two => 2,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Slot::One => 0,
            Slot::Two => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    InProgress,
    /// `line` holds every connected `(row, column)` of the winning run (≥ 4 cells).
    Won {
        winner: Slot,
        line: Vec<(u8, u8)>,
    },
    Draw,
}

impl GameStatus {
    pub fn is_finished(&self) -> bool {
        !matches!(self, GameStatus::InProgress)
    }

    fn hash_tag(&self) -> &'static str {
        match self {
            GameStatus::InProgress => "P",
            GameStatus::Won {
                winner: Slot::One, ..
            } => "W1",
            GameStatus::Won {
                winner: Slot::Two, ..
            } => "W2",
            GameStatus::Draw => "D",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    /// The caller is not one of the two players of this game.
    Unauthorized,
    /// The game already ended (win, draw, …).
    GameOver,
    NotYourTurn,
    /// Column index outside `0..=6`.
    InvalidColumn(usize),
    ColumnFull(usize),
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::Unauthorized => write!(f, "player is not part of this game"),
            MoveError::GameOver => write!(f, "game is already finished"),
            MoveError::NotYourTurn => write!(f, "it is not your turn"),
            MoveError::InvalidColumn(c) => write!(f, "column {c} is out of range (0-6)"),
            MoveError::ColumnFull(c) => write!(f, "column {c} is full"),
        }
    }
}

impl std::error::Error for MoveError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackError {
    /// The rollback record does not describe the most recent move.
    NotLastMove,
}

impl std::fmt::Display for RollbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rollback record does not match the last move")
    }
}

impl std::error::Error for RollbackError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    BadLength(usize),
    BadCellValue(u8),
    /// A disc floats above an empty cell.
    Gravity,
    /// `move_count` / `next` do not match the disc counts.
    Inconsistent,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid connect-four state: {self:?}")
    }
}

impl std::error::Error for StateError {}

/// Everything needed to undo exactly one move (kdapp `CommandRollback`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveRollback {
    pub column: u8,
    pub row: u8,
    pub slot: Slot,
    prev_next: Slot,
    prev_status: GameStatus,
}

/// Result of a successful [`ConnectFour::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveApplied {
    pub column: u8,
    pub row: u8,
    pub slot: Slot,
    pub status: GameStatus,
    pub rollback: MoveRollback,
}

/// Serializable board state (persisted as JSON in `native_game_sessions.board_state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectFourState {
    /// Row-major, `index = row * 7 + column`, row 0 = bottom. 0 = empty, 1/2 = slot One/Two.
    pub cells: Vec<u8>,
    pub next: Slot,
    pub move_count: u8,
    pub status: GameStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectFour {
    players: [String; 2],
    state: ConnectFourState,
}

impl ConnectFour {
    /// New empty game. `player_one` moves first.
    pub fn new(player_one: impl Into<String>, player_two: impl Into<String>) -> Self {
        ConnectFour {
            players: [player_one.into(), player_two.into()],
            state: ConnectFourState {
                cells: vec![0; CELLS],
                next: Slot::One,
                move_count: 0,
                status: GameStatus::InProgress,
            },
        }
    }

    /// Restores a game from persisted state, rejecting anything that could not have been reached
    /// through legal play (defends against a corrupted / tampered `board_state`).
    pub fn from_state(
        player_one: impl Into<String>,
        player_two: impl Into<String>,
        state: ConnectFourState,
    ) -> Result<Self, StateError> {
        if state.cells.len() != CELLS {
            return Err(StateError::BadLength(state.cells.len()));
        }
        if let Some(bad) = state.cells.iter().find(|c| **c > 2) {
            return Err(StateError::BadCellValue(*bad));
        }
        for col in 0..COLUMNS {
            let mut seen_empty = false;
            for row in 0..ROWS {
                let empty = state.cells[row * COLUMNS + col] == 0;
                if empty {
                    seen_empty = true;
                } else if seen_empty {
                    return Err(StateError::Gravity);
                }
            }
        }
        let ones = state.cells.iter().filter(|c| **c == 1).count();
        let twos = state.cells.iter().filter(|c| **c == 2).count();
        if ones + twos != state.move_count as usize || !(ones == twos || ones == twos + 1) {
            return Err(StateError::Inconsistent);
        }
        if !state.status.is_finished() {
            let expected_next = if ones == twos { Slot::One } else { Slot::Two };
            if state.next != expected_next {
                return Err(StateError::Inconsistent);
            }
        }
        Ok(ConnectFour {
            players: [player_one.into(), player_two.into()],
            state,
        })
    }

    pub fn state(&self) -> &ConnectFourState {
        &self.state
    }

    pub fn status(&self) -> &GameStatus {
        &self.state.status
    }

    pub fn move_count(&self) -> u8 {
        self.state.move_count
    }

    pub fn players(&self) -> &[String; 2] {
        &self.players
    }

    pub fn slot_of(&self, player: &str) -> Option<Slot> {
        if self.players[0] == player {
            Some(Slot::One)
        } else if self.players[1] == player {
            Some(Slot::Two)
        } else {
            None
        }
    }

    pub fn player_at(&self, slot: Slot) -> &str {
        &self.players[slot.index()]
    }

    /// The player whose turn it is, or `None` once the game is over.
    pub fn current_player(&self) -> Option<&str> {
        if self.state.status.is_finished() {
            None
        } else {
            Some(self.player_at(self.state.next))
        }
    }

    pub fn winner(&self) -> Option<&str> {
        match &self.state.status {
            GameStatus::Won { winner, .. } => Some(self.player_at(*winner)),
            _ => None,
        }
    }

    /// Cell value at `(row, column)` (row 0 = bottom).
    pub fn cell(&self, row: usize, column: usize) -> u8 {
        self.state.cells[row * COLUMNS + column]
    }

    /// Lowest free row in `column`, if any.
    fn drop_row(&self, column: usize) -> Option<usize> {
        (0..ROWS).find(|row| self.cell(*row, column) == 0)
    }

    /// Applies a move for `player` (identified by their user id) in `column`.
    ///
    /// Check order: participant → game over → turn → column range → column full.
    pub fn execute(&mut self, player: &str, column: usize) -> Result<MoveApplied, MoveError> {
        let slot = self.slot_of(player).ok_or(MoveError::Unauthorized)?;
        if self.state.status.is_finished() {
            return Err(MoveError::GameOver);
        }
        if slot != self.state.next {
            return Err(MoveError::NotYourTurn);
        }
        if column >= COLUMNS {
            return Err(MoveError::InvalidColumn(column));
        }
        let row = self.drop_row(column).ok_or(MoveError::ColumnFull(column))?;

        let rollback = MoveRollback {
            column: column as u8,
            row: row as u8,
            slot,
            prev_next: self.state.next,
            prev_status: self.state.status.clone(),
        };

        self.state.cells[row * COLUMNS + column] = slot.cell_value();
        self.state.move_count += 1;

        if let Some(line) = self.winning_line(row, column, slot) {
            self.state.status = GameStatus::Won { winner: slot, line };
        } else if self.state.move_count as usize == CELLS {
            self.state.status = GameStatus::Draw;
        } else {
            self.state.next = slot.other();
        }

        Ok(MoveApplied {
            column: column as u8,
            row: row as u8,
            slot,
            status: self.state.status.clone(),
            rollback,
        })
    }

    /// Undoes the most recent move. Only the last move can be rolled back (LIFO), including the
    /// winning move.
    pub fn rollback(&mut self, rb: &MoveRollback) -> Result<(), RollbackError> {
        let (row, col) = (rb.row as usize, rb.column as usize);
        if row >= ROWS || col >= COLUMNS || self.state.move_count == 0 {
            return Err(RollbackError::NotLastMove);
        }
        let is_top = self.cell(row, col) == rb.slot.cell_value()
            && (row + 1 == ROWS || self.cell(row + 1, col) == 0);
        if !is_top {
            return Err(RollbackError::NotLastMove);
        }
        self.state.cells[row * COLUMNS + col] = 0;
        self.state.move_count -= 1;
        self.state.next = rb.prev_next;
        self.state.status = rb.prev_status.clone();
        Ok(())
    }

    /// All connected cells (≥ 4) through `(row, column)` for `slot`, if any.
    fn winning_line(&self, row: usize, column: usize, slot: Slot) -> Option<Vec<(u8, u8)>> {
        let v = slot.cell_value();
        // horizontal, vertical, diagonal ↗, diagonal ↘
        for (dr, dc) in [(0i32, 1i32), (1, 0), (1, 1), (-1, 1)] {
            let mut line = vec![(row as u8, column as u8)];
            for sign in [-1i32, 1] {
                let (mut r, mut c) = (row as i32 + sign * dr, column as i32 + sign * dc);
                while (0..ROWS as i32).contains(&r)
                    && (0..COLUMNS as i32).contains(&c)
                    && self.cell(r as usize, c as usize) == v
                {
                    line.push((r as u8, c as u8));
                    r += sign * dr;
                    c += sign * dc;
                }
            }
            if line.len() >= 4 {
                line.sort();
                return Some(line);
            }
        }
        None
    }

    /// Deterministic SHA-256 over the canonical state (cells, side to move, move count, status).
    pub fn state_hash(&self) -> String {
        let digits: String = self
            .state
            .cells
            .iter()
            .map(|c| char::from(b'0' + *c))
            .collect();
        let mut h = Sha256::new();
        h.update(b"kaspabattle-connect-four-v1|");
        h.update(digits.as_bytes());
        h.update(b"|");
        h.update([b'0' + self.state.next.cell_value()]);
        h.update(b"|");
        h.update(self.state.move_count.to_string().as_bytes());
        h.update(b"|");
        h.update(self.state.status.hash_tag().as_bytes());
        hex_encode(&h.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "player-a";
    const B: &str = "player-b";

    fn game() -> ConnectFour {
        ConnectFour::new(A, B)
    }

    /// Plays `(player, column)` pairs, asserting each succeeds.
    fn play(g: &mut ConnectFour, moves: &[(&str, usize)]) {
        for (p, c) in moves {
            g.execute(p, *c)
                .unwrap_or_else(|e| panic!("move {p}/{c} failed: {e}"));
        }
    }

    #[test]
    fn first_disc_falls_to_bottom_and_turn_alternates() {
        let mut g = game();
        let m = g.execute(A, 3).unwrap();
        assert_eq!((m.row, m.column), (0, 3));
        assert_eq!(g.cell(0, 3), 1);
        assert_eq!(g.current_player(), Some(B));
        let m = g.execute(B, 3).unwrap();
        assert_eq!(m.row, 1);
        assert_eq!(g.cell(1, 3), 2);
    }

    #[test]
    fn horizontal_win() {
        let mut g = game();
        play(&mut g, &[(A, 0), (B, 0), (A, 1), (B, 1), (A, 2), (B, 2)]);
        let m = g.execute(A, 3).unwrap();
        assert!(matches!(
            m.status,
            GameStatus::Won {
                winner: Slot::One,
                ..
            }
        ));
        assert_eq!(g.winner(), Some(A));
        assert_eq!(g.current_player(), None);
    }

    #[test]
    fn vertical_win() {
        let mut g = game();
        play(&mut g, &[(A, 0), (B, 1), (A, 0), (B, 1), (A, 0), (B, 1)]);
        let m = g.execute(A, 0).unwrap();
        match m.status {
            GameStatus::Won { winner, line } => {
                assert_eq!(winner, Slot::One);
                assert_eq!(line, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
            }
            other => panic!("expected win, got {other:?}"),
        }
    }

    #[test]
    fn diagonal_up_right_win() {
        // A builds (0,0) (1,1) (2,2) (3,3)
        let mut g = game();
        play(
            &mut g,
            &[
                (A, 0),
                (B, 1),
                (A, 1),
                (B, 2),
                (A, 6),
                (B, 2),
                (A, 2),
                (B, 3),
                (A, 3),
                (B, 3),
            ],
        );
        let m = g.execute(A, 3).unwrap();
        assert!(
            matches!(
                m.status,
                GameStatus::Won {
                    winner: Slot::One,
                    ..
                }
            ),
            "{:?}",
            m.status
        );
    }

    #[test]
    fn diagonal_down_right_win() {
        // Mirror image: A builds (0,6) (1,5) (2,4) (3,3)
        let mut g = game();
        play(
            &mut g,
            &[
                (A, 6),
                (B, 5),
                (A, 5),
                (B, 4),
                (A, 0),
                (B, 4),
                (A, 4),
                (B, 3),
                (A, 3),
                (B, 3),
            ],
        );
        let m = g.execute(A, 3).unwrap();
        assert!(
            matches!(
                m.status,
                GameStatus::Won {
                    winner: Slot::One,
                    ..
                }
            ),
            "{:?}",
            m.status
        );
    }

    #[test]
    fn full_column_is_rejected() {
        let mut g = game();
        // Alternate players in column 0 without connecting four: A,B,A,B,A,B fill 6 rows.
        play(&mut g, &[(A, 0), (B, 0), (A, 0), (B, 0), (A, 0), (B, 0)]);
        assert_eq!(g.execute(A, 0), Err(MoveError::ColumnFull(0)));
        // A rejected move must not change anything or pass the turn.
        assert_eq!(g.current_player(), Some(A));
        assert_eq!(g.move_count(), 6);
    }

    #[test]
    fn invalid_column_is_rejected() {
        let mut g = game();
        assert_eq!(g.execute(A, 7), Err(MoveError::InvalidColumn(7)));
        assert_eq!(
            g.execute(A, usize::MAX),
            Err(MoveError::InvalidColumn(usize::MAX))
        );
        assert_eq!(g.move_count(), 0);
    }

    #[test]
    fn wrong_player_is_rejected() {
        let mut g = game();
        assert_eq!(g.execute(B, 0), Err(MoveError::NotYourTurn));
        g.execute(A, 0).unwrap();
        assert_eq!(g.execute(A, 1), Err(MoveError::NotYourTurn));
    }

    #[test]
    fn unauthorized_player_is_rejected() {
        let mut g = game();
        assert_eq!(g.execute("mallory", 0), Err(MoveError::Unauthorized));
        assert_eq!(g.move_count(), 0);
    }

    #[test]
    fn no_moves_after_game_over() {
        let mut g = game();
        play(
            &mut g,
            &[(A, 0), (B, 0), (A, 1), (B, 1), (A, 2), (B, 2), (A, 3)],
        );
        assert!(g.status().is_finished());
        assert_eq!(g.execute(B, 4), Err(MoveError::GameOver));
        assert_eq!(g.execute(A, 4), Err(MoveError::GameOver));
        // Non-participants are still reported as unauthorized, not as game over.
        assert_eq!(g.execute("mallory", 4), Err(MoveError::Unauthorized));
    }

    #[test]
    fn draw_game() {
        // Search for a legal, win-free full board deterministically (depth-first, bounded).
        fn dfs(g: &mut ConnectFour, players: [&str; 2]) -> bool {
            if g.move_count() as usize == CELLS {
                return matches!(g.status(), GameStatus::Draw);
            }
            let p = players[(g.move_count() % 2) as usize];
            for col in 0..COLUMNS {
                if let Ok(m) = g.execute(p, col) {
                    if matches!(m.status, GameStatus::InProgress | GameStatus::Draw)
                        && dfs(g, players)
                    {
                        return true;
                    }
                    g.rollback(&m.rollback).unwrap();
                }
            }
            false
        }
        let mut g = game();
        assert!(
            dfs(&mut g, [A, B]),
            "a drawn game exists and must be reachable"
        );
        assert_eq!(g.status(), &GameStatus::Draw);
        assert_eq!(g.move_count() as usize, CELLS);
        assert_eq!(g.winner(), None);
        assert_eq!(g.current_player(), None);
        assert_eq!(g.execute(A, 0), Err(MoveError::GameOver));
    }

    #[test]
    fn state_hash_is_deterministic_and_state_sensitive() {
        let mut g1 = game();
        let mut g2 = game();
        assert_eq!(g1.state_hash(), g2.state_hash());
        play(&mut g1, &[(A, 3), (B, 4)]);
        play(&mut g2, &[(A, 3), (B, 4)]);
        assert_eq!(g1.state_hash(), g2.state_hash());
        assert_eq!(g1.state_hash().len(), 64);
        g2.execute(A, 0).unwrap();
        assert_ne!(g1.state_hash(), g2.state_hash());
        // Same discs reached in a different order yields the same board but the same hash only when
        // side-to-move and count agree — here transposition gives an identical state.
        let mut g3 = game();
        play(&mut g3, &[(A, 3), (B, 4)]);
        assert_eq!(g1.state_hash(), g3.state_hash());
    }

    #[test]
    fn rollback_of_normal_move_restores_exact_state() {
        let mut g = game();
        play(&mut g, &[(A, 3), (B, 3)]);
        let before = g.clone();
        let hash_before = g.state_hash();
        let m = g.execute(A, 2).unwrap();
        assert_ne!(g.state_hash(), hash_before);
        g.rollback(&m.rollback).unwrap();
        assert_eq!(g, before);
        assert_eq!(g.state_hash(), hash_before);
        assert_eq!(g.current_player(), Some(A));
    }

    #[test]
    fn rollback_of_winning_move_reopens_the_game() {
        let mut g = game();
        play(&mut g, &[(A, 0), (B, 0), (A, 1), (B, 1), (A, 2), (B, 2)]);
        let before = g.clone();
        let win = g.execute(A, 3).unwrap();
        assert!(win.status.is_finished());
        g.rollback(&win.rollback).unwrap();
        assert_eq!(g, before);
        assert_eq!(g.status(), &GameStatus::InProgress);
        assert_eq!(g.current_player(), Some(A));
        // …and the game can continue after the rollback.
        g.execute(A, 4).unwrap();
    }

    #[test]
    fn rollback_only_accepts_the_last_move() {
        let mut g = game();
        let first = g.execute(A, 3).unwrap();
        g.execute(B, 3).unwrap();
        // `first` is buried under B's disc → must be refused and change nothing.
        let snapshot = g.clone();
        assert_eq!(g.rollback(&first.rollback), Err(RollbackError::NotLastMove));
        assert_eq!(g, snapshot);
    }

    #[test]
    fn state_roundtrips_through_json_and_validates() {
        let mut g = game();
        play(&mut g, &[(A, 3), (B, 3), (A, 4)]);
        let json = serde_json::to_value(g.state()).unwrap();
        let restored: ConnectFourState = serde_json::from_value(json).unwrap();
        let g2 = ConnectFour::from_state(A, B, restored).unwrap();
        assert_eq!(g, g2);
        assert_eq!(g.state_hash(), g2.state_hash());
    }

    #[test]
    fn from_state_rejects_corrupted_boards() {
        let mut s = game().state().clone();
        s.cells.pop();
        assert_eq!(
            ConnectFour::from_state(A, B, s),
            Err(StateError::BadLength(41))
        );

        let mut s = game().state().clone();
        s.cells[COLUMNS] = 1; // floating disc at row 1
        s.move_count = 1;
        assert_eq!(ConnectFour::from_state(A, B, s), Err(StateError::Gravity));

        let mut s = game().state().clone();
        s.cells[0] = 3;
        assert_eq!(
            ConnectFour::from_state(A, B, s),
            Err(StateError::BadCellValue(3))
        );

        let mut s = game().state().clone();
        s.cells[0] = 2; // player two moved first
        s.move_count = 1;
        assert_eq!(
            ConnectFour::from_state(A, B, s),
            Err(StateError::Inconsistent)
        );
    }

    #[test]
    fn result_hash_binds_all_inputs() {
        use super::super::{result_hash, EndReason};
        let h1 = result_hash("m1", "CONNECT_FOUR", "abc", Some(A), EndReason::ConnectFour);
        assert_eq!(
            h1,
            result_hash("m1", "CONNECT_FOUR", "abc", Some(A), EndReason::ConnectFour)
        );
        assert_ne!(
            h1,
            result_hash("m2", "CONNECT_FOUR", "abc", Some(A), EndReason::ConnectFour)
        );
        assert_ne!(
            h1,
            result_hash("m1", "CONNECT_FOUR", "abc", Some(B), EndReason::ConnectFour)
        );
        assert_ne!(
            h1,
            result_hash("m1", "CONNECT_FOUR", "abc", None, EndReason::Draw)
        );
    }

    /// Property-style invariants over many seeded random playouts (no extra dependency).
    #[test]
    fn random_playouts_keep_the_board_consistent() {
        use rand::{rngs::StdRng, Rng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(0xC0FFEE);
        for _ in 0..300 {
            let mut g = game();
            let mut last_count = 0u8;
            while !g.status().is_finished() {
                let player = g.current_player().unwrap().to_string();
                let col = rng.gen_range(0..COLUMNS);
                let before = g.state().cells.clone();
                match g.execute(&player, col) {
                    Ok(m) => {
                        // exactly one new disc, on the bottom-most free cell, by the mover
                        let diff: Vec<usize> = (0..CELLS)
                            .filter(|i| before[*i] != g.state().cells[*i])
                            .collect();
                        assert_eq!(diff.len(), 1);
                        let (r, c) = (diff[0] / COLUMNS, diff[0] % COLUMNS);
                        assert_eq!((r as u8, c as u8), (m.row, m.column));
                        assert!(r == 0 || g.cell(r - 1, c) != 0, "no floating disc");
                        assert_eq!(g.cell(r, c), m.slot.cell_value());
                        assert_eq!(
                            g.move_count(),
                            last_count + 1,
                            "move count rises by exactly one"
                        );
                        last_count = g.move_count();
                    }
                    Err(MoveError::ColumnFull(_)) => {
                        assert_eq!(g.state().cells, before, "rejected move changes nothing")
                    }
                    Err(e) => panic!("unexpected {e}"),
                }
                assert!(g.state().cells.iter().all(|c| *c <= 2));
                assert!(
                    ConnectFour::from_state("a", "b", g.state().clone()).is_ok(),
                    "every reachable state validates"
                );
            }
            // a finished board never changes again
            let frozen = g.clone();
            for col in 0..COLUMNS {
                assert_eq!(g.execute(A, col), Err(MoveError::GameOver));
                assert_eq!(g.execute(B, col), Err(MoveError::GameOver));
            }
            assert_eq!(g, frozen);
        }
    }
}
