use colored::Colorize;
use core::fmt::Arguments;
use line_chart::{error, LineChartLog, LineChartTool};

struct LineChartLogger;

impl LineChartLogger {
    fn new() -> LineChartLogger {
        LineChartLogger {}
    }
}

impl LineChartLog for LineChartLogger {
    fn output(self: &Self, args: Arguments) {
        println!("{}", args);
    }
    fn warning(self: &Self, args: Arguments) {
        eprintln!("{}", format!("warning: {}", args).yellow());
    }
    fn error(self: &Self, args: Arguments) {
        eprintln!("{}", format!("error: {}", args).red());
    }
}

fn main() {
    let logger = LineChartLogger::new();

    if let Err(error) = LineChartTool::new(&logger).run(std::env::args_os()) {
        error!(logger, "{}", error);
        std::process::exit(1);
    }
}
