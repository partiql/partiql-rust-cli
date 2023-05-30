#![deny(rustdoc::broken_intra_doc_links)]

use rustyline::completion::Completer;
use rustyline::config::Configurer;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};

use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{ColorMode, Context, Helper};
use std::borrow::Cow;

use std::io::Write;

use clap::ArgEnum;
use config::Config;
use indicatif::{HumanDuration, ProgressBar};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};
use syntect::util::as_24_bit_terminal_escaped;

use miette::{IntoDiagnostic, Report};
use owo_colors::OwoColorize;
use partiql_eval::env::basic::MapBindings;
use partiql_eval::eval::Evaluated;

use crate::args::OutputFormat;
use partiql_value::Value;

use crate::error::CLIErrors;
use crate::evaluate::{evaluate_parsed, get_bindings};
use crate::formatting::print_value;
use crate::repl::config::{repl_config, ReplConfig, ION_SYNTAX, PARTIQL_SYNTAX};

struct PartiqlHelperConfig {
    config: Config,
    history_path: PathBuf,
}

impl PartiqlHelperConfig {
    pub fn new(config: Config, history_path: PathBuf) -> Self {
        PartiqlHelperConfig {
            config: config,
            history_path: history_path,
        }
    }
}

impl From<ReplConfig> for PartiqlHelperConfig {
    fn from(
        ReplConfig {
            config,
            history_path,
            ..
        }: ReplConfig,
    ) -> Self {
        PartiqlHelperConfig {
            config,
            history_path,
        }
    }
}

struct PartiqlHelper {
    config: PartiqlHelperConfig,
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    globals: MapBindings<Value>,
}

impl PartiqlHelper {
    pub fn new(globals: MapBindings<Value>, config: PartiqlHelperConfig) -> Result<Self, ()> {
        let ion_def = SyntaxDefinition::load_from_str(ION_SYNTAX, false, Some("ion")).unwrap();
        let partiql_def =
            SyntaxDefinition::load_from_str(PARTIQL_SYNTAX, false, Some("partiql")).unwrap();
        let mut builder = SyntaxSetBuilder::new();
        builder.add(ion_def);
        builder.add(partiql_def);

        let syntaxes = builder.build();

        let _ps = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        Ok(PartiqlHelper {
            config,
            syntaxes,
            themes,
            globals,
        })
    }
}

impl Helper for PartiqlHelper {}

impl Completer for PartiqlHelper {
    type Candidate = String;
}
impl Hinter for PartiqlHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        let hinter = HistoryHinter {};
        hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for PartiqlHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let syntax = self
            .syntaxes
            .find_syntax_by_extension("partiql")
            .unwrap()
            .clone();
        let theme: String = self
            .config
            .config
            .get("repl.theme")
            .expect("config repl.theme");
        let dark_theme = theme == "dark";
        let theme = if dark_theme {
            &self.themes.themes["Solarized (dark)"]
        } else {
            &self.themes.themes["Solarized (light)"]
        };
        let mut highlighter = HighlightLines::new(&syntax, theme);

        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, &self.syntaxes).unwrap();
        (as_24_bit_terminal_escaped(&ranges[..], true) + "\x1b[0m").into()
    }
    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        let _ = (line, pos);
        true
    }
}

impl Validator for PartiqlHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        // TODO remove this command parsing hack do something better
        let mut source = ctx.input();
        let flag_display = source.starts_with("\\ast");
        if flag_display {
            source = &source[4..];
        }

        let config_of: Result<String, _> = self.config.config.get("repl.output_format");
        let mut output = if let Ok(Ok(fmt)) = config_of.map(|of| OutputFormat::from_str(&of, true))
        {
            fmt
        } else {
            OutputFormat::Partiql
        };

        if source.starts_with("\\table") {
            output = OutputFormat::Table;
            source = &source[6..];
        }
        if source.starts_with("\\ion-lines") {
            output = OutputFormat::IonLines;
            source = &source[10..];
        }
        if source.starts_with("\\ion-pretty") {
            output = OutputFormat::IonPretty;
            source = &source[11..];
        }
        if source.starts_with("\\partiql") {
            output = OutputFormat::Partiql;
            source = &source[8..];
        }

        let source_len = source.len();
        match source_len {
            0 => return Ok(ValidationResult::Valid(None)),
            _ => match &source[source_len - 1..source_len] {
                ";" => {
                    // TODO: there's a bug here where hitting enter from the middle of a query
                    //  containing a semi-colon will repeat the query
                    //  https://github.com/partiql/partiql-rust-cli/issues/10
                    source = &source[..source_len - 1];
                }
                "\n" => {}
                _ => return Ok(ValidationResult::Incomplete),
            },
        }

        let parser = partiql_parser::Parser::default();
        let result = parser.parse(source);
        let globals = self.globals.clone();
        match result {
            Ok(parsed) => {
                #[cfg(feature = "visualize")]
                if flag_display {
                    use crate::visualize::render::display;
                    display(&parsed.ast);
                }

                println!();
                std::io::stdout().flush();
                std::io::stderr().flush();
                let spinner = ProgressBar::new_spinner();
                spinner.enable_steady_tick(Duration::from_millis(100));
                spinner.set_message("Query running");

                let start = SystemTime::now();
                let evaluated = evaluate_parsed(&parsed, globals);
                let end = SystemTime::now();
                let duration = end.duration_since(start).unwrap();
                let duration = HumanDuration(duration);

                match evaluated {
                    Ok(Evaluated { result: v }) => {
                        spinner.finish_with_message(format!("Query finished in {duration}"));
                        println!("\n==='\n");
                        print_value(&output, &v);
                        println!();
                        std::io::stdout().flush();
                        std::io::stderr().flush();
                        Ok(ValidationResult::Valid(None))
                    }
                    Err(e) => {
                        spinner.finish_with_message(format!("Query failed after {duration}"));
                        let err = Report::new(e);
                        Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
                    }
                }
            }
            Err(e) => {
                let err = Report::new(CLIErrors::from(e));
                Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
            }
        }
    }
}

pub fn repl(environment: &Option<String>) -> miette::Result<()> {
    let bindings = get_bindings(environment)?;

    let config = PartiqlHelperConfig::from(repl_config());
    let history_path = config.history_path.clone();

    let mut rl = rustyline::Editor::<PartiqlHelper>::new().into_diagnostic()?;
    rl.set_color_mode(ColorMode::Forced);
    rl.set_helper(Some(PartiqlHelper::new(bindings, config).unwrap()));
    rl.load_history(&history_path).expect("history load");

    println!("===============================");
    println!("PartiQL REPL");
    println!("CTRL-D on an empty line to quit");
    println!("===============================");

    loop {
        let readline = rl.readline("PartiQL> ");
        match readline {
            Ok(line) => {
                println!("\n---\n{}", "OK!".green());
                rl.add_history_entry(line);
            }
            Err(_) => {
                println!("Exiting...");
                rl.append_history(&history_path).expect("append history");
                break;
            }
        }
    }

    Ok(())
}
