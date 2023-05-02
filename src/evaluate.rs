use crate::error::CLIErrors;
use partiql_eval::env::basic::MapBindings;
use partiql_eval::eval::Evaluated;
use partiql_value::ion::parse_ion;
use partiql_value::Value;
use std::fs;
use std::path::Path;

pub fn evaluate(query: &str, globals: MapBindings<Value>) -> Result<Evaluated, CLIErrors> {
    let parser = partiql_parser::Parser::default();
    let parsed = parser.parse(query);
    let lowered = partiql_logical_planner::lower(&parsed.expect("parse"));
    let mut plan = partiql_eval::plan::EvaluatorPlanner.compile(&lowered);
    plan.execute_mut(globals)
        .map_err(|err| CLIErrors::from_eval_error(err, query))
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
                        let buf = fs::read_to_string(path)
                            .map_err(|err| CLIErrors::from_io_error(err, ""))?;
                        let env = evaluate(&buf, MapBindings::default())?.result;
                        match env {
                            Value::Tuple(t) => MapBindings::from(*t),
                            _ => panic!("Expected a struct containing the input environment"),
                        }
                    }
                    Some("ion") => {
                        let buf = fs::read_to_string(path)
                            .map_err(|err| CLIErrors::from_io_error(err, ""))?;
                        let env = parse_ion(&buf);
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
