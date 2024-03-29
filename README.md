# PartiQL Rust CLI
PoC for a CLI & REPL. It should be considered experimental, subject to change, etc.

In its current state, it largely exists to test parser and evaluation interfaces & types from the perspective of an 
external application.
Probably the mietter::Diagnostic stuff should be refactored and moved to the main parser crate.

## CLI Commands

- **`help`** : print the CLI's help message and supported commands
- **`repl`** : launches the [REPL](##REPL)
- **`ast -T<format> "<query>"`**: outputs a rendered version of the parsed AST  ([see Visualization](##Visualizations)):
  - **`<format>`**:
    - **`json`** : pretty-print to stdout in a json dump
    - **`dot`** : pretty-print to stdout in [Graphviz][Graphviz] [dot][GvDot] format
    - **`svg`** : print to stdout a [Graphviz][Graphviz] rendered svg xml document
    - **`png`** : print to stdout a [Graphviz][Graphviz] rendered png bitmap
    - **`display`** : display a [Graphviz][Graphviz] rendered png bitmap directly in supported terminals
  - **`query`** : the PartiQL query text
- **`eval -E<environment file> "<query>"`** : evaluate the query with the optional global environment
  - **`<environment file>`** : supports PartiQL values (as `.env`) and Ion text files (as `.ion`). See [sample-env](./sample-env) for some examples.
  - **`query`** : the PartiQL query text

## REPL

The REPL currently assumes most of the input line is a PartiQL query, which it will attempt to parse and evaluate.
- For an invalid query, errors are pretty printed to the output.
- For a valid query,
  - with no prefix, the evaluation result is pretty printed to the REPL shell
  - if prefixed by `\ast`, a rendered AST tree image is printed to the output ([see Visualization](##Visualizations))

Features:
- Syntax highlighting of query input
- User-friendly error reporting
- Reading/editing
- `CTRL-D`/`CTRL-C` to quit.

# Visualizations
In order to use any of the [Graphviz][Graphviz]-based visualizations, you will need the graphviz libraries
installed on your machine (e.g. `brew install graphviz` or similar). You will also need to build with the
`visualize` [feature][CargoFeatures] enabled.

# TODO

See [REPL-tagged issues](https://github.com/partiql/partiql-rust-cli/issues?q=is%3Aissue+is%3Aopen+%5BREPL%5D)

- Use central location for syntax files rather than embedded in this crate
- Better interaction model
  - commands
  - more robust editing
  - etc.
- Syntax highlighting for REPL output value


[Graphviz]: https://graphviz.org/
[GvDot]: https://graphviz.org/doc/info/lang.html
[CargoFeatures]: https://doc.rust-lang.org/cargo/reference/features.html#command-line-feature-options