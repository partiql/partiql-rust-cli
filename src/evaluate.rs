use crate::error::{CLIError, CLIErrors};

use partiql_catalog::extension::Extension;
use partiql_catalog::catalog::PartiqlCatalog;
use partiql_eval::env::basic::MapBindings;
use partiql_eval::eval::{BasicContext, EvalPlan, Evaluated};
use partiql_eval::plan::EvaluationMode;
use partiql_extension_ion::decode::IonDecoderConfig;
use partiql_extension_ion::Encoding;
use partiql_extension_ion_functions::IonExtension;
use partiql_logical::{BindingsOp, LogicalPlan};
use partiql_parser::Parsed;
use partiql_value::{DateTime, Value};
use std::fs;
use std::path::Path;
use partiql_catalog::context::SystemContext;

pub struct Compiler {
    catalog: PartiqlCatalog,
}

impl Default for Compiler {
    fn default() -> Self {
        Compiler { catalog: catalog() }
    }
}

impl Compiler {
    pub fn parse<'a>(&self, query: &'a str) -> Result<Parsed<'a>, CLIErrors> {
        partiql_parser::Parser::default()
            .parse(query)
            .map_err(CLIErrors::from)
    }

    pub fn plan(&self, query: &Parsed) -> Result<LogicalPlan<BindingsOp>, CLIErrors> {
        let planner = partiql_logical_planner::LogicalPlanner::new(&self.catalog);
        let lowered = planner.lower(query);
        match lowered {
            Ok(plan) => Ok(plan),
            Err(err) => Err(CLIErrors::from((query.text, err))),
        }
    }

    pub fn compile(
        &self,
        query: &Parsed,
        plan: &LogicalPlan<BindingsOp>,
    ) -> Result<EvalPlan, CLIErrors> {
        let mut compiler =
            partiql_eval::plan::EvaluatorPlanner::new(EvaluationMode::Permissive, &self.catalog);
        compiler
            .compile(&plan)
            .map_err(|err| CLIErrors::from((query.text, err)))
    }

    pub fn evaluate(
        &self,
        query: &Parsed,
        mut eval_plan: EvalPlan,
        bindings: MapBindings<Value>,
    ) -> Result<Evaluated, CLIErrors> {
        let sys = SystemContext {
            now: DateTime::from_system_now_utc(),
        };
        let ctx = BasicContext::new(bindings, sys);
        eval_plan
            .execute_mut(&ctx)
            .map_err(|err| CLIErrors::from((query.text, err)))
    }
}

fn catalog() -> PartiqlCatalog {
    let mut catalog = PartiqlCatalog::default();
    let ext = IonExtension {};
    ext.load(&mut catalog)
        .expect("ion extension load to succeed");
    catalog
}

pub fn evaluate(query: &str, globals: MapBindings<Value>) -> Result<Evaluated, CLIErrors> {
    let compiler = Compiler::default();
    let parsed = compiler.parse(query)?;
    let plan = compiler.plan(&parsed)?;
    let mut eval = compiler.compile(&parsed, &plan)?;
    compiler.evaluate(&parsed, eval, globals)
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
