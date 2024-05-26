mod log_macros;

use clap::Parser;
use core::fmt::Arguments;
use easy_error::{self, ResultExt};
use serde::Deserialize;
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};
use svg::{node::element::*, node::*, Document};

pub trait LineChartLog {
    fn output(self: &Self, args: Arguments);
    fn warning(self: &Self, args: Arguments);
    fn error(self: &Self, args: Arguments);
}

pub struct LineChartTool<'a> {
    log: &'a dyn LineChartLog,
}

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// The JSON5 input file
    #[clap(value_name = "INPUT_FILE")]
    input_file: Option<PathBuf>,

    /// The SVG output file
    #[clap(value_name = "OUTPUT_FILE")]
    output_file: Option<PathBuf>,
}

impl Cli {
    fn get_output(&self) -> Result<Box<dyn Write>, Box<dyn Error>> {
        match self.output_file {
            Some(ref path) => File::create(path)
                .context(format!(
                    "Unable to create file '{}'",
                    path.to_string_lossy()
                ))
                .map(|f| Box::new(f) as Box<dyn Write>)
                .map_err(|e| Box::new(e) as Box<dyn Error>),
            None => Ok(Box::new(io::stdout())),
        }
    }

    fn get_input(&self) -> Result<Box<dyn Read>, Box<dyn Error>> {
        match self.input_file {
            Some(ref path) => File::open(path)
                .context(format!("Unable to open file '{}'", path.to_string_lossy()))
                .map(|f| Box::new(f) as Box<dyn Read>)
                .map_err(|e| Box::new(e) as Box<dyn Error>),
            None => Ok(Box::new(io::stdin())),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChartData {
    pub title: String,
    pub units: String,
    pub data: Vec<ItemData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ItemData {
    pub key: String,
    pub value: f64,
}

#[derive(Debug)]
struct Gutter {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[derive(Debug)]
struct RenderData {
    title: String,
    units: String,
    plot_width: f64,
    y_axis_height: f64,
    y_axis_range: (f64, f64),
    y_axis_interval: f64,
    gutter: Gutter,
    styles: Vec<String>,
    tuples: Vec<(String, f64)>,
}

impl<'a> LineChartTool<'a> {
    pub fn new(log: &'a dyn LineChartLog) -> LineChartTool {
        LineChartTool { log }
    }

    pub fn run(
        self: &mut Self,
        args: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Result<(), Box<dyn Error>> {
        let cli = match Cli::try_parse_from(args) {
            Ok(m) => m,
            Err(err) => {
                output!(self.log, "{}", err.to_string());
                return Ok(());
            }
        };

        let chart_data = Self::read_chart_file(cli.get_input()?)?;
        let render_data = self.process_chart_data(&chart_data)?;
        let document = self.render_chart(&render_data)?;

        Self::write_svg_file(cli.get_output()?, &document)?;

        Ok(())
    }

    fn read_chart_file(mut reader: Box<dyn Read>) -> Result<ChartData, Box<dyn Error>> {
        let mut content = String::new();

        reader.read_to_string(&mut content)?;

        let chart_data: ChartData = json5::from_str(&content)?;

        Ok(chart_data)
    }

    fn write_svg_file(writer: Box<dyn Write>, document: &Document) -> Result<(), Box<dyn Error>> {
        svg::write(writer, document)?;

        Ok(())
    }

    fn process_chart_data(self: &Self, cd: &ChartData) -> Result<RenderData, Box<dyn Error>> {
        let mut tuples = vec![];
        let mut y_axis_range: (f64, f64) = (f64::MAX, f64::MIN);

        for item_data in cd.data.iter() {
            let value = item_data.value;

            if value < y_axis_range.0 {
                y_axis_range.0 = value;
            } else if value > y_axis_range.1 {
                y_axis_range.1 = value;
            }

            tuples.push((item_data.key.to_owned(), item_data.value));
        }

        let plot_width = 50.0;
        let y_axis_height = 400.0;
        let y_axis_num_intervals = 20;
        let y_axis_interval = (10.0_f64).powf(((y_axis_range.1 - y_axis_range.0).log10()).ceil())
            / (y_axis_num_intervals as f64);

        y_axis_range = (
            f64::floor(y_axis_range.0 / y_axis_interval) * y_axis_interval,
            f64::ceil(y_axis_range.1 / y_axis_interval) * y_axis_interval,
        );

        let gutter = Gutter {
            top: 40.0,
            bottom: 80.0,
            left: 80.0,
            right: 80.0,
        };

        Ok(RenderData {
            title: cd.title.to_owned(),
            units: cd.units.to_owned(),
            plot_width,
            y_axis_height,
            y_axis_range,
            y_axis_interval,
            gutter,
            styles: vec![
                ".line{fill:none;stroke:rgb(0,0,200);stroke-width:2;}".to_owned(),
                ".axis{fill:none;stroke:rgb(0,0,0);stroke-width:1;}".to_owned(),
                ".labels{fill:rgb(0,0,0);font-size:10;font-family:Arial}".to_owned(),
                ".y-labels{text-anchor:end;}".to_owned(),
                ".title{font-family:Arial;font-size:12;text-anchor:middle;}".to_owned(),
            ],
            tuples,
        })
    }

    fn render_chart(self: &Self, rd: &RenderData) -> Result<Document, Box<dyn Error>> {
        let width = rd.gutter.left + ((rd.tuples.len() as f64) * rd.plot_width) + rd.gutter.right;
        let height = rd.gutter.top + rd.gutter.bottom + rd.y_axis_height;
        let y_range = ((rd.y_axis_range.1 - rd.y_axis_range.0) / rd.y_axis_interval) as usize;
        let y_scale = rd.y_axis_height / (rd.y_axis_range.1 - rd.y_axis_range.0);
        let scale =
            |n: &f64| -> f64 { height - rd.gutter.bottom - (n - rd.y_axis_range.0) * y_scale };
        let mut document = Document::new()
            .set("xmlns", "http://www.w3.org/2000/svg")
            .set("width", width)
            .set("height", height)
            .set("viewBox", format!("0 0 {} {}", width, height))
            .set("style", "background-color: white;");
        let style = element::Style::new(rd.styles.join("\n"));
        let axis = element::Polyline::new().set("class", "axis").set(
            "points",
            vec![
                (rd.gutter.left, rd.gutter.top),
                (rd.gutter.left, rd.gutter.top + rd.y_axis_height),
                (width - rd.gutter.right, rd.gutter.top + rd.y_axis_height),
            ],
        );
        let mut x_axis_labels = element::Group::new().set("class", "labels");

        for i in 0..rd.tuples.len() {
            x_axis_labels.append(element::Text::new(format!("{}", rd.tuples[i].0)).set(
                "transform",
                format!(
                    "translate({},{}) rotate(45)",
                    rd.gutter.left + (i as f64 * rd.plot_width) + rd.plot_width / 2.0,
                    height - rd.gutter.bottom + 15.0
                ),
            ));
        }

        let mut y_axis_labels = element::Group::new().set("class", "labels y-labels");

        for i in 0..=y_range {
            let n = i as f64 * rd.y_axis_interval;

            y_axis_labels.append(
                element::Text::new(format!("{}", n + rd.y_axis_range.0)).set(
                    "transform",
                    format!(
                        "translate({},{})",
                        rd.gutter.left - 10.0,
                        height - rd.gutter.bottom - f64::floor(n * y_scale) + 5.0
                    ),
                ),
            );
        }

        let line = element::Path::new().set("class", "line").set(
            "d",
            path::Data::from(
                rd.tuples
                    .iter()
                    .enumerate()
                    .map(|t| {
                        let x = rd.gutter.left + (t.0 as f64) * rd.plot_width + rd.plot_width / 2.0;
                        let y = scale(&(*t.1).1);

                        if t.0 == 0 {
                            path::Command::Move(
                                path::Position::Absolute,
                                path::Parameters::from((x, y)),
                            )
                        } else {
                            path::Command::Line(
                                path::Position::Absolute,
                                path::Parameters::from((x, y)),
                            )
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
        );

        let title = element::Text::new(format!("{} ({})", &rd.title, &rd.units))
            .set("class", "title")
            .set("x", width / 2.0)
            .set("y", rd.gutter.top / 2.0);

        document.append(style);
        document.append(axis);
        document.append(x_axis_labels);
        document.append(y_axis_labels);
        document.append(line);
        document.append(title);

        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_test() {
        struct TestLogger;

        impl TestLogger {
            fn new() -> TestLogger {
                TestLogger {}
            }
        }

        impl LineChartLog for TestLogger {
            fn output(self: &Self, _args: Arguments) {}
            fn warning(self: &Self, _args: Arguments) {}
            fn error(self: &Self, _args: Arguments) {}
        }

        let logger = TestLogger::new();
        let mut tool = LineChartTool::new(&logger);
        let args: Vec<std::ffi::OsString> = vec!["".into(), "--help".into()];

        tool.run(args).unwrap();
    }
}
