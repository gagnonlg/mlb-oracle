pub mod config;
mod mlbstats;
mod simba;

use std::io::{self, Write};

use crate::{
    config::Config,
    mlbstats::Game,
};

pub fn run(cfg: Config) -> Result<(), String> {
    if cfg.verbose {
        eprintln!("[VERBOSE] cfg.date={:?}", cfg.date);
        eprintln!("[VERBOSE] cfg.verbose={:?}", cfg.verbose);
    }
    for game in mlbstats::schedule(&cfg)? {
        oracle(&cfg, &game)?
    }
    Ok(())
}

#[derive(Clone)]
enum TTYColor {
    Black = 30,
    Red = 31,
    Green = 32,
    Yellow = 33,
    Blue = 34,
    Cyan = 36,
    White = 37,
}

fn colormap(winprob: f64) -> TTYColor {
    if winprob >= 0.75 {
	TTYColor::Cyan
    } else if winprob >= 0.55 {
	TTYColor::Green
    } else if winprob >= 0.45 {
	TTYColor::Yellow
    } else if winprob >= 0.25 {
	TTYColor::Red
    } else {
	TTYColor::White
    }
} 

struct GameLine<'a> {
    game: &'a Game,
    status: Option<String>,
    color: Option<TTYColor>,
}

impl<'a> GameLine<'a> {
    fn new(game: &'a Game) -> GameLine<'a> {
        GameLine {
            game: &game,
            status: None,
            color: None,
        }
    }

    fn fetching(&mut self) {
	self.status = Some("FETCHING DATA...".to_string());
	self.color = Some(TTYColor::Black);
    }

    fn postponed(&mut self) {
        self.status = Some("POSTPONED".to_string());
        self.color = Some(TTYColor::Blue)
    }

    fn frontend_error(&mut self) {
	self.status = Some("FRONTEND ERROR".to_string());
	self.color = Some(TTYColor::Red);
    }

    fn missing_lineups(&mut self) {
	self.status = Some("MISSING LINEUPS".to_string());
	self.color = Some(TTYColor::Yellow);
    }

    fn missing_lineup_away(&mut self) {
	self.status = Some("MISSING LINEUP A".to_string());
	self.color = Some(TTYColor::Yellow);
    }

    fn missing_lineup_home(&mut self) {
	self.status = Some("MISSING LINEUP H".to_string());
	self.color = Some(TTYColor::Yellow);
    }

    fn predicting(&mut self) {
	self.status = Some("PREDICTING...".to_string());
	self.color = Some(TTYColor::Black);
    }

    fn backend_error(&mut self) {
	self.status = Some("BACKEND ERROR".to_string());
	self.color = Some(TTYColor::Red);
    }

    fn prediction(&mut self, hwp: Option<f64>) {
	match hwp {
	    None => {
		self.status = Some("NO PREDICTION".to_string());
		self.color = Some(TTYColor::Red);
	    }
	    Some(hwp) => {
		let nboxes_per_team = 10;
		let awp = 1.0 - hwp;
		let nfull_h = (hwp * nboxes_per_team as f64).round() as i32;
		let nfull_a = (awp * nboxes_per_team as f64).round() as i32;
		let mut line = String::new();
		for _ in 0..(nboxes_per_team - nfull_a) {
                    line.push_str("□");
		}
		let mut subline = String::new();
		for _ in 0..nfull_a {
                    subline.push_str("■");
		}
		line.push_str(&colored_msg(colormap(awp), &subline));

		// Separator
		line.push_str(" ");

		// Home
		let mut subline = String::new();
		for _ in 0..nfull_h {
                    subline.push_str("■")
		}
			
		line.push_str(&colored_msg(colormap(hwp), &subline));
		for _ in 0..(nboxes_per_team - nfull_h) {
                    line.push_str("□");
		}

		self.color = None;
		self.status = Some(line);
	    }
	}
    }

    fn update(&self) {
        print!("\x1B[2k\r");

        let status = self.status.clone().unwrap_or("UNKNOWN".to_string());
        let status = if let Some(col) = &self.color {
            colored_msg(col.clone(), &bold(&format!("{:^21}", status)))
        } else {
            status
        };

	let line = format!("{:>25} {} {}", self.game.away_name, status, self.game.home_name);
	print!("{}", line);
	let _ = io::stdout().flush();
    }

    fn finalize(&self) {
	self.update();
	println!("");
    }
	    
    
}

fn bold(msg: &String) -> String {
    format!("\x1B[1m{msg}\x1B[0m")
}

fn colored_msg(color: TTYColor, msg: &String) -> String {
    format!("\x1B[{}m{}\x1B[0m", color as isize, msg)
}

fn oracle(cfg: &Config, game: &Game) -> Result<(), String> {
    let mut gline = GameLine::new(&game);

    if game.status == "Postponed" {
        gline.postponed();
        gline.finalize();
        return Ok(());
    }

    gline.fetching();
    gline.update();

    let result = mlbstats::teams(&cfg, &game.game_id);
    if let Err(e) = result {
	gline.frontend_error();
	gline.finalize();
	return Err(e);
    }
    let (away, home) = result.unwrap();

    if away.is_none() && home.is_none() {
	gline.missing_lineups();
	gline.finalize();
	return Ok(());
    }
    
    if away.is_none() {
	gline.missing_lineup_away();
	gline.finalize();
	return Ok(());
    }

    if home.is_none() {
	gline.missing_lineup_home();
	gline.finalize();
	return Ok(());
    }

    gline.predicting();
    gline.update();

    let sim_result = simba::predict(&away.unwrap(), &home.unwrap());
    if let Err(e) = sim_result {
	gline.backend_error();
	gline.finalize();
	return Err(e);
    }

    gline.prediction(sim_result.unwrap().home_win_probability);
    gline.finalize();

    Ok(())
}

