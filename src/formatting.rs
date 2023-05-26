use crate::args::OutputFormat;
use crate::pretty::PrettyPrint;

use comfy_table::{Cell, Color, Table};
use ion_rs::IonWriter;
use partiql_extension_ion::encode::{IonEncoderBuilder, IonEncoderConfig};
use partiql_extension_ion::Encoding;
use partiql_value::Value;
use std::collections::HashMap;
use std::io::Write;

pub fn print_value(format: &OutputFormat, value: &Value) {
    match format {
        OutputFormat::Partiql => {
            partiql_pretty_print(value);
        }
        OutputFormat::IonLines => {
            let mut out = std::io::stdout().lock();
            let mut writer = ion_rs::TextWriterBuilder::lines()
                .build(&mut out)
                .expect("ion writer");
            ion_encode(&mut writer, value);
        }
        OutputFormat::IonPretty => {
            let mut out = std::io::stdout().lock();
            let mut writer = ion_rs::TextWriterBuilder::pretty()
                .build(&mut out)
                .expect("ion writer");
            ion_encode(&mut writer, value);
        }
        OutputFormat::Table => {
            // TODO: this should really be based on static analysis of the query's returned 'columns'
            let mut columns = vec![];
            let mut columns_to_id = HashMap::new();
            for v in value.iter() {
                if let Value::Tuple(t) = v {
                    for (k, _v) in t.pairs() {
                        columns_to_id.entry(k).or_insert_with(|| {
                            columns.push(k);

                            columns.len() - 1
                        });
                    }
                }
            }
            let empty_row: Vec<_> = std::iter::repeat(Value::Null)
                .take(columns.len())
                .map(|v| Cell::new(partiql_pretty(&v)).fg(Color::DarkRed))
                .collect();

            if columns.is_empty() {
                let mut table = Table::new();
                table.set_header(vec!["Value"]);
                for v in value.iter() {
                    table.add_row(&[partiql_table_pretty(v)]);
                }

                println!("{table}");
            } else {
                let mut table = Table::new();
                table.set_header(columns);
                for v in value.iter() {
                    let mut row = empty_row.clone();
                    for (k, v) in v.as_tuple_ref().as_ref().pairs() {
                        match columns_to_id.get(&k) {
                            None => {
                                todo!("error mapping key to column")
                            }
                            Some(idx) => {
                                let col = Cell::new(partiql_table_pretty(v));
                                let col = match v {
                                    // Color Null & Missing red
                                    Value::Null | Value::Missing => col.fg(Color::DarkRed),
                                    _ => col,
                                };
                                row[*idx] = col
                            }
                        }
                    }
                    table.add_row(row);
                }

                println!("{table}");
            }
        }
    }
}

fn partiql_table_pretty(value: &Value) -> String {
    let pretty = partiql_pretty(value);
    if pretty.starts_with('\'') && pretty.ends_with('\'') {
        pretty.trim_matches('\'').to_string()
    } else {
        pretty
    }
}

fn partiql_pretty(value: &Value) -> String {
    let mut pretty = String::new();
    value
        .pretty(&mut pretty)
        .expect("Error when trying to pretty print result");
    pretty
}

fn partiql_pretty_print(value: &Value) {
    let pretty = partiql_pretty(value);
    println!("{pretty}");
}

fn ion_encode<'a, W, I>(writer: &'a mut I, value: &Value)
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
