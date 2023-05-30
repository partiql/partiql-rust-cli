use crate::error::{CLIError, CLIErrors};

use partiql_catalog::{Extension, PartiqlCatalog};
use partiql_eval::env::basic::MapBindings;
use partiql_eval::eval::Evaluated;
use partiql_extension_ion::decode::IonDecoderConfig;
use partiql_extension_ion::Encoding;
use partiql_extension_ion_functions::IonExtension;
use partiql_logical::{BindingsOp, LogicalPlan};
use partiql_logical_planner::error::LoweringError;
use partiql_parser::Parsed;
use partiql_value::Value;
use std::fs;
use std::path::Path;

fn catalog() -> PartiqlCatalog {
    let mut catalog = PartiqlCatalog::default();
    let ext = IonExtension {};
    ext.load(&mut catalog)
        .expect("ion extension load to succeed");
    catalog
}

pub fn evaluate(query: &str, globals: MapBindings<Value>) -> Result<Evaluated, CLIErrors> {
    let parser = partiql_parser::Parser::default();
    let parsed = parser.parse(query);
    evaluate_parsed(&parsed?, globals)
}

pub fn evaluate_parsed(
    query: &Parsed,
    globals: MapBindings<Value>,
) -> Result<Evaluated, CLIErrors> {
    let catalog = catalog();
    let planner = partiql_logical_planner::LogicalPlanner::new(&catalog);
    let lowered = planner.lower(query);
    let plan = match lowered {
        Ok(plan) => plan,
        Err(err) => return Err(CLIErrors::from((query.text, err))),
    };

    let mut compiler = partiql_eval::plan::EvaluatorPlanner::new(&catalog);
    let mut plan = compiler
        .compile(&plan)
        .map_err(|err| CLIErrors::from((query.text, err)))?;

    plan.execute_mut(globals)
        .map_err(|err| CLIErrors::from((query.text, err)))
}

pub fn get_bindings(environment: &Option<String>) -> Result<MapBindings<Value>, CLIErrors> {
    let bindings = match environment {
        None => MapBindings::default(),
        Some(path) => {
            let path = Path::new(path);
            match path.extension() {
                // TODO: can replace panics with an actual `CLIError`
                None => panic!("Expected path"),
                Some(extension) => match extension.to_str() {
                    Some("env") => {
                        let buf =
                            fs::read_to_string(path).map_err(|err| CLIErrors::from(("", err)))?;
                        let env = evaluate(&buf, MapBindings::default())?.result;
                        match env {
                            Value::Tuple(t) => MapBindings::from(*t),
                            _ => panic!("Expected a struct containing the input environment"),
                        }
                    }
                    Some("ion") => {
                        let buf =
                            fs::read_to_string(path).map_err(|err| CLIErrors::from(("", err)))?;
                        let reader = ion_rs::ReaderBuilder::new().build(buf).expect("ion reader");
                        let mut decoder = partiql_extension_ion::decode::IonDecoderBuilder::new(
                            IonDecoderConfig::default().with_mode(Encoding::PartiqlEncodedAsIon),
                        )
                        .build(reader)
                        .expect("expected ion file");
                        let env = decoder
                            .next()
                            .expect("expected single environment value in ion stream")
                            .expect("expected single environment value in ion stream");

                        match env {
                            Value::Tuple(t) => MapBindings::from(*t),
                            _ => panic!("Expected a struct containing the input environment"),
                        }
                    }
                    _ => panic!("Expected one of `.env` or `.ion`"),
                },
            }
        }
    };
    Ok(bindings)
}
