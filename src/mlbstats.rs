use std::collections::HashMap;
use std::process::Command;

use log;
use serde_json as json;

use crate::config::Config;

#[derive(Debug)]
pub struct Game {
    pub away_name: String,
    pub home_name: String,
    pub game_id: String,
    pub status: String,
}

#[derive(Debug)]
pub struct BatterStats {
    pub name: String,
    pub hand: String,
    pub plate_appearances: i32,
    pub bases_on_balls: i32,
    pub hits: i32,
    pub doubles: i32,
    pub triples: i32,
    pub homeruns: i32,
    pub strikeouts: i32,
}

#[derive(Debug)]
pub struct PitcherStats {
    pub name: String,
    pub hand: String,
    pub batters_faced: i32,
    pub bases_on_balls: i32,
    pub hits: i32,
    pub doubles: i32,
    pub triples: i32,
    pub homeruns: i32,
    pub strikeouts: i32,
}

// #[derive(Debug)]
// pub struct Player {
//     pub player_id: String,
//     pub player_stats: Option<PlayerStats>,
// }

#[derive(Debug)]
pub struct Team {
    pub name: String,
    pub starting_pitcher: PitcherStats,
    pub batters: Vec<BatterStats>,
}

pub struct StatsApi<'a> {
    url: String,
    params: HashMap<&'a str, &'a str>,
}

impl<'a> StatsApi<'a> {
    pub fn schedule(date: &'a str) -> StatsApi<'a> {
	StatsApi {
	    url: "v1/schedule".to_string(),
	    params: HashMap::from([
		("sportId", "1"),
		("date", date)
	    ])
	}
    }
 
    pub fn game(game_id: &str) -> StatsApi<'a> {
	StatsApi {
	    url: format!("v1.1/game/{game_id}/feed/live"),
	    params: HashMap::new(),
	}
    }

    pub fn player(player_id: &str) -> StatsApi<'a> {
	StatsApi {
	    url: format!("v1/people/{player_id}"),
	    params: HashMap::new()
	}
    }
	    
 
    pub fn param(mut self, k: &'a str, v: &'a str) -> Self {
	self.params.insert(k, v);
	self
    }

    pub fn build_url(&mut self) -> Result<String, String> {
	let mut url = format!("https://statsapi.mlb.com/api/{}", self.url);

	for (i, (k, v)) in self.params.iter().enumerate() {
	    url = format!(
		"{}{}{}={}", 
		url,
		if i == 0 { "?" } else { "&" },
		k,
		v
	    )
	}

	Ok(url)
    }

    pub fn json(mut self) -> Result<json::Value, String> {
	let url = self.build_url()?;
	log::debug!(target: "StatsApi.json", "url={:?}", url);
	let data = match Command::new("curl").arg(&url).output() {
            Ok(out) => {
		if out.status.success() {
                    Ok(String::from_utf8(out.stdout).unwrap())
		} else {
                    Err(String::from_utf8(out.stderr).unwrap())
		}
            }
            Err(err) => Err(format!("Error in StatsApi.json: {}", err)),
	};
	json::from_str(&data?).map_err(|err| format!("{:?}", err))
    }
}




// TODO: decouple from config
pub fn schedule(cfg: &Config) -> Result<Vec<Game>, String> {
    let data = StatsApi::schedule(&cfg.date).json()?;

    let mut games: Vec<Game> = Vec::new();

    if let json::Value::Array(games_data) = &data["dates"] {
        if games_data.is_empty() {
	    println!("[WARNING] No games found on this date.");
	    return Ok(Vec::new());
        } else if games_data.len() > 1 {
            return Err(String::from("Ambiguous data for this date!"));
        }

        if let json::Value::Array(games_data) = &games_data[0]["games"] {
            for obj in games_data {
                games.push(Game {
                    away_name: value_to_string(&obj["teams"]["away"]["team"]["name"]),
                    home_name: value_to_string(&obj["teams"]["home"]["team"]["name"]),
                    game_id: value_to_string(&obj["gamePk"]),
                    status: value_to_string(&obj["status"]["detailedState"]),
                });
            }
        }
    };
    // println!("{:#?}", games);
    Ok(games)
}

