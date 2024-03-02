// TODO:
// - Split off the GameState from the SimState such that it can
//   be used by analyzers as well
// - Using this, start validating against retrosheet data
// - Properly design the Play type
// - Organize this file

use std::iter;

use itertools::Itertools;

use crate::mlbstats::{BatterStats, PitcherStats, Team};

pub struct SimbaConfig {
    pub n_iter: usize,
}

impl SimbaConfig {
    pub fn run<'a>(&self, gamestate: &GameState) -> Result<SimResult, String> {
        let scores = iter::repeat_with(|| {
            SimbaState::new(gamestate.clone())
                .into_iter()
                .fold_ok(Score::default(), |s, p| s.add(p.team, p.runs))
        })
        .take(self.n_iter)
        .collect::<Result<Vec<_>, _>>()?;

        let scores = scores.iter().counts();

        let norm = scores.iter().fold(0, |acc, (_, n)| acc + *n);

        let wins = scores
            .iter()
            .fold(0, |acc, (s, n)| acc + if s.home > s.away { *n } else { 0 });

        let hwp = wins as f64 / norm as f64;

        Ok(SimResult {
            home_win_probability: Some(hwp),
        })
    }
}

impl Default for SimbaConfig {
    fn default() -> SimbaConfig {
        SimbaConfig { n_iter: 1000 }
    }
}

#[derive(Clone)]
pub struct GameState<'a> {
    pub bases: [bool; 3],
    pub score: Score,
    pub teams: [LiveTeam<'a>; 2],
    pub team_idx: i32,
    pub inning: i32,
    pub outs: i32,
    pub live: bool,
}

impl<'a> GameState<'a> {
    pub fn new(visteam: &'a Team, hometeam: &'a Team) -> GameState<'a> {
        GameState {
            bases: [false, false, false],
            score: Score::default(),
            teams: [LiveTeam::from(visteam), LiveTeam::from(hometeam)],
            team_idx: 0,
            inning: 1,
            outs: 0,
            live: true,
        }
    }

    fn transition(&mut self, play: &Play) -> i32 {
	let (advs, outs) = match play.outcome {
            Outcome::Walk => (1, 0),
            Outcome::Single => (1, 0),
            Outcome::Double => (2, 0),
            Outcome::Triple => (3, 0),
            Outcome::HomeRun => (4, 0),
            Outcome::StrikeOut => (0, 1),
            Outcome::TagOut => (0, 1),
            Outcome::FlyOut => (0, 1),
        };

	// Step in the batting order
        self.teams[self.team_idx as usize].advance();

        // Count runs and advance field state
        let mut runs = 0;
        for i in 0..advs {
            if self.bases[2] {
                runs += 1;
            }
            self.bases[2] = self.bases[1];
            self.bases[1] = self.bases[0];
            self.bases[0] = i == 0;
        }

        // Credit runs to offense
        if self.team_idx == 0 {
            self.score.away += runs;
        } else {
            self.score.home += runs;
        }

        // Charge outs to offense
        self.outs += outs;

        let vis_ab = self.team_idx == 0;
        let vis_losing = self.score.home > self.score.away;

	// Game is over if either:
        // A. inning >= 9, vis at bat,  vis losing, 3 outs
        // C. inning >= 9, home at bat, vis winning, 3 outs
        // B. inning >= 9, home at bat, vis losing

        if self.inning >= 9 {
            if vis_ab && vis_losing && self.outs == 3 {
                self.live = false;
            } else if !vis_ab && !vis_losing && self.outs == 3 {
                self.live = false;
            } else if !vis_ab && vis_losing {
                self.live = false
            }
        }

        // Else, Inning is over if 3 outs
        if self.live && self.outs == 3 {
            self.bases[0] = false;
            self.bases[1] = false;
            self.bases[2] = false;
            if self.team_idx == 1 {
                self.inning += 1;
            }
            self.team_idx = 1 - self.team_idx;
            self.outs = 0;
        }

	runs
    }
}


struct Play {
    team: i32,
    runs: i32,
    outcome: Outcome,
}

struct SimbaState<'a> {
    gamestate: GameState<'a>
}

impl<'a> SimbaState<'a> {
    pub fn new(gamestate: GameState<'a>) -> SimbaState<'a> {
	SimbaState { gamestate }
    }    

    fn eval(&self) -> Option<Play> {
        if !self.gamestate.live {
            return None;
        }

        let off_idx = self.gamestate.team_idx as usize;
        let def_idx = 1 - self.gamestate.team_idx as usize;
        let batter = self.gamestate.teams[off_idx].batter();
        let pitcher = self.gamestate.teams[def_idx].pitcher();

        Some(Play {
            team: self.gamestate.team_idx,
            runs: 0, // Filled later
            outcome: OutcomeProbs::compute(pitcher, batter).sample(),
        })
    }

    fn transition(&mut self) -> Result<Option<Play>, String> {
        if let Some(play) = self.eval() {
	    let runs = self.gamestate.transition(&play);
            Ok(Some(Play { runs, ..play }))
        } else {
            Ok(None)
        }
    }
}

struct SimbaIter<'a>(Option<SimbaState<'a>>);

