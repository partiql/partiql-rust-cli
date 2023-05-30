use miette::{Diagnostic, LabeledSpan, SourceCode};
use partiql_eval::error::{EvalErr, EvaluationError, PlanErr, PlanningError};
use partiql_logical_planner::error::{LowerError, LoweringError};
use partiql_parser::{ParseError, ParserError};
use partiql_source_map::location::{ByteOffset, BytePosition, Location};
use std::io::Error;

use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("Error for query `{query}`")]
pub struct CLIErrors {
    query: String,
    #[related]
    related: Vec<CLIError>,
}

impl From<(&str, std::io::Error)> for CLIErrors {
    fn from((query, err): (&str, Error)) -> Self {
        CLIErrors {
            query: query.to_string(),
            related: vec![err.into()],
        }
    }
}

impl<'a> From<ParserError<'a>> for CLIErrors {
    fn from(err: ParserError<'a>) -> Self {
        let query = err.text.to_string();

        let related = err
            .errors
            .into_iter()
            .map(|e| (query.as_str(), e).into())
            .collect();
        CLIErrors { query, related }
    }
}

impl From<(&str, LoweringError)> for CLIErrors {
    fn from((query, err): (&str, LoweringError)) -> Self {
        let related = err.errors.into_iter().map(|e| (query, e).into()).collect();
        CLIErrors {
            query: query.to_string(),
            related,
        }
    }
}

impl From<(&str, EvalErr)> for CLIErrors {
    fn from((query, err): (&str, EvalErr)) -> Self {
        let related = err.errors.into_iter().map(|e| (query, e).into()).collect();
        CLIErrors {
            query: query.to_string(),
            related,
        }
    }
}

impl From<(&str, PlanErr)> for CLIErrors {
    fn from((query, err): (&str, PlanErr)) -> Self {
        let related = err.errors.into_iter().map(|e| (query, e).into()).collect();
        CLIErrors {
            query: query.to_string(),
            related,
        }
    }
}

#[derive(Debug, Error)]
pub enum CLIError {
    #[error("PartiQL syntax error:")]
    SyntaxError {
        src: String,
        msg: String,
        loc: Location<BytePosition>,
    },

    #[error("PartiQL compile error:")]
    CompileError {
        src: String,
        msg: String,
        // TODO loc: Location<BytePosition>,
    },

    #[error("Internal Compiler Error: `{msg}`\nplease report this (https://github.com/partiql/partiql-lang-rust/issues).")]
    InternalCompilerError { src: String, msg: String },

    #[error("I/O Error reading input environment")]
    IOReadError,

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl Diagnostic for CLIError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            CLIError::SyntaxError { src, .. } => Some(src),
            CLIError::InternalCompilerError { src, .. } => Some(src),
            CLIError::IOReadError => None,
            CLIError::CompileError { msg, src } => Some(src),
            CLIError::UnknownError(_) => None,
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            CLIError::SyntaxError { msg, loc, .. } => {
                Some(Box::new(std::iter::once(LabeledSpan::new(
                    Some(msg.to_string()),
                    loc.start.0 .0 as usize,
                    loc.end.0 .0 as usize - loc.start.0 .0 as usize,
                ))))
            }
            CLIError::InternalCompilerError { .. } => None,
            CLIError::IOReadError => None,
            CLIError::CompileError { .. } => None,
            CLIError::UnknownError(_) => None,
        }
    }
}

impl From<std::io::Error> for CLIError {
    fn from(err: Error) -> Self {
        CLIError::IOReadError
    }
}

impl<'a> From<(&str, ParseError<'a>)> for CLIError {
    fn from((source, err): (&str, ParseError<'a>)) -> Self {
        match err {
            ParseError::SyntaxError(partiql_source_map::location::Located { inner, location }) => {
                CLIError::SyntaxError {
                    src: source.to_string(),
                    msg: format!("Syntax error `{inner}`"),
                    loc: location,
                }
            }
            ParseError::UnexpectedToken(partiql_source_map::location::Located {
                inner,
                location,
            }) => CLIError::SyntaxError {
                src: source.to_string(),
                msg: format!("Unexpected token `{}`", inner.token),
                loc: location,
            },
            ParseError::LexicalError(partiql_source_map::location::Located { inner, location }) => {
                CLIError::SyntaxError {
                    src: source.to_string(),
                    msg: format!("Lexical error `{inner}`"),
                    loc: location,
                }
            }
            ParseError::Unknown(location) => CLIError::SyntaxError {
                src: source.to_string(),
                msg: "Unknown parser error".to_string(),
                loc: Location {
                    start: location,
                    end: location,
                },
            },
            ParseError::IllegalState(error) => CLIError::InternalCompilerError {
                msg: format!("Parser Illegal State: {error}"),
                src: source.to_string(),
            },
            ParseError::UnexpectedEndOfInput => {
                // Since `UnexpectedEndOfInput` doesn't include a source location, have the CLIError
                // point to the end of the input source. Tracking issue to add source location
                // to `UnexpectedEndOfInput`: https://github.com/partiql/partiql-lang-rust/issues/350
                let last_char = (source.len() - 1) as u32;
                CLIError::SyntaxError {
                    src: source.to_string(),
                    msg: "Unexpected end of input".to_string(),
                    loc: Location {
                        start: BytePosition(ByteOffset(last_char)),
                        end: BytePosition(ByteOffset(last_char)),
                    },
                }
            }
            other => CLIError::UnknownError(other.to_string()),
        }
    }
}

impl From<(&str, LowerError)> for CLIError {
    fn from((source, err): (&str, LowerError)) -> Self {
        match err {
            LowerError::IllegalState(error) => CLIError::InternalCompilerError {
                msg: format!("Compiler Illegal State: {error}"),
                src: source.to_string(),
            },
            LowerError::Literal { literal, error } => CLIError::CompileError {
                msg: format!(
                    "Compiler literal value error. Literal: `{literal}`. Error: `{error}`"
                ),
                src: source.to_string(),
            },
            LowerError::InvalidNumberOfArguments(error) => CLIError::CompileError {
                msg: format!("Compiler function error: Invalid number of args. Error: `{error}`"),
                src: source.to_string(),
            },
            LowerError::UnsupportedFunction(error) => CLIError::CompileError {
                msg: format!("Compiler function error: Unsupported function. Error: `{error}`"),
                src: source.to_string(),
            },
            LowerError::UnsupportedAggregationFunction(error) => CLIError::CompileError {
                msg: format!(
                    "Compiler function error: Unsupported aggregate function. Error: `{error}`"
                ),
                src: source.to_string(),
            },
            other => CLIError::UnknownError(other.to_string()),
        }
    }
}

impl From<(&str, PlanningError)> for CLIError {
    fn from((source, err): (&str, PlanningError)) -> Self {
        match err {
            PlanningError::IllegalState(error) => CLIError::InternalCompilerError {
                msg: format!("Planner Illegal State: {error}"),
                src: source.to_string(),
            },
            other => CLIError::UnknownError(other.to_string()),
        }
    }
}

impl From<(&str, EvaluationError)> for CLIError {
    fn from((query, err): (&str, EvaluationError)) -> Self {
        match err {
            EvaluationError::InvalidEvaluationPlan(error) => CLIError::InternalCompilerError {
                msg: format!("Compiler function error: Invalid Plan. Error: `{error}`"),
                src: query.to_string(),
            },
            other => CLIError::UnknownError(other.to_string()),
        }
    }
}
