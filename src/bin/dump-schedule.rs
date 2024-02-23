use std::env;
use std::error::Error;

use chrono::NaiveDate;

use mlb_oracle::config::Config;
use mlb_oracle::mlbstats;

fn get_datestr() -> Result<String, Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    let datestr = if args.len() >= 2 {
        NaiveDate::parse_from_str(&args[1], "%F")?
            .format("%F")
            .to_string()
    } else {
        chrono::offset::Local::now().format("%F").to_string()
    };
    Ok(datestr)
}

fn main() -> Result<(), Box<dyn Error>> {
    let datestr = get_datestr()?;
    let cfg = Config {
        date: datestr,
        verbose: true,
    };
    let sched = mlbstats::schedule(&cfg)?;
    println!("{:#?}", sched);
    Ok(())
}
