#![deny(rustdoc::broken_intra_doc_links)]

use clap::Parser;
use partiql_cli::args::Commands;
use partiql_cli::evaluate::{get_bindings, Compiler};
use partiql_cli::formatting::print_value;
use partiql_cli::{args, repl};

fn main() -> miette::Result<()> {
    let args = args::Args::parse();

    match &args.command {
        Commands::Repl { environment } => repl::repl(environment),

        #[cfg(feature = "visualize")]
        Commands::Ast { format, query } => {
            use partiql_cli::args::Format;
            use partiql_cli::visualize::render::{display, to_dot, to_json, to_png, to_svg};
            use std::io::Write;

            let compiler = Compiler::default();
            let parsed = compiler.parse(&query)?;
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
        #[cfg(feature = "visualize")]
        Commands::Plan { format, query } => {
            use partiql_cli::args::Format;
            use partiql_cli::visualize::render::{display, to_dot, to_json, to_png, to_svg};
            use std::io::Write;

            let compiler = Compiler::default();
            let parsed = compiler.parse(&query)?;
            let plan = compiler.plan(&parsed)?;
            match format {
                Format::Json => println!("{}", to_json(&plan)),
                Format::Dot => println!("{}", to_dot(&plan)),
                Format::Svg => println!("{}", to_svg(&plan)),
                Format::Png => {
                    std::io::stdout().write(&to_png(&plan)).expect("png write");
                }
                Format::Display => display(&plan),
            }

            Ok(())
        }
        Commands::Eval {
            query,
            output,
            environment,
        } => {
            let bindings = get_bindings(environment)?;
            let compiler = Compiler::default();
            let parsed = compiler.parse(&query)?;
            let plan = compiler.plan(&parsed)?;
            let eval = compiler.compile(&parsed, &plan)?;
            let evaluated = compiler.evaluate(&parsed, eval, bindings)?.result;
            print_value(output, &evaluated);
            Ok(())
        }
    }
}