pub fn teams(cfg: &Config, game_id: &String) -> Result<(Option<Team>, Option<Team>), String> {
    let data = StatsApi::game(game_id)
	.param("fields", "gameData,liveData,boxscore,teams,players,id,abbreviation")
	.json()?;

    // Heuristic to check if the lineup exists
    let away = if !data["liveData"]["boxscore"]["teams"]["away"]["pitchers"][0].is_null() {
	Some(Team {
	    name: data["gameData"]["teams"]["away"]["abbreviation"].as_str().unwrap().to_string(),
	    starting_pitcher: pitcher_stats(cfg, &data["liveData"]["boxscore"]["teams"]["away"]["pitchers"][0])?,
	    batters: batter_stats(cfg, &data["liveData"]["boxscore"]["teams"]["away"]["battingOrder"])?
	})
    } else {
	None
    };

    let home = if !data["liveData"]["boxscore"]["teams"]["home"]["pitchers"][0].is_null() {
	Some(Team {
	    name: data["gameData"]["teams"]["home"]["abbreviation"].as_str().unwrap().to_string(),
	    starting_pitcher: pitcher_stats(cfg, &data["liveData"]["boxscore"]["teams"]["home"]["pitchers"][0])?,
	    batters: batter_stats(cfg, &data["liveData"]["boxscore"]["teams"]["home"]["battingOrder"])?
	})
    } else {
	None
    };

    Ok((away, home))
}


fn batter_stats(cfg: &Config, data: &json::Value) -> Result<Vec<BatterStats>, String> {
    let mut bats: Vec<BatterStats> = Vec::new();

    if let json::Value::Array(data) = data {
	for obj in data {
            let player_id = value_to_string(obj);
            let raw_stats = fetch_batter_stats(cfg, &player_id)?;

            let obj = &raw_stats["people"][0]["stats"][0]["splits"][0]["stat"];

	    let name = raw_stats["people"][0]["initLastName"]
		.as_str()
		.unwrap()
		.to_string();

	    let hand = format!(
		"{}HB",
		raw_stats["people"][0]["batSide"]["code"].as_str().unwrap()
	    );

            bats.push(BatterStats {
		name,
		hand,
		plate_appearances: value_to_int(&obj["plateAppearances"])?,
		bases_on_balls: value_to_int(&obj["baseOnBalls"])?,
		hits: value_to_int(&obj["hits"])?,
		doubles: value_to_int(&obj["doubles"])?,
		triples: value_to_int(&obj["triples"])?,
		homeruns: value_to_int(&obj["homeRuns"])?,
		strikeouts: value_to_int(&obj["strikeOuts"])?,
            })
	}
    }
    Ok(bats)
}

fn pitcher_stats(cfg: &Config, data: &json::Value) -> Result<PitcherStats, String> {
    let player_id = value_to_string(data);
    let raw_stats = fetch_pitcher_stats(cfg, &player_id)?;
    let obj = &raw_stats["people"][0]["stats"][0]["splits"][0]["stat"];

    let name = raw_stats["people"][0]["initLastName"]
	.as_str()
	.unwrap()
	.to_string();
    
    let hand = format!(
	"{}HP",
	raw_stats["people"][0]["batSide"]["code"].as_str().unwrap()
    );

    Ok(PitcherStats {
	name,
	hand,
	batters_faced: value_to_int(&obj["battersFaced"])?,
	bases_on_balls: value_to_int(&obj["baseOnBalls"])?,
	hits: value_to_int(&obj["hits"])?,
	doubles: value_to_int(&obj["doubles"])?,
	triples: value_to_int(&obj["triples"])?,
	homeruns: value_to_int(&obj["homeRuns"])?,
	strikeouts: value_to_int(&obj["strikeOuts"])?,
    })
}
							  
fn fetch_batter_stats(_cfg: &Config, player_id: &String) -> Result<json::Value, String> {
    StatsApi::player(player_id)
	.param("hydrate", "stats(group=hitting,type=career,sportId=1),currentTeam")
	.json()
}

fn fetch_pitcher_stats(_cfg: &Config, player_id: &String) -> Result<json::Value, String> {
    StatsApi::player(player_id)
	.param("hydrate", "stats(group=pitching,type=career,sportId=1),currentTeam")
	.json()
}


fn value_to_int(value: &json::Value) -> Result<i32, String> {
    value_to_string(value)
        .parse()
        .map_err(|e| format!("Error while parsing \"{}\" to i32: {:?}", value, e))
}

fn value_to_string(value: &json::Value) -> String {
    value.to_string().replace('\"', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_value_to_string() {
        let val = json::Value::String(String::from("Toronto Blue Jays"));
        assert_eq!("Toronto Blue Jays", value_to_string(&val));
        let val = json::json!(12345);
        assert_eq!("12345", value_to_string(&val))
    }
}
