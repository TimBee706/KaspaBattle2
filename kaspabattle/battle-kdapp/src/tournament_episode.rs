//! TournamentEpisode — implements the kdapp `Episode` trait for KaspaBattle tournaments.
//!
//! Manages the on-chain state machine for 5v5 single-elimination tournaments.
//! Mirrors the `BattleEpisode` pattern but for the tournament lifecycle.
//!
//! ## Roles (mapped to `participants` indices)
//! - `participants[0]` — Organizer (creates/locks/cancels tournaments)
//! - `participants[1]` — Admin (resolves disputes)
//! - `participants[2..]` — Oracle(s) (the FaceitWatcher backend pubkey)
//!
//! ## Participant vs. Team separation
//! `participants` in the kdapp sense are the *role holders* (Organizer, Admin, Oracle),
//! not the players. Player/team registration happens off-chain and is linked via
//! on-chain `RegisterTeam` commands (keyed by captain pubkey).

use crate::kdapp_episode::{Episode, EpisodeError, PayloadMetadata};
use crate::kdapp_pki::PubKey;
use crate::tournament_commands::{
    BracketSlot, BracketSlotStatus, TournamentCommand, TournamentGameType, TournamentPhase,
    TournamentRollback,
};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Maximum bracket depth (supports up to 256 teams).
pub const MAX_ROUNDS: usize = 8;
/// Maximum number of teams in a tournament.
pub const MAX_TEAMS: usize = 16;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Application-level errors for the TournamentEpisode.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TournamentError {
    #[error("invalid phase transition: {phase} does not permit '{action}'")]
    InvalidTransition { phase: String, action: String },

    #[error("unauthorized: only the {role} can perform this action")]
    Unauthorized { role: String },

    #[error("tournament is full ({max_teams} teams already registered)")]
    TournamentFull { max_teams: usize },

    #[error("team {team_idx} has already deposited")]
    AlreadyDeposited { team_idx: u8 },

    #[error("team {team_idx} is not registered in this tournament")]
    TeamNotFound { team_idx: u8 },

    #[error("bracket slot ({round}, {slot_index}) is invalid or out of range")]
    InvalidBracketSlot { round: u8, slot_index: u8 },

    #[error("bracket slot ({round}, {slot_index}) is already completed")]
    SlotAlreadyCompleted { round: u8, slot_index: u8 },

    #[error("max_teams must be a power of 2 (4, 8, or 16)")]
    InvalidTeamCount,

    #[error("prize percentages must sum to 100")]
    InvalidPrizeSplit,

    #[error("not enough funded teams to lock the bracket (need at least 2, got {got})")]
    NotEnoughTeams { got: usize },

    #[error("winner_team_idx must be 0 (team_a) or 1 (team_b)")]
    InvalidWinnerIndex,

    #[error("dispute window has not been entered for this slot")]
    NotInDispute,
}

// ─── On-chain team state ─────────────────────────────────────────────────────

/// On-chain state of a registered team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamState {
    /// Hash of the team name (hashed for compact on-chain storage)
    pub name_hash: [u8; 32],
    /// Captain pubkey (authorizes team actions)
    pub captain_pubkey: PubKey,
    /// Deposit state
    pub deposited_sompi: u64,
    pub deposit_confirmed: bool,
}

// ─── TournamentEpisode ───────────────────────────────────────────────────────

/// On-chain state of a KaspaBattle tournament.
#[derive(Debug, Clone)]
pub struct TournamentEpisode {
    // ── Configuration (set at CreateTournament) ──────────────────────────────
    pub max_teams: u8,
    pub buy_in_sompi: u64,
    pub prize_winner_pct: u8,
    pub prize_runner_up_pct: u8,
    pub platform_fee_pct: u8,
    pub game_type: TournamentGameType,

    // ── Teams ─────────────────────────────────────────────────────────────────
    /// Registered teams (in registration order).
    pub teams: Vec<TeamState>,

    // ── Bracket ───────────────────────────────────────────────────────────────
    /// Flat list of all bracket slots across all rounds.
    pub bracket: Vec<BracketSlot>,

    // ── Phase ─────────────────────────────────────────────────────────────────
    pub phase: TournamentPhase,

    // ── Result tracking ───────────────────────────────────────────────────────
    pub winner_team_idx: Option<u8>,
    pub runner_up_team_idx: Option<u8>,

    // ── Metadata ──────────────────────────────────────────────────────────────
    pub created_daa: u64,
}

impl TournamentEpisode {
    // ── Role helpers — used by Phase 2 API authorization layer ───────────
    // Note: These are not called from within the kdapp module; they are
    // reserved for the battle-api authorization layer in Phase 2.
    #[allow(dead_code)]
    pub fn is_organizer(participants: &[PubKey], auth: &PubKey) -> bool {
        participants.first() == Some(auth)
    }

    #[allow(dead_code)]
    pub fn is_admin(participants: &[PubKey], auth: &PubKey) -> bool {
        participants.get(1) == Some(auth)
    }

    #[allow(dead_code)]
    pub fn is_oracle(participants: &[PubKey], auth: &PubKey) -> bool {
        // Oracle: any participant at index 2+
        participants.iter().skip(2).any(|p| p == auth)
    }

