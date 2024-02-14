use std::process;

use mlb_oracle;
use mlb_oracle::config::Config;

fn main() {
    if let Err(msg) = Config::get().and_then(mlb_oracle::run) {
        eprintln!("[FATAL] {}", msg);
        process::exit(1);
    }
}
