use log;
use stderrlog::{self, ColorChoice, Timestamp};

pub fn init_log(verbose: bool) {
    stderrlog::new()
	.color(ColorChoice::Auto)
	.timestamp(Timestamp::Second)
	.show_module_names(true)
	.verbosity(if verbose { log::Level::Debug } else { log::Level::Error })
	.init()
	.unwrap();
}
