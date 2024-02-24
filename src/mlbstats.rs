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
    pub starting_pitcher: PitcherStats,
    pub batters: Vec<BatterStats>,
}

// TODO: decouple from config
pub fn schedule(cfg: &Config) -> Result<Vec<Game>, String> {
    let base_url = "https://statsapi.mlb.com/api";
    let ver = "v1";
    let url = format!("{}/{}/schedule?sportId=1&date={}", base_url, ver, cfg.date);

    let data = curl_json(&url)?;
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
    let base_url = "https://statsapi.mlb.com/api";
    let ver = "v1.1";
    let url = format!(
        "{base_url}/{ver}/game/{game_id}/feed/live?fields=liveData,boxscore,teams,players,id"
    );

    let data = curl_json(&url)?;
    // println!("{:#?}", &data);
    // return Err("baaaz".to_string());

    // let mut away_batters: Vec<BatterStats> = Vec::new();
    // let mut home_batters: Vec<BatterStats> = Vec::new();

    // Heuristic to check if the lineup exists
    let away = if !data["liveData"]["boxscore"]["teams"]["away"]["pitchers"][0].is_null() {
	Some(Team {
	    starting_pitcher: pitcher_stats(cfg, &data["liveData"]["boxscore"]["teams"]["away"]["pitchers"][0])?,
	    batters: batter_stats(cfg, &data["liveData"]["boxscore"]["teams"]["away"]["battingOrder"])?
	})
    } else {
	None
    };

    let home = if !data["liveData"]["boxscore"]["teams"]["home"]["pitchers"][0].is_null() {
	Some(Team {
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
            bats.push(BatterStats {
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

    Ok(PitcherStats {
	batters_faced: value_to_int(&obj["battersFaced"])?,
	bases_on_balls: value_to_int(&obj["baseOnBalls"])?,
	hits: value_to_int(&obj["hits"])?,
	doubles: value_to_int(&obj["doubles"])?,
	triples: value_to_int(&obj["triples"])?,
	homeruns: value_to_int(&obj["homeRuns"])?,
	strikeouts: value_to_int(&obj["strikeOuts"])?,
    })
}
							  
fn fetch_batter_stats(cfg: &Config, player_id: &String) -> Result<json::Value, String> {
    let base_url = "https://statsapi.mlb.com/api";
    let ver = "v1";
    let url = format!(
        "{base_url}/{ver}/people/{player_id}?hydrate=stats(group=hitting,type=career,sportId=1),currentTeam"
    );

    let data = curl_json(&url)?;

    // println!("{:#?}", &data);

    Ok(data)
}

fn fetch_pitcher_stats(cfg: &Config, player_id: &String) -> Result<json::Value, String> {
    let base_url = "https://statsapi.mlb.com/api";
    let ver = "v1";
    let url = format!(
        "{base_url}/{ver}/people/{player_id}?hydrate=stats(group=pitching,type=career,sportId=1),currentTeam"
    );

    let data = curl_json(&url)?;

    Ok(data)
}

fn call_curl(url: &str) -> Result<String, String> {
    log::debug!(target: "mlbstats::call_curl", "url=\"{}\"", url);
    match Command::new("curl").args([url]).output() {
        Ok(out) => {
            if out.status.success() {
                Ok(String::from_utf8(out.stdout).unwrap())
            } else {
                Err(format!(
                    "Error in call_curl for url \"{}\": curl exited with status={}, with stderr={}",
                    url,
                    out.status,
                    String::from_utf8(out.stderr).unwrap()
                ))
            }
        }
        Err(err) => Err(format!("Error in curl_json: {}", err)),
    }
}

fn curl_json(url: &str) -> Result<json::Value, String> {
    json::from_str(&call_curl(url)?).map_err(|err| format!("{:?}", err))
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
