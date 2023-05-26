#![deny(rustdoc::broken_intra_doc_links)]

use clap::Parser;
use partiql_cli::args::Commands;
use partiql_cli::evaluate::{evaluate_parsed, get_bindings};
use partiql_cli::formatting::print_value;
use partiql_cli::{args, repl};

fn main() -> miette::Result<()> {
    let args = args::Args::parse();

    match &args.command {
        Commands::Repl { environment } => repl::repl(environment),

        #[cfg(feature = "visualize")]
        Commands::Ast { format, query } => {
            use partiql_cli::args::Format;
            use partiql_cli::parse::parse;
            use partiql_cli::visualize::render::{display, to_dot, to_json, to_png, to_svg};
            use std::io::Write;

            let parsed = parse(&query)?;
            match format {
                Format::Json => println!("{}", to_json(&parsed.ast)),
                Format::Dot => println!("{}", to_dot(&parsed.ast)),
                Format::Svg => println!("{}", to_svg(&parsed.ast)),
                Format::Png => {
                    std::io::stdout()
                        .write(&to_png(&parsed.ast))
                        .expect("png write");
                }
                Format::Display => display(&parsed.ast),
            }

            Ok(())
        }
        Commands::Eval {
            query,
            output,
            environment,
        } => {
            let bindings = get_bindings(environment)?;
            let parser = partiql_parser::Parser::default();
            let parsed = parser.parse(query).expect("parse");
            let evaluated = evaluate_parsed(&parsed, bindings)?.result;
            print_value(output, &evaluated);
            Ok(())
        }
    }
}
