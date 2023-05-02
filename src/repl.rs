#![deny(rustdoc::broken_intra_doc_links)]

use rustyline::completion::Completer;
use rustyline::config::Configurer;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};

use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{ColorMode, Context, Helper};
use std::borrow::Cow;

use std::fs::OpenOptions;
use std::panic;
use std::panic::AssertUnwindSafe;

use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};
use syntect::util::as_24_bit_terminal_escaped;

use miette::{IntoDiagnostic, Report};
use owo_colors::OwoColorize;
use partiql_eval::env::basic::MapBindings;
use partiql_eval::eval::Evaluated;

use partiql_value::Value;

use crate::error::CLIErrors;
use crate::evaluate::get_bindings;
use crate::pretty::PrettyPrint;

static ION_SYNTAX: &str = include_str!("ion.sublime-syntax");
static PARTIQL_SYNTAX: &str = include_str!("partiql.sublime-syntax");

struct PartiqlHelperConfig {
    dark_theme: bool,
}

impl PartiqlHelperConfig {
    pub fn infer() -> Self {
        const TERM_TIMEOUT_MILLIS: u64 = 20;
        let timeout = std::time::Duration::from_millis(TERM_TIMEOUT_MILLIS);
        let theme = termbg::theme(timeout);
        let dark_theme = match theme {
            Ok(termbg::Theme::Light) => false,
            Ok(termbg::Theme::Dark) => true,
            _ => true,
        };
        PartiqlHelperConfig { dark_theme }
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
        let theme = if self.config.dark_theme {
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

        let source_len = source.len();
        match source_len {
            0 => return Ok(ValidationResult::Valid(None)),
            _ => match &source[source_len - 1..source_len] {
                ";" => {
                    // TODO: there's a bug here where hitting enter from the middle of a query
                    //  containing a semi-colon will repeat the query
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
                // TODO: when better error-handling ergonomics are added to partiql-lang-rust
                //  evaluation such as Result types rather than panics. Replace following code.
                //  Tracking issue: https://github.com/partiql/partiql-lang-rust/issues/349
                let evaluated = panic::catch_unwind(AssertUnwindSafe(|| {
                    let lowered = partiql_logical_planner::lower(&parsed);
                    let mut plan = partiql_eval::plan::EvaluatorPlanner.compile(&lowered);
                    plan.execute_mut(globals)
                }));
                match evaluated {
                    Ok(Ok(Evaluated { result: v })) => {
                        let mut pretty_v = String::new();
                        v.pretty(&mut pretty_v).expect("TODO: panic message");
                        println!("\n==='\n{pretty_v}");
                        Ok(ValidationResult::Valid(None))
                    }
                    Ok(Err(e)) => {
                        let err = Report::new(CLIErrors::from_eval_error(e, source));
                        Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
                    }
                    Err(panic_eval_error) => Ok(ValidationResult::Invalid(Some(format!(
                        "\n\n{panic_eval_error:?}"
                    )))),
                }
            }
            Err(e) => {
                let err = Report::new(CLIErrors::from_parser_error(e));
                Ok(ValidationResult::Invalid(Some(format!("\n\n{err:?}"))))
            }
        }
    }
}

pub fn repl(environment: &Option<String>) -> miette::Result<()> {
    let bindings = get_bindings(environment)?;
    let mut rl = rustyline::Editor::<PartiqlHelper>::new().into_diagnostic()?;
    rl.set_color_mode(ColorMode::Forced);
    rl.set_helper(Some(
        PartiqlHelper::new(bindings, PartiqlHelperConfig::infer()).unwrap(),
    ));
    let expanded = shellexpand::tilde("~/partiql_cli.history").to_string();
    let history_path = Path::new(&expanded);
    OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(history_path)
        .expect("history file create if not exists");
    rl.load_history(history_path).expect("history load");

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
                rl.append_history(history_path).expect("append history");
                break;
            }
        }
    }

    Ok(())
}
