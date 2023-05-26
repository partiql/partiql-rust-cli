use clap::{ArgEnum, Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[cfg(feature = "visualize")]
    /// Dump the AST for a query
    Ast {
        #[clap(short = 'T', long = "format", value_enum)]
        format: Format,

        /// Query to parse
        #[clap(value_parser)]
        query: String,
    },
    /// Evaluate the query with the optional global environment
    Eval {
        /// Query to evaluate
        #[clap(value_parser)]
        query: String,
        /// Output format
        #[clap(short = 'f', long = "format", value_enum, default_value_t=OutputFormat::Partiql)]
        output: OutputFormat,
        /// Optional environment file (.env or .ion)
        #[clap(short = 'E', long = "environment")]
        environment: Option<String>,
    },
    /// Interactive REPL (Read Eval Print Loop) shell
    Repl {
        /// Optional environment file (.env or .ion)
        #[clap(short = 'E', long = "environment")]
        environment: Option<String>,
    },
}

#[derive(ArgEnum, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutputFormat {
    /// PartiQL
    Partiql,
    /// Ion Text, one top-level item per line
    IonLines,
    /// Ion Text, pretty printed
    IonPretty,
}

#[derive(ArgEnum, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    /// JSON
    Json,
    /// Graphviz dot
    Dot,
    /// Graphviz svg output
    Svg,
    /// Graphviz svg rendered to png
    Png,
    /// Display rendered output
    Display,
}
