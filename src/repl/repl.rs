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
use tracing::field::DisplayValue;
use tracing::{error, info, span, trace, Level};
use uuid::Uuid;

use crate::error::CLIErrors;
use crate::evaluate::{get_bindings, Compiler};
use crate::formatting::print_value;
use crate::repl::config::{repl_config, ReplConfig, ION_SYNTAX, PARTIQL_SYNTAX};

struct PartiqlHelper {
    config: ReplConfig,
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    globals: MapBindings<Value>,
}

impl PartiqlHelper {
    pub fn new(globals: MapBindings<Value>, config: ReplConfig) -> Result<Self, ()> {
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
        let request_id = Uuid::new_v4();
        let mut source = ctx.input();

        // TODO remove this command parsing hack do something better
        let flag_ast = source.starts_with("\\ast");
        if flag_ast {
            source = source.trim_start_matches("\\ast");
        }

        let flag_plan = source.starts_with("\\plan");
        if flag_plan {
            source = source.trim_start_matches("\\plan");
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
        span!(
            Level::INFO,
            "validate",
            %request_id,
        )
        .in_scope(|| {
            info!(query = &source, "Validating");

            info!("Parsing");
            let compiler = Compiler::default();
            let result = compiler.parse(source);
            let globals = self.globals.clone();
            match result {
                Ok(parsed) => {
                    #[cfg(feature = "visualize")]
                    if flag_ast {
                        use crate::visualize::render::display;
                        display(&parsed.ast);
                    }

                    println!();
                    std::io::stdout().flush();
                    std::io::stderr().flush();
                    let spinner = ProgressBar::new_spinner();
                    spinner.enable_steady_tick(Duration::from_millis(100));
                    spinner.set_message("Query running");

                    info!("Planning");
                    let plan = compiler.plan(&parsed);
                    let plan = match plan {
                        Ok(plan) => plan,
                        Err(e) => {
                            error!("Planning failed due to {e}");
                            let err = Report::new(e);
                            return Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))));
                        }
                    };
                    #[cfg(feature = "visualize")]
                    if flag_plan {
                        use crate::visualize::render::display;
                        display(&plan);
                    }

                    info!("Compiling");
                    let eval = compiler.compile(&parsed, &plan);
                    let eval = match eval {
                        Ok(eval) => eval,
                        Err(e) => {
                            error!("Compiling failed due to {e}");
                            let err = Report::new(e);
                            return Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))));
                        }
                    };

                    info!("Evaluating");
                    let start = SystemTime::now();
                    let evaluated = compiler.evaluate(&parsed, eval, globals);
                    let end = SystemTime::now();
                    let duration = end.duration_since(start).unwrap();
                    let duration = HumanDuration(duration);

                    match evaluated {
                        Ok(Evaluated { result: v }) => {
                            info!("Evaluation finished in {duration}");
                            spinner.finish_with_message(format!("Query finished in {duration}"));
                            println!("\n==='\n");

                            info!(?output, "Printing");
                            print_value(&output, &v);
                            println!();
                            std::io::stdout().flush();
                            std::io::stderr().flush();
                            Ok(ValidationResult::Valid(None))
                        }
                        Err(e) => {
                            error!("Evaluation failed after {duration} due to {e}");
                            spinner.finish_with_message(format!("Query failed after {duration}"));
                            let err = Report::new(e);
                            Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
                        }
                    }
                }
                Err(e) => {
                    error!("Parse failed due to {e:?}");
                    let err = Report::new(CLIErrors::from(e));
                    Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
                }
            }
        })
    }
}

pub fn repl(environment: &Option<String>) -> miette::Result<()> {
    let config = repl_config();
    let history_path = config.history_path.clone();

    let mut log_dir = config.cache_dir.clone();
    log_dir.push("logs");
    let file_appender = tracing_appender::rolling::daily(&log_dir, "partiql-cli.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt().with_writer(non_blocking).init();

    let bindings = get_bindings(environment)?;

    let mut rl = rustyline::Editor::<PartiqlHelper>::new().into_diagnostic()?;
    rl.set_color_mode(ColorMode::Forced);
    rl.set_helper(Some(PartiqlHelper::new(bindings, config).unwrap()));
    rl.load_history(&history_path).expect("history load");

    println!("===============================");
    println!("PartiQL REPL");
    println!("CTRL-D on an empty line to quit");
    println!("===============================");

    span!(Level::INFO, "repl",).in_scope(|| loop {
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
    });

    Ok(())
}