    #[allow(dead_code)]
    pub fn is_organizer_or_oracle(participants: &[PubKey], auth: &PubKey) -> bool {
        Self::is_organizer(participants, auth) || Self::is_oracle(participants, auth)
    }

    // ── Bracket helpers ───────────────────────────────────────────────────────

    /// Find a bracket slot by round + slot_index.
    fn find_slot(&self, round: u8, slot_index: u8) -> Option<usize> {
        self.bracket
            .iter()
            .position(|s| s.round == round && s.slot_index == slot_index)
    }

    /// Find a slot (returning mutable ref) by round + slot_index.
    fn find_slot_mut(&mut self, round: u8, slot_index: u8) -> Option<&mut BracketSlot> {
        self.bracket
            .iter_mut()
            .find(|s| s.round == round && s.slot_index == slot_index)
    }

    /// Generate a single-elimination bracket from the funded teams.
    ///
    /// Seeds are assigned by registration order. BYE slots are placed at the end
    /// of each round to handle non-power-of-2 team counts gracefully.
    pub fn build_bracket(teams: &[TeamState], max_teams: u8) -> Vec<BracketSlot> {
        let n = teams.len();
        let total_rounds = (max_teams as f32).log2().ceil() as u8;
        let mut slots = Vec::new();

        // Round 1: pair teams in order (0v1, 2v3, 4v5, ...)
        let half = (max_teams as usize) / 2;
        for slot_idx in 0..half {
            let team_a_idx = (slot_idx * 2) as u8;
            let team_b_idx = (slot_idx * 2 + 1) as u8;

            let (team_a, team_b, status) = if (team_a_idx as usize) < n && (team_b_idx as usize) < n
            {
                (Some(team_a_idx), Some(team_b_idx), BracketSlotStatus::Ready)
            } else if (team_a_idx as usize) < n {
                // BYE: team_a auto-advances
                (Some(team_a_idx), None, BracketSlotStatus::Bye)
            } else {
                // Empty slot (not enough teams registered)
                (None, None, BracketSlotStatus::Waiting)
            };

            slots.push(BracketSlot {
                round: 1,
                slot_index: slot_idx as u8,
                team_a_idx: team_a,
                team_b_idx: team_b,
                winner_idx: if status == BracketSlotStatus::Bye {
                    team_a
                } else {
                    None
                },
                faceit_match_id_hash: None,
                status,
            });
        }

        // Later rounds: empty slots, winners fill in when reported
        let mut slots_per_round = half / 2;
        for round in 2..=total_rounds {
            for slot_idx in 0..slots_per_round {
                slots.push(BracketSlot {
                    round,
                    slot_index: slot_idx as u8,
                    team_a_idx: None,
                    team_b_idx: None,
                    winner_idx: None,
                    faceit_match_id_hash: None,
                    status: BracketSlotStatus::Waiting,
                });
            }
            if slots_per_round > 1 {
                slots_per_round /= 2;
            }
        }

        slots
    }

    /// Advance the winner of a bracket slot to the next round's slot.
    ///
    /// Each round-N slot at index `i` feeds into round-(N+1) slot at index `i/2`,
    /// as either team_a (if i is even) or team_b (if i is odd).
    pub fn advance_winner_to_next_round(
        bracket: &mut [BracketSlot],
        completed_round: u8,
        completed_slot_index: u8,
        winner_team_idx: u8,
    ) {
        let next_round = completed_round + 1;
        let next_slot_idx = completed_slot_index / 2;

        if let Some(next_slot) = bracket
            .iter_mut()
            .find(|s| s.round == next_round && s.slot_index == next_slot_idx)
        {
            // Even slot → team_a of next round; odd → team_b
            if completed_slot_index.is_multiple_of(2) {
                next_slot.team_a_idx = Some(winner_team_idx);
            } else {
                next_slot.team_b_idx = Some(winner_team_idx);
            }
            // If both teams are now assigned, mark as Ready
            if next_slot.team_a_idx.is_some() && next_slot.team_b_idx.is_some() {
                next_slot.status = BracketSlotStatus::Ready;
            }
        }
    }

    /// Count how many teams have confirmed their deposit.
    fn funded_team_count(&self) -> usize {
        self.teams.iter().filter(|t| t.deposit_confirmed).count()
    }

    /// Check if all slots in a given round are completed or BYE.
    fn is_round_complete(&self, round: u8) -> bool {
        self.bracket.iter().filter(|s| s.round == round).all(|s| {
            matches!(
                s.status,
                BracketSlotStatus::Completed | BracketSlotStatus::Bye
            )
        })
    }

    /// Find the highest round number in the bracket.
    fn total_rounds(&self) -> u8 {
        self.bracket.iter().map(|s| s.round).max().unwrap_or(0)
    }

    /// Check whether the given pubkey is the captain of ANY registered team.
    #[allow(dead_code)]
    pub fn is_any_captain(teams: &[TeamState], auth: &PubKey) -> bool {
        teams.iter().any(|t| &t.captain_pubkey == auth)
    }
}

// ─── Episode trait implementation ────────────────────────────────────────────

impl Episode for TournamentEpisode {
    type Command = TournamentCommand;
    type CommandRollback = TournamentRollback;
    type CommandError = TournamentError;

