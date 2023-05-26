#![deny(rustdoc::broken_intra_doc_links)]

use clap::Parser;
use ion_rs::IonWriter;
use partiql_cli::args::{Commands, OutputFormat};
use partiql_cli::evaluate::{evaluate, get_bindings};
use partiql_cli::pretty::PrettyPrint;
use partiql_cli::{args, repl};
use partiql_extension_ion::encode::{IonEncoderBuilder, IonEncoderConfig};
use partiql_extension_ion::Encoding;
use partiql_value::Value;
use std::io::Write;

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
            let evaluated = evaluate(query, bindings)?.result;
            match output {
                OutputFormat::Partiql => {
                    partiql_pretty_print(evaluated);
                }
                OutputFormat::IonLines => {
                    let mut out = std::io::stdout().lock();
                    let mut writer = ion_rs::TextWriterBuilder::lines()
                        .build(&mut out)
                        .expect("ion writer");
                    ion_encode(&mut writer, evaluated);
                }
                OutputFormat::IonPretty => {
                    let mut out = std::io::stdout().lock();
                    let mut writer = ion_rs::TextWriterBuilder::pretty()
                        .build(&mut out)
                        .expect("ion writer");
                    ion_encode(&mut writer, evaluated);
                }
            }

            Ok(())
        }
    }
}

fn partiql_pretty_print(value: Value) {
    let mut pretty = String::new();
    value
        .pretty(&mut pretty)
        .expect("Error when trying to pretty print result");
    println!("{pretty}");
}

fn ion_encode<'a, W, I>(writer: &'a mut I, value: Value)
where
    W: Write + 'a,
    I: IonWriter<Output = W> + 'a,
{
    let mut encoder = IonEncoderBuilder::new(IonEncoderConfig::default().with_mode(Encoding::Ion))
        .build(writer)
        .expect("ion encoder build");

    value
        .iter()
        .for_each(|v| encoder.write_value(v).expect("ion encoder write"));
}
