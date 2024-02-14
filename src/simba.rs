use rand;
use std::collections::HashMap;

use crate::mlbstats::{BatterStats, PitcherStats, Team};

pub struct SimResult {
    pub home_win_probability: Option<f64>,
}

pub fn predict(away: &Team, home: &Team) -> Result<SimResult, String> {
    // Err("simba::predict: Not implemented".to_string())
    // Ok(SimResult{home_win_probability: None})
    // Ok(SimResult{home_win_probability: Some(1.0)})

    // let n_iter = 100000;
    let n_iter = 1000;
    let mut score_map = HashMap::new();
    for _ in 0..n_iter {
        let score = play_single_game(away, home);
        score_map.entry(score).and_modify(|e| *e += 1).or_insert(1);
    }
    let norm = score_map.iter().fold(0.0, |acc, (_, p)| acc + *p as f64);
    let hwp = score_map.iter().fold(0.0, |acc, (sco, p)| {
	let mut my_acc = acc;
	if sco.home > sco.away {
	    let p = *p as f64;
	    my_acc += p / norm;
	}
	my_acc
    });

    if hwp < 0.0 || hwp > 1.0 {
	Err(format!("Invalid win probability: {hwp}!"))
    } else {
	Ok(SimResult{home_win_probability: Some(hwp)})
    }
}

///////////////////////////////////////////////////////////////////////////

#[derive(PartialEq, Eq, Hash)]
struct Score {
    away: usize,
    home: usize,
}

impl Score {
    fn default() -> Score {
        Score { away: 0, home: 0 }
    }
}

struct LiveTeam<'a> {
    team: &'a Team,
    current_batter: usize,
}

impl<'a> LiveTeam<'a> {
    fn from(team: &'a Team) -> LiveTeam<'a> {
        LiveTeam {
            team: &team,
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

    fn pitcher(&self) -> &PitcherStats {
        &self.team.starting_pitcher
    }

    fn batter(&mut self) -> &BatterStats {
        let bat = &self.team.batters[self.current_batter];
        self.advance();
        bat
    }
}

fn play_single_game(away: &Team, home: &Team) -> Score {
    let mut score = Score::default();
    let mut away = LiveTeam::from(away);
    let mut home = LiveTeam::from(home);

    let mut inning = 1;
    while inning <= 9 || score.away == score.home {
        score.away += play_half_inning(&mut away, &home, inning > 9);
        if inning <= 9 || score.home <= score.away {
            score.home += play_half_inning(&mut home, &away, inning > 9);
        }
        inning += 1;
    }
    score
}

struct Field {
    runs: usize,
    first_base: bool,
    second_base: bool,
    third_base: bool,
}

impl Field {
    fn new(extras: bool) -> Field {
        Field {
            runs: 0,
            first_base: false,
            second_base: extras,
            third_base: false,
        }
    }

    fn advance(&mut self, n: usize) {
        for i in 0..n {
            if self.third_base {
                self.runs += 1;
            }
            self.third_base = self.second_base;
            self.second_base = self.first_base;
            self.first_base = i == 0;
        }
    }

    fn random_out(&mut self) {
        if !(self.first_base || self.second_base || self.third_base) {
            return;
        }

        loop {
            let rnd = rand::random::<f64>() * 3.0;
            let base = if rnd < 1.0 {
                &mut self.first_base
            } else if rnd < 2.0 {
                &mut self.second_base
            } else {
                &mut self.third_base
            };
            if *base {
                *base = false;
                break;
            }
        }
    }
}

fn play_half_inning(off: &mut LiveTeam, def: &LiveTeam, extras: bool) -> usize {
    let mut field = Field::new(extras);
    let mut outs = 0;
    while outs < 3 {
        let pitcher = def.pitcher();
        let batter = off.batter();
        outs += at_bat(&mut field, pitcher, batter);
    }
    field.runs
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

fn at_bat(field: &mut Field, pitcher: &PitcherStats, batter: &BatterStats) -> usize {
    match OutcomeProbs::compute(pitcher, batter).sample() {
        Outcome::Walk | Outcome::Single => {
            field.advance(1);
            0
        }
        Outcome::Double => {
            field.advance(2);
            0
        }
        Outcome::Triple => {
            field.advance(3);
            0
        }
        Outcome::HomeRun => {
            field.advance(4);
            0
        }
        Outcome::TagOut => {
            field.advance(1);
            field.random_out();
            1
        }
        _ => 1,
    }
}
