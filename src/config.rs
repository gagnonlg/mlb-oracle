use chrono;
use chrono::NaiveDate;
use clap::Parser;

pub struct Config {
    pub date: String,
    pub verbose: bool,
}

impl Config {
    pub fn get() -> Result<Config, String> {
	Cli::parse().to_config()
    }
}

#[derive(Debug, Parser)]
#[command(name = "mlb-oracle")]
#[command(version = "0.1.0")]
#[command(about = "MLB daily predictions!", long_about = None)]
struct Cli {
    /// Make predictions for this date (Default: today)
    #[arg(value_name = "YYYY-MM-DD")]
    date: Option<String>,
    #[arg(short, long)]
    verbose: bool,
}


fn parse_date(datestr: &str) -> Result<String, String> {
    match NaiveDate::parse_from_str(datestr, "%F") {
        Ok(v) => Ok(v.to_string()),
        Err(_) => Err(format!("Invalid date: {}", datestr)),
    }
}

impl Cli {
    fn to_config(self) -> Result<Config, String> {
        let date = match self.date {
            Some(s) => parse_date(&s)?,
            None => chrono::offset::Local::now().format("%m/%d/%Y").to_string(),
        };
        Ok(Config {
            date: date,
            verbose: self.verbose,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cfg_date_invalid() {
        let v = parse_date("foobarbaz");
        assert!(v.is_err());
        let v = parse_date("2024");
        assert!(v.is_err());
        let v = parse_date("2024-01");
        assert!(v.is_err());
        let v = parse_date("2024-01-0");
        assert!(v.is_err());
        let v = parse_date("2024-01-01FOOBAR");
        assert!(v.is_err());
    }

    #[test]
    fn cfg_date_valid() {
        let v = parse_date("2024-01-01").unwrap();
        assert_eq!(v, "2024-01-01");
    }
}