impl<'a> Iterator for SimbaIter<'a> {
    type Item = Result<Play, String>;
    fn next(&mut self) -> Option<Self::Item> {
        let SimbaIter(o) = self;
        let mut s = o.take()?;
        match s.transition() {
            Ok(Some(p)) => {
                o.replace(s);
                Some(Ok(p))
            }
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

impl<'a> IntoIterator for SimbaState<'a> {
    type Item = Result<Play, String>;
    type IntoIter = SimbaIter<'a>;
    fn into_iter(self) -> SimbaIter<'a> {
        SimbaIter(Some(self))
    }
}

pub struct SimResult {
    pub home_win_probability: Option<f64>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Score {
    pub away: i32,
    pub home: i32,
}

impl Score {
    fn default() -> Score {
        Score { away: 0, home: 0 }
    }
    fn add(mut self, idx: i32, runs: i32) -> Self {
        if idx == 0 {
            self.away += runs;
        } else if idx == 1 {
            self.home += runs;
        }
        self
    }
}

#[derive(Clone)]
pub struct LiveTeam<'a> {
    pub team: &'a Team,
    pub current_batter: usize,
}

impl<'a> LiveTeam<'a> {
    fn from(team: &'a Team) -> LiveTeam<'a> {
        LiveTeam {
            team,
            current_batter: 0,
        }
    }

    fn advance(&mut self) {
        self.current_batter = if self.current_batter < 8 {
            self.current_batter + 1
        } else {
            0
        };
    }

    pub fn pitcher(&self) -> &PitcherStats {
        &self.team.starting_pitcher
    }

    pub fn batter(&self) -> &BatterStats {
        &self.team.batters[self.current_batter]
    }
}

enum Outcome {
    Walk,
    Single,
    Double,
    Triple,
    HomeRun,
    StrikeOut,
    TagOut,
    FlyOut,
}

struct OutcomeProbs {
    prob_walk: f64,
    prob_single: f64,
    prob_double: f64,
    prob_triple: f64,
    prob_homerun: f64,
    prob_strikeout: f64,
    prob_tagout: f64,
    prob_flyout: f64,
}

fn average(x: f64, y: f64) -> f64 {
    if x == 0.0 || y == 0.0 {
        (x + y) / 2.0
    } else {
        (x * y).sqrt()
    }
}

fn div(x: i32, y: i32) -> f64 {
    (x as f64) / (y as f64)
}

impl OutcomeProbs {
    fn compute(pitcher: &PitcherStats, batter: &BatterStats) -> OutcomeProbs {
        let prob_walk_p = div(pitcher.bases_on_balls, pitcher.batters_faced);
        let prob_walk_b = div(batter.bases_on_balls, batter.plate_appearances);
        let prob_walk = average(prob_walk_p, prob_walk_b);

        let prob_strikeout_p = div(pitcher.strikeouts, pitcher.batters_faced);
        let prob_strikeout_b = div(batter.strikeouts, batter.plate_appearances);
        let prob_strikeout = average(prob_strikeout_p, prob_strikeout_b);

        let prob_hit_p = div(pitcher.hits, pitcher.batters_faced);
        let prob_hit_b = div(batter.hits, batter.plate_appearances);
        let prob_hit = average(prob_hit_p, prob_hit_b);

        let prob_bip_out = 1.0 - prob_hit - prob_walk - prob_strikeout;

        let prob_2b_p = div(pitcher.doubles, pitcher.hits);
        let prob_2b_b = div(batter.doubles, batter.hits);
        let prob_2b = average(prob_2b_p, prob_2b_b);
        let prob_3b_p = div(pitcher.triples, pitcher.hits);
        let prob_3b_b = div(batter.triples, batter.hits);
        let prob_3b = average(prob_3b_p, prob_3b_b);
        let prob_hr_p = div(pitcher.homeruns, pitcher.hits);
        let prob_hr_b = div(batter.homeruns, batter.hits);
        let prob_hr = average(prob_hr_p, prob_hr_b);
        let prob_1b = 1.0 - prob_2b - prob_3b - prob_hr;

        let prob_flyout = 0.5 * prob_bip_out;
        let prob_tagout = 0.5 * prob_bip_out;

        let prob_single = prob_hit * prob_1b;
        let prob_double = prob_hit * prob_2b;
        let prob_triple = prob_hit * prob_3b;
        let prob_homerun = prob_hit * prob_hr;

        OutcomeProbs {
            prob_walk,
            prob_single,
            prob_double,
            prob_triple,
            prob_homerun,
            prob_strikeout,
            prob_tagout,
            prob_flyout,
        }
    }

    fn sample(&self) -> Outcome {
        loop {
            if rand::random::<f64>() < self.prob_walk {
                return Outcome::Walk;
            }
            if rand::random::<f64>() < self.prob_single {
                return Outcome::Single;
            }
            if rand::random::<f64>() < self.prob_double {
                return Outcome::Double;
            }
            if rand::random::<f64>() < self.prob_triple {
                return Outcome::Triple;
            }
            if rand::random::<f64>() < self.prob_homerun {
                return Outcome::HomeRun;
            }
            if rand::random::<f64>() < self.prob_strikeout {
                return Outcome::StrikeOut;
            }
            if rand::random::<f64>() < self.prob_tagout {
                return Outcome::TagOut;
            }
            if rand::random::<f64>() < self.prob_flyout {
                return Outcome::FlyOut;
            }
        }
    }
}