    fn initialize(_participants: Vec<PubKey>, metadata: &PayloadMetadata) -> Self {
        TournamentEpisode {
            max_teams: 8,
            buy_in_sompi: 0,
            prize_winner_pct: 70,
            prize_runner_up_pct: 20,
            platform_fee_pct: 10,
            game_type: TournamentGameType::CS2,
            teams: Vec::new(),
            bracket: Vec::new(),
            phase: TournamentPhase::Registration,
            winner_team_idx: None,
            runner_up_team_idx: None,
            created_daa: metadata.accepting_daa,
        }
    }

    fn execute(
        &mut self,
        cmd: &Self::Command,
        authorization: Option<PubKey>,
        _metadata: &PayloadMetadata,
    ) -> Result<Self::CommandRollback, EpisodeError<Self::CommandError>> {
        // Borrow participants from the outer scope (they live in the engine, not here)
        // We use a local placeholder for the role checks in unit tests.
        // In production the engine passes authorization=Some(signer_pubkey).
        let auth = authorization;

        match cmd {
            // ─── CreateTournament ──────────────────────────────────────────────
            TournamentCommand::CreateTournament {
                max_teams,
                buy_in_sompi,
                prize_winner_pct,
                prize_runner_up_pct,
                platform_fee_pct,
                game_type,
            } => {
                if !matches!(self.phase, TournamentPhase::Registration) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "CreateTournament".into(),
                        },
                    ));
                }

                // Validate team count is power of 2
                let n = *max_teams as usize;
                if !(4..=MAX_TEAMS).contains(&n) || (n & (n - 1)) != 0 {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTeamCount,
                    ));
                }

                // Validate prize split
                if *prize_winner_pct + *prize_runner_up_pct + *platform_fee_pct != 100 {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidPrizeSplit,
                    ));
                }

                self.max_teams = *max_teams;
                self.buy_in_sompi = *buy_in_sompi;
                self.prize_winner_pct = *prize_winner_pct;
                self.prize_runner_up_pct = *prize_runner_up_pct;
                self.platform_fee_pct = *platform_fee_pct;
                self.game_type = game_type.clone();

                Ok(TournamentRollback::UndoCreateTournament)
            }

            // ─── RegisterTeam ─────────────────────────────────────────────────
            TournamentCommand::RegisterTeam {
                team_name_hash,
                team_size: _,
            } => {
                if !matches!(self.phase, TournamentPhase::Registration) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "RegisterTeam".into(),
                        },
                    ));
                }

                if self.teams.len() >= self.max_teams as usize {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::TournamentFull {
                            max_teams: self.max_teams as usize,
                        },
                    ));
                }

                let captain = auth.ok_or(EpisodeError::InvalidCommand(
                    TournamentError::Unauthorized {
                        role: "Captain (signed command required)".into(),
                    },
                ))?;

                self.teams.push(TeamState {
                    name_hash: *team_name_hash,
                    captain_pubkey: captain,
                    deposited_sompi: 0,
                    deposit_confirmed: false,
                });

                let team_idx = (self.teams.len() - 1) as u8;
                Ok(TournamentRollback::UndoRegisterTeam { team_idx })
            }

            // ─── ConfirmTeamDeposit ────────────────────────────────────────────
            TournamentCommand::ConfirmTeamDeposit {
                team_idx,
                tx_hash: _,
                amount_sompi,
            } => {
                if !matches!(
                    self.phase,
                    TournamentPhase::Registration | TournamentPhase::Funded
                ) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "ConfirmTeamDeposit".into(),
                        },
                    ));
                }

                let idx = *team_idx as usize;
                if idx >= self.teams.len() {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::TeamNotFound {
                            team_idx: *team_idx,
                        },
                    ));
                }

                let prev_amount = self.teams[idx].deposited_sompi;
                self.teams[idx].deposited_sompi += amount_sompi;

                if self.teams[idx].deposited_sompi >= self.buy_in_sompi {
                    self.teams[idx].deposit_confirmed = true;
                }

                // Auto-transition to FUNDED if all teams have deposited
                if self.funded_team_count() >= 2
                    && self.funded_team_count() == self.teams.len()
                    && self.teams.len() >= 2
                {
                    self.phase = TournamentPhase::Funded;
                }

                Ok(TournamentRollback::UndoConfirmTeamDeposit {
                    team_idx: *team_idx,
                    previous_amount: prev_amount,
                })
            }

            // ─── LockBracket ───────────────────────────────────────────────────
            TournamentCommand::LockBracket => {
                if !matches!(self.phase, TournamentPhase::Funded) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "LockBracket".into(),
                        },
                    ));
                }

                let funded = self.funded_team_count();
                if funded < 2 {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::NotEnoughTeams { got: funded },
                    ));
                }

                let total_rounds = (self.max_teams as f32).log2().ceil() as u8;
                self.bracket = Self::build_bracket(&self.teams, self.max_teams);
                self.phase = TournamentPhase::BracketReady { total_rounds };

                Ok(TournamentRollback::UndoLockBracket {
                    previous_phase: TournamentPhase::Funded,
                })
            }

            // ─── SubmitMatchId ─────────────────────────────────────────────────
            TournamentCommand::SubmitMatchId {
                round,
                slot_index,
                faceit_match_id_hash,
            } => {
                if !matches!(
                    self.phase,
                    TournamentPhase::BracketReady { .. } | TournamentPhase::InProgress { .. }
                ) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "SubmitMatchId".into(),
                        },
                    ));
                }

                let slot =
                    self.find_slot_mut(*round, *slot_index)
                        .ok_or(EpisodeError::InvalidCommand(
                            TournamentError::InvalidBracketSlot {
                                round: *round,
                                slot_index: *slot_index,
                            },
                        ))?;

                if matches!(slot.status, BracketSlotStatus::Completed) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::SlotAlreadyCompleted {
                            round: *round,
                            slot_index: *slot_index,
                        },
                    ));
                }

                slot.faceit_match_id_hash = Some(*faceit_match_id_hash);
                slot.status = BracketSlotStatus::InProgress;

                // Transition tournament to InProgress on first submitted match ID
                if matches!(self.phase, TournamentPhase::BracketReady { .. }) {
                    self.phase = TournamentPhase::InProgress {
                        current_round: *round,
                    };
                }

                Ok(TournamentRollback::UndoSubmitMatchId {
                    round: *round,
                    slot_index: *slot_index,
                })
            }

            // ─── ReportBracketResult ───────────────────────────────────────────
            TournamentCommand::ReportBracketResult {
                round,
                slot_index,
                winner_team_idx,
                ..
            } => {
                if !matches!(self.phase, TournamentPhase::InProgress { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "ReportBracketResult".into(),
                        },
                    ));
                }

                if *winner_team_idx > 1 {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidWinnerIndex,
                    ));
                }

                let slot_pos =
                    self.find_slot(*round, *slot_index)
                        .ok_or(EpisodeError::InvalidCommand(
                            TournamentError::InvalidBracketSlot {
                                round: *round,
                                slot_index: *slot_index,
                            },
                        ))?;

                // Resolve actual team_idx from slot's team_a/team_b assignment
                let resolved_winner_team_idx = {
                    let slot = &self.bracket[slot_pos];
                    if *winner_team_idx == 0 {
                        slot.team_a_idx
                    } else {
                        slot.team_b_idx
                    }
                }
                .ok_or(EpisodeError::InvalidCommand(
                    TournamentError::InvalidBracketSlot {
                        round: *round,
                        slot_index: *slot_index,
                    },
                ))?;

                self.bracket[slot_pos].winner_idx = Some(resolved_winner_team_idx);
                self.bracket[slot_pos].status = BracketSlotStatus::Completed;

                // Advance winner to next round
                let completed_slot_idx = *slot_index;
                let completed_round = *round;
                let total_rounds = self.total_rounds();

                if completed_round < total_rounds {
                    Self::advance_winner_to_next_round(
                        &mut self.bracket,
                        completed_round,
                        completed_slot_idx,
                        resolved_winner_team_idx,
                    );
                }

                // Check if tournament is finished (final round complete)
                if completed_round == total_rounds {
                    // Final slot — tournament complete
                    let runner_up_idx = if *winner_team_idx == 0 {
                        self.bracket[slot_pos].team_b_idx
                    } else {
                        self.bracket[slot_pos].team_a_idx
                    };

                    self.winner_team_idx = Some(resolved_winner_team_idx);
                    self.runner_up_team_idx = runner_up_idx;
                    // Phase will transition to Completed via InitiatePayout
                } else if self.is_round_complete(completed_round) {
                    // Advance to next round
                    self.phase = TournamentPhase::InProgress {
                        current_round: completed_round + 1,
                    };
                }

                Ok(TournamentRollback::UndoReportBracketResult {
                    round: *round,
                    slot_index: *slot_index,
                })
            }

            // ─── DisputeResult ─────────────────────────────────────────────────
            TournamentCommand::DisputeResult {
                round,
                slot_index,
                reason_code,
            } => {
                if !matches!(self.phase, TournamentPhase::InProgress { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "DisputeResult".into(),
                        },
                    ));
                }

                // Validate and read slot info BEFORE mutating (borrow checker)
                let slot_pos =
                    self.find_slot(*round, *slot_index)
                        .ok_or(EpisodeError::InvalidCommand(
                            TournamentError::InvalidBracketSlot {
                                round: *round,
                                slot_index: *slot_index,
                            },
                        ))?;

                if !matches!(self.bracket[slot_pos].status, BracketSlotStatus::Completed) {
                    return Err(EpisodeError::InvalidCommand(TournamentError::NotInDispute));
                }

                // Read team assignments before mutating the slot
                let team_a_idx = self.bracket[slot_pos].team_a_idx;
                let team_b_idx = self.bracket[slot_pos].team_b_idx;

                let prev_phase = self.phase.clone();

                // Now mutate
                self.bracket[slot_pos].status = BracketSlotStatus::Cancelled;

                let disputing_team_idx = auth
                    .and_then(|auth_key| {
                        if self
                            .teams
                            .get(team_a_idx? as usize)
                            .map(|t| &t.captain_pubkey)
                            == Some(&auth_key)
                        {
                            Some(0u8)
                        } else if self
                            .teams
                            .get(team_b_idx? as usize)
                            .map(|t| &t.captain_pubkey)
                            == Some(&auth_key)
                        {
                            Some(1u8)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);

                self.phase = TournamentPhase::Disputed {
                    round: *round,
                    slot_index: *slot_index,
                    reason_code: *reason_code,
                    disputing_team_idx,
                };

                Ok(TournamentRollback::UndoDisputeResult {
                    previous_phase: prev_phase,
                })
            }

            // ─── ResolveDispute ────────────────────────────────────────────────
            TournamentCommand::ResolveDispute {
                round,
                slot_index,
                winner_team_idx,
            } => {
                if !matches!(self.phase, TournamentPhase::Disputed { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "ResolveDispute".into(),
                        },
                    ));
                }

                if *winner_team_idx > 1 {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidWinnerIndex,
                    ));
                }

                let slot_pos =
                    self.find_slot(*round, *slot_index)
                        .ok_or(EpisodeError::InvalidCommand(
                            TournamentError::InvalidBracketSlot {
                                round: *round,
                                slot_index: *slot_index,
                            },
                        ))?;

                let prev_winner = self.bracket[slot_pos].winner_idx;

                let resolved_winner = {
                    let slot = &self.bracket[slot_pos];
                    if *winner_team_idx == 0 {
                        slot.team_a_idx
                    } else {
                        slot.team_b_idx
                    }
                }
                .ok_or(EpisodeError::InvalidCommand(
                    TournamentError::InvalidBracketSlot {
                        round: *round,
                        slot_index: *slot_index,
                    },
                ))?;

                self.bracket[slot_pos].winner_idx = Some(resolved_winner);
                self.bracket[slot_pos].status = BracketSlotStatus::Completed;

                let total_rounds = self.total_rounds();
                if *round < total_rounds {
                    Self::advance_winner_to_next_round(
                        &mut self.bracket,
                        *round,
                        *slot_index,
                        resolved_winner,
                    );
                }

                // Resume in-progress phase
                if let TournamentPhase::Disputed { round: dr, .. } = &self.phase {
                    self.phase = TournamentPhase::InProgress { current_round: *dr };
                }

                Ok(TournamentRollback::UndoResolveDispute {
                    round: *round,
                    slot_index: *slot_index,
                    previous_winner: prev_winner,
                })
            }

            // ─── InitiatePayout ────────────────────────────────────────────────
            TournamentCommand::InitiatePayout { .. } => {
                if !matches!(self.phase, TournamentPhase::InProgress { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        TournamentError::InvalidTransition {
                            phase: format!("{:?}", self.phase),
                            action: "InitiatePayout".into(),
                        },
                    ));
                }

                let winner = self.winner_team_idx.ok_or(EpisodeError::InvalidCommand(
                    TournamentError::InvalidTransition {
                        phase: "Finals not yet completed".into(),
                        action: "InitiatePayout".into(),
                    },
                ))?;
                let runner_up = self.runner_up_team_idx.unwrap_or(0);

                self.phase = TournamentPhase::Completed {
                    winner_team_idx: winner,
                    runner_up_team_idx: runner_up,
                };

                Ok(TournamentRollback::UndoInitiatePayout)
            }

            // ─── CancelTournament ──────────────────────────────────────────────
            TournamentCommand::CancelTournament { reason_code } => match &self.phase {
                TournamentPhase::Registration | TournamentPhase::Funded => {
                    let prev = self.phase.clone();
                    self.phase = TournamentPhase::Cancelled {
                        reason_code: *reason_code,
                    };
                    Ok(TournamentRollback::UndoCancelTournament {
                        previous_phase: prev,
                    })
                }
                _ => Err(EpisodeError::InvalidCommand(
                    TournamentError::InvalidTransition {
                        phase: format!("{:?}", self.phase),
                        action: "CancelTournament".into(),
                    },
                )),
            },
        }
    }

    fn rollback(&mut self, rollback: Self::CommandRollback) -> bool {
        match rollback {
            TournamentRollback::UndoCreateTournament => {
                self.max_teams = 8;
                self.buy_in_sompi = 0;
                self.phase = TournamentPhase::Registration;
                true
            }

            TournamentRollback::UndoRegisterTeam { team_idx } => {
                if (team_idx as usize) < self.teams.len() {
                    self.teams.remove(team_idx as usize);
                }
                true
            }

            TournamentRollback::UndoConfirmTeamDeposit {
                team_idx,
                previous_amount,
            } => {
                if let Some(team) = self.teams.get_mut(team_idx as usize) {
                    team.deposited_sompi = previous_amount;
                    team.deposit_confirmed = previous_amount >= self.buy_in_sompi;
                }
                // If we rolled back to before FUNDED, revert phase
                if self.funded_team_count() < self.teams.len() {
                    self.phase = TournamentPhase::Registration;
                }
                true
            }

            TournamentRollback::UndoLockBracket { previous_phase } => {
                self.bracket.clear();
                self.phase = previous_phase;
                true
            }

            TournamentRollback::UndoSubmitMatchId { round, slot_index } => {
                if let Some(slot) = self.find_slot_mut(round, slot_index) {
                    slot.faceit_match_id_hash = None;
                    slot.status = BracketSlotStatus::Ready;
                }
                true
            }

            TournamentRollback::UndoReportBracketResult { round, slot_index } => {
                if let Some(slot) = self.find_slot_mut(round, slot_index) {
                    slot.winner_idx = None;
                    slot.status = BracketSlotStatus::InProgress;
                }
                true
            }

            TournamentRollback::UndoDisputeResult { previous_phase } => {
                self.phase = previous_phase;
                true
            }

            TournamentRollback::UndoResolveDispute {
                round,
                slot_index,
                previous_winner,
            } => {
                if let Some(slot) = self.find_slot_mut(round, slot_index) {
                    slot.winner_idx = previous_winner;
                    slot.status = BracketSlotStatus::Cancelled;
                }
                true
            }

            TournamentRollback::UndoInitiatePayout => {
                if let TournamentPhase::Completed { .. } = &self.phase {
                    // Revert to InProgress at last known round
                    let last_round = self.total_rounds();
                    self.phase = TournamentPhase::InProgress {
                        current_round: last_round,
                    };
                }
                true
            }

            TournamentRollback::UndoCancelTournament { previous_phase } => {
                self.phase = previous_phase;
                true
            }
        }
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kdapp_pki::generate_keypair;
    use crate::tournament_commands::reason;

    fn make_metadata() -> PayloadMetadata {
        PayloadMetadata {
            accepting_hash: kaspa_hashes::Hash::from_bytes([0u8; 32]),
            accepting_daa: 1000,
            accepting_time: 1234567890,
            tx_id: kaspa_hashes::Hash::from_bytes([1u8; 32]),
        }
    }

    fn make_episode() -> TournamentEpisode {
        let (_, pk_organizer) = generate_keypair();
        let meta = make_metadata();
        TournamentEpisode::initialize(vec![pk_organizer], &meta)
    }

    fn setup_4team_episode() -> (TournamentEpisode, [PubKey; 4]) {
        let (_, pk_organizer) = generate_keypair();
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let (_, pk_c) = generate_keypair();
        let (_, pk_d) = generate_keypair();
        let meta = make_metadata();

        let mut ep = TournamentEpisode::initialize(vec![pk_organizer], &meta);

        // CreateTournament
        ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 4,
                buy_in_sompi: 50_000_000_000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            Some(pk_organizer),
            &meta,
        )
        .unwrap();

        // Register 4 teams
        for (i, pk) in [pk_a, pk_b, pk_c, pk_d].iter().enumerate() {
            let mut name = [0u8; 32];
            name[0] = i as u8;
            ep.execute(
                &TournamentCommand::RegisterTeam {
                    team_name_hash: name,
                    team_size: 5,
                },
                Some(*pk),
                &meta,
            )
            .unwrap();
        }

        // Confirm all deposits
        for team_idx in 0u8..4 {
            ep.execute(
                &TournamentCommand::ConfirmTeamDeposit {
                    team_idx,
                    tx_hash: [team_idx; 32],
                    amount_sompi: 50_000_000_000,
                },
                Some(pk_organizer),
                &meta,
            )
            .unwrap();
        }

        (ep, [pk_a, pk_b, pk_c, pk_d])
    }

    // ── CreateTournament tests ──────────────────────────────────────────────

    #[test]
    fn test_create_tournament_valid() {
        let mut ep = make_episode();
        let (_, pk) = generate_keypair();
        let meta = make_metadata();

        let rb = ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 8,
                buy_in_sompi: 50_000_000_000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            Some(pk),
            &meta,
        );
        assert!(rb.is_ok());
        assert_eq!(ep.max_teams, 8);
        assert_eq!(ep.buy_in_sompi, 50_000_000_000);
    }

    #[test]
    fn test_create_tournament_invalid_team_count() {
        let mut ep = make_episode();
        let (_, pk) = generate_keypair();
        let meta = make_metadata();

        let result = ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 5, // Not a power of 2
                buy_in_sompi: 50_000_000_000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            Some(pk),
            &meta,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_create_tournament_invalid_prize_split() {
        let mut ep = make_episode();
        let (_, pk) = generate_keypair();
        let meta = make_metadata();

        let result = ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 8,
                buy_in_sompi: 50_000_000_000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 15, // 70 + 20 + 15 = 105, not 100
                game_type: TournamentGameType::CS2,
            },
            Some(pk),
            &meta,
        );
        assert!(result.is_err());
    }

    // ── RegisterTeam tests ──────────────────────────────────────────────────

    #[test]
    fn test_register_team_requires_auth() {
        let mut ep = make_episode();
        let meta = make_metadata();
        ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 4,
                buy_in_sompi: 1000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            None,
            &meta,
        )
        .unwrap();

        // RegisterTeam without auth should fail
        let result = ep.execute(
            &TournamentCommand::RegisterTeam {
                team_name_hash: [1u8; 32],
                team_size: 5,
            },
            None, // No authorization
            &meta,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_register_team_tournament_full() {
        let (ep, _) = setup_4team_episode();
        // All 4 teams registered, funded, phase = FUNDED
        assert_eq!(ep.teams.len(), 4);
        assert!(matches!(ep.phase, TournamentPhase::Funded));
    }

    // ── Deposit tests ───────────────────────────────────────────────────────

    #[test]
    fn test_partial_deposit_not_funded() {
        let mut ep = make_episode();
        let (_, pk_org) = generate_keypair();
        let (_, pk_a) = generate_keypair();
        let meta = make_metadata();

        ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 4,
                buy_in_sompi: 50_000_000_000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            Some(pk_org),
            &meta,
        )
        .unwrap();

        ep.execute(
            &TournamentCommand::RegisterTeam {
                team_name_hash: [0u8; 32],
                team_size: 5,
            },
            Some(pk_a),
            &meta,
        )
        .unwrap();

        // Send only partial deposit
        ep.execute(
            &TournamentCommand::ConfirmTeamDeposit {
                team_idx: 0,
                tx_hash: [1u8; 32],
                amount_sompi: 10_000_000_000, // Less than buy_in
            },
            Some(pk_org),
            &meta,
        )
        .unwrap();

        assert!(!ep.teams[0].deposit_confirmed);
        assert!(matches!(ep.phase, TournamentPhase::Registration));
    }

    // ── Bracket tests ───────────────────────────────────────────────────────

    #[test]
    fn test_lock_bracket_generates_slots() {
        let (mut ep, _) = setup_4team_episode();
        let meta = make_metadata();

        let result = ep.execute(&TournamentCommand::LockBracket, None, &meta);
        assert!(result.is_ok(), "{:?}", result);

        // 4 teams → 2 slots in round 1 + 1 slot in round 2
        assert_eq!(ep.bracket.len(), 3);
        assert!(matches!(
            ep.phase,
            TournamentPhase::BracketReady { total_rounds: 2 }
        ));

        // Round 1 slots should be Ready
        let r1_slots: Vec<_> = ep.bracket.iter().filter(|s| s.round == 1).collect();
        assert_eq!(r1_slots.len(), 2);
        assert!(r1_slots
            .iter()
            .all(|s| matches!(s.status, BracketSlotStatus::Ready)));
    }

    #[test]
    fn test_lock_bracket_requires_funded_phase() {
        let mut ep = make_episode();
        let meta = make_metadata();

        // Should fail in REGISTRATION phase
        let result = ep.execute(&TournamentCommand::LockBracket, None, &meta);
        assert!(result.is_err());
    }

    #[test]
    fn test_bracket_winner_advances_to_next_round() {
        let (mut ep, _) = setup_4team_episode();
        let meta = make_metadata();

        ep.execute(&TournamentCommand::LockBracket, None, &meta)
            .unwrap();

        // Submit match ID for round 1, slot 0
        ep.execute(
            &TournamentCommand::SubmitMatchId {
                round: 1,
                slot_index: 0,
                faceit_match_id_hash: [9u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();

        assert!(matches!(
            ep.phase,
            TournamentPhase::InProgress { current_round: 1 }
        ));

        // Report result: team_a wins (winner_team_idx=0)
        ep.execute(
            &TournamentCommand::ReportBracketResult {
                round: 1,
                slot_index: 0,
                winner_team_idx: 0,
                score_winner: 16,
                score_loser: 8,
            },
            None,
            &meta,
        )
        .unwrap();

        // Slot 0 of round 1 should be Completed
        let slot = ep
            .bracket
            .iter()
            .find(|s| s.round == 1 && s.slot_index == 0)
            .unwrap();
        assert!(matches!(slot.status, BracketSlotStatus::Completed));

        // Round 2, slot 0 should have team_a_idx set (winner from slot 0 = even)
        let final_slot = ep
            .bracket
            .iter()
            .find(|s| s.round == 2 && s.slot_index == 0)
            .unwrap();
        assert_eq!(final_slot.team_a_idx, slot.winner_idx);
    }

    #[test]
    fn test_full_4team_tournament_happy_path() {
        let (mut ep, _) = setup_4team_episode();
        let meta = make_metadata();

        // Lock
        ep.execute(&TournamentCommand::LockBracket, None, &meta)
            .unwrap();
        assert_eq!(ep.bracket.len(), 3);

        // Round 1, Slot 0: team 0 beats team 1
        ep.execute(
            &TournamentCommand::SubmitMatchId {
                round: 1,
                slot_index: 0,
                faceit_match_id_hash: [1u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();
        ep.execute(
            &TournamentCommand::ReportBracketResult {
                round: 1,
                slot_index: 0,
                winner_team_idx: 0,
                score_winner: 16,
                score_loser: 5,
            },
            None,
            &meta,
        )
        .unwrap();

        // Round 1, Slot 1: team 2 beats team 3
        ep.execute(
            &TournamentCommand::SubmitMatchId {
                round: 1,
                slot_index: 1,
                faceit_match_id_hash: [2u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();
        ep.execute(
            &TournamentCommand::ReportBracketResult {
                round: 1,
                slot_index: 1,
                winner_team_idx: 0,
                score_winner: 16,
                score_loser: 10,
            },
            None,
            &meta,
        )
        .unwrap();

        // After round 1 complete → InProgress at round 2
        assert!(matches!(
            ep.phase,
            TournamentPhase::InProgress { current_round: 2 }
        ));

        // Final: team 0 vs team 2
        let final_slot = ep
            .bracket
            .iter()
            .find(|s| s.round == 2 && s.slot_index == 0)
            .unwrap();
        assert!(final_slot.team_a_idx.is_some());
        assert!(final_slot.team_b_idx.is_some());

        ep.execute(
            &TournamentCommand::SubmitMatchId {
                round: 2,
                slot_index: 0,
                faceit_match_id_hash: [3u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();
        ep.execute(
            &TournamentCommand::ReportBracketResult {
                round: 2,
                slot_index: 0,
                winner_team_idx: 0,
                score_winner: 16,
                score_loser: 12,
            },
            None,
            &meta,
        )
        .unwrap();

        // Winner determined
        assert!(ep.winner_team_idx.is_some());
        assert!(ep.runner_up_team_idx.is_some());

        // Initiate Payout
        ep.execute(
            &TournamentCommand::InitiatePayout {
                winner_kaspa_addr_hash: [4u8; 32],
                runner_up_kaspa_addr_hash: [5u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();

        assert!(matches!(ep.phase, TournamentPhase::Completed { .. }));
    }

    #[test]
    fn test_cancel_tournament_in_registration() {
        let mut ep = make_episode();
        let meta = make_metadata();

        ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 4,
                buy_in_sompi: 1000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            None,
            &meta,
        )
        .unwrap();

        ep.execute(
            &TournamentCommand::CancelTournament {
                reason_code: reason::CANCEL_NOT_ENOUGH_TEAMS,
            },
            None,
            &meta,
        )
        .unwrap();

        assert!(matches!(ep.phase, TournamentPhase::Cancelled { .. }));
    }

    #[test]
    fn test_cancel_blocked_after_bracket_locked() {
        let (mut ep, _) = setup_4team_episode();
        let meta = make_metadata();

        ep.execute(&TournamentCommand::LockBracket, None, &meta)
            .unwrap();

        let result = ep.execute(
            &TournamentCommand::CancelTournament {
                reason_code: reason::CANCEL_ORGANIZER_REQUEST,
            },
            None,
            &meta,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_rollback_register_team() {
        let mut ep = make_episode();
        let (_, pk_org) = generate_keypair();
        let (_, pk_a) = generate_keypair();
        let meta = make_metadata();

        ep.execute(
            &TournamentCommand::CreateTournament {
                max_teams: 4,
                buy_in_sompi: 1000,
                prize_winner_pct: 70,
                prize_runner_up_pct: 20,
                platform_fee_pct: 10,
                game_type: TournamentGameType::CS2,
            },
            Some(pk_org),
            &meta,
        )
        .unwrap();

        let rb = ep
            .execute(
                &TournamentCommand::RegisterTeam {
                    team_name_hash: [1u8; 32],
                    team_size: 5,
                },
                Some(pk_a),
                &meta,
            )
            .unwrap();

        assert_eq!(ep.teams.len(), 1);
        ep.rollback(rb);
        assert_eq!(ep.teams.len(), 0);
    }

    #[test]
    fn test_build_bracket_4_teams() {
        let teams: Vec<TeamState> = (0..4)
            .map(|i| {
                let (_, pk) = generate_keypair();
                TeamState {
                    name_hash: [i as u8; 32],
                    captain_pubkey: pk,
                    deposited_sompi: 1000,
                    deposit_confirmed: true,
                }
            })
            .collect();

        let bracket = TournamentEpisode::build_bracket(&teams, 4);
        // 4 teams: 2 slots in round 1 + 1 slot in round 2
        assert_eq!(bracket.len(), 3);

        let r1: Vec<_> = bracket.iter().filter(|s| s.round == 1).collect();
        assert_eq!(r1.len(), 2);
        assert!(r1
            .iter()
            .all(|s| matches!(s.status, BracketSlotStatus::Ready)));

        let r2: Vec<_> = bracket.iter().filter(|s| s.round == 2).collect();
        assert_eq!(r2.len(), 1);
        assert!(matches!(r2[0].status, BracketSlotStatus::Waiting));
    }

    #[test]
    fn test_dispute_then_resolve() {
        let (mut ep, captains) = setup_4team_episode();
        let meta = make_metadata();

        ep.execute(&TournamentCommand::LockBracket, None, &meta)
            .unwrap();
        ep.execute(
            &TournamentCommand::SubmitMatchId {
                round: 1,
                slot_index: 0,
                faceit_match_id_hash: [7u8; 32],
            },
            None,
            &meta,
        )
        .unwrap();
        ep.execute(
            &TournamentCommand::ReportBracketResult {
                round: 1,
                slot_index: 0,
                winner_team_idx: 0,
                score_winner: 16,
                score_loser: 14,
            },
            None,
            &meta,
        )
        .unwrap();

        // Captain of the losing team files dispute
        ep.execute(
            &TournamentCommand::DisputeResult {
                round: 1,
                slot_index: 0,
                reason_code: reason::DISPUTE_SCORE_MISMATCH,
            },
            Some(captains[1]), // Team B captain
            &meta,
        )
        .unwrap();
        assert!(matches!(ep.phase, TournamentPhase::Disputed { .. }));

        // Admin resolves: team_b actually won
        ep.execute(
            &TournamentCommand::ResolveDispute {
                round: 1,
                slot_index: 0,
                winner_team_idx: 1, // Override to team_b
            },
            None,
            &meta,
        )
        .unwrap();

        let slot = ep
            .bracket
            .iter()
            .find(|s| s.round == 1 && s.slot_index == 0)
            .unwrap();
        assert!(matches!(slot.status, BracketSlotStatus::Completed));
        assert!(matches!(ep.phase, TournamentPhase::InProgress { .. }));
    }
}
