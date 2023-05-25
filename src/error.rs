use miette::{Diagnostic, LabeledSpan, SourceCode};
use partiql_eval::eval::{EvalErr, EvaluationError};
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

impl CLIErrors {
    pub fn from_parser_error(err: ParserError) -> Self {
        let query = err.text.to_string();

        let related = err
            .errors
            .into_iter()
            .map(|e| CLIError::from_parse_error(e, &query))
            .collect();
        CLIErrors { query, related }
    }

    pub fn from_eval_error(err: EvalErr, query: &str) -> Self {
        let related = err
            .errors
            .into_iter()
            .map(|e| CLIError::from_eval_error(e, query))
            .collect();
        CLIErrors {
            query: query.to_string(),
            related,
        }
    }

    pub fn from_io_error(err: Error, query: &str) -> Self {
        CLIErrors {
            query: query.to_string(),
            related: vec![CLIError::from_io_error(err)],
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
    #[error("Internal Compiler Error - please report this (https://github.com/partiql/partiql-lang-rust/issues).")]
    InternalCompilerError { src: String },

    #[error("I/O Error reading input environment")]
    IOReadError,
}

impl Diagnostic for CLIError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            CLIError::SyntaxError { src, .. } => Some(src),
            CLIError::InternalCompilerError { src, .. } => Some(src),
            CLIError::IOReadError => None,
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
        }
    }
}

impl CLIError {
    pub fn from_parse_error(err: ParseError, source: &str) -> Self {
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
            ParseError::IllegalState(_location) => CLIError::InternalCompilerError {
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
            _ => {
                todo!("Not yet handled {:?}", err);
            }
        }
    }

    pub fn from_eval_error(err: EvaluationError, source: &str) -> Self {
        match err {
            EvaluationError::InvalidEvaluationPlan(_s) => CLIError::InternalCompilerError {
                src: source.to_string(),
            },
            _ => CLIError::InternalCompilerError {
                src: source.to_string(),
            },
        }
    }

    pub fn from_io_error(_err: Error) -> Self {
        CLIError::IOReadError
    }
}
