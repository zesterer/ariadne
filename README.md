# Ariadne

[![crates.io](https://img.shields.io/crates/v/ariadne.svg)](https://crates.io/crates/ariadne)
[![crates.io](https://docs.rs/ariadne/badge.svg)](https://docs.rs/ariadne)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/zesterer/ariadne)
![actions-badge](https://github.com/zesterer/ariadne/workflows/Rust/badge.svg?branch=main)

A fancy compiler diagnostics crate.

## Usage

For each error you wish to report:
* Call [`Report::build()`] to start a [`ReportBuilder`].
* Assign whatever details are appropriate to the error using the various
  methods, and then call the [`finish`](ReportBuilder::finish) method to get a
  [`Report`] value.
* For each `Report`, call [`print`](Report::print) or [`eprint`](Report::eprint)
  to send the report directly to `stdout` or `stderr`. Alternately, you can use
  [`write`](Report::write) to send the report to any other
  [`Write`](std::io::Write) destinarion (such as a file).

A program such as this:

```rust
use ariadne::{Report, ReportKind, Label, Source};

fn main() {
    let source = Source::from(include_str!("../examples/sample.tao"));

    Report::build(ReportKind::Error, (), 34)
        .with_message("Incompatible types")
        .with_label(Label::new(32..33).with_message("This is of type Nat"))
        .with_label(Label::new(42..45).with_message("This is of type Str"))
        .finish()
        .print(source)
        .unwrap();
}
```

Gives us the following printout in the terminal:

<a href = "https://github.com/zesterer/ariadne/blob/main/examples/multiline.rs">
<img src="https://raw.githubusercontent.com/zesterer/ariadne/main/misc/example.png" alt="Ariadne supports arbitrary multi-line spans"/>
</a>

See [`examples/`](https://github.com/zesterer/ariadne/tree/main/examples) for more examples.

## About

`ariadne` is a sister project of [`chumsky`](https://github.com/zesterer/chumsky/). Neither are dependent on
one-another, but I'm working on both simultaneously and like to think that their features complement each other. If
you're thinking of using `ariadne` to process your compiler's output, why not try using `chumsky` to process its input?

## Features

- Inline and multi-line labels capable of handling arbitrary configurations of spans
- Multi-file errors
- Generic across custom spans and file caches
- A choice of character sets to ensure compatibility
- Coloured labels & highlighting with 8-bit and 24-bit color support (thanks to
  [`yansi`](https://github.com/SergioBenitez/yansi))
- Label priority and ordering
- Compact mode for smaller diagnostics
- Correct handling of variable-width characters such as tabs
- A `ColorGenerator` type that generates distinct colours for visual elements.
- A plethora of other options (tab width, label attach points, underlines, etc.)
- Built-in ordering/overlap heuristics that come up with the best way to avoid overlapping & label crossover

## Planned Features

- Improved layout planning & space usage
- Non-ANSI terminal support
- More accessibility options (screenreader-friendly mode, textured highlighting as an alternative to color, etc.)
- More color options
- Better support for layout restrictions (maximum terminal width, for example)

## Stability

The API (should) follow [semver](https://www.semver.org/). However, this does not apply to the layout of final error
messages. Minor tweaks to the internal layout heuristics can often result in the exact format of error messages changing
with labels moving slightly. If you experience a change in layout that you believe to be a regression (either the change
is incorrect, or makes your diagnostics harder to read) then please open an issue.

## Credit

Thanks to:

- `@brendanzab` for their beautiful [`codespan`](https://github.com/brendanzab/codespan) crate that inspired me to try
  pushing the envelope of error diagnostics.

- `@estebank` for showing innumerable people just how good compiler diagnostics can be through their work on Rust.
