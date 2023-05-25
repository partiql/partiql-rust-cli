use crate::error::CLIErrors;
use partiql_parser::Parsed;

pub fn parse(query: &str) -> Result<Parsed, CLIErrors> {
    partiql_parser::Parser::default()
        .parse(query)
        .map_err(CLIErrors::from_parser_error)
}
