# Report



An as-yet unnamed fancy error diagnostics & reporting crate. Designed to be used with [`chumsky`](https://github.com/zesterer/chumsky),
although not exclusively.

## Features

- Inline and multi-line labels capable of handling arbitrary configurations of spans
- Multi-file source support
- Generic across custom spans and file caches

## Planned Feature

- Support for syntax highlight, coloured text & labels, etc.
- Probably some other things

## Examples

The following...

```rust
fn main() {
    Report::build(ReportKind::Error, "b.tao", 10)
        .with_code(3)
        .with_message(format!("Cannot add types Nat and Str"))
        .with_label(Label::new(("b.tao", 10..14)).with_note("This is of type Nat"))
        .with_label(Label::new(("b.tao", 17..20)).with_note("This is of type Str"))
        .with_label(Label::new(("a.tao", 4..8)).with_note("Original definition of 'five' is here"))
        .finish()
        .print(sources(vec![
            ("a.tao", include_str!("a.tao")),
            ("b.tao", include_str!("b.tao")),
        ]))
        .unwrap();
}
```

...produces neat, inline, overlapping labels.

```
[E03] Error: Cannot add types Nat and Str
    ╭─[b.tao:1:11]
    │
  1 │ def six = five + "1"
    ·           ──┬─   ─┬─
    ·             ╰─────│── This is of type Nat
    ·                   │
    ·                   ╰── This is of type Str
    │
    ├─[a.tao:1:5]
    │
  1 │ def five = 5
    ·     ──┬─
    ·       ╰── Original definition of 'five' is here
────╯
```

The crate can also handle arbitrarily complex spans!

```
[E03] Error: Incompatible types
    ╭─[<unknown>:1:14]
    │
  1 │         def fives = ["5", 5]
    ·                           ┬
    ·                           ╰── This is of type Nat
  3 │         def sixes = ["6", 6, True, (), []]
    ·                      ─┬─  ┬  ──┬─  ─┬  ─┬
    ·                       ╰───┼────┼────┼───┼── This is of type Str
    ·                           │    │    │   │
    ·                           ╰────┼────┼───┼── This is of type Nat
    ·                                │    │   │
    ·                                ╰────┼───┼── This is of type Bool
    ·                                     │   │
    ·                                     ╰───┼── This is of type ()
    ·                                         │
    ·                                         ╰── This is of type [_]
  5 │         def multiline :: Str = match Some 5 in {
    ·                          ─┬─   │
    · ╭─────────────────────────┼────╯
    · │                         │
    · │                         ╰─────── This is of type Str
  6 │ │           | Some x => x
    · │        │     │ │ ┬┬
    · │ ╭──────╯     │ │ ││
    · │ │            │ │ ││
    · │ │ ╭──────────╯ │ ││
    · │ │ │            │ ││
    · │ │ │ ╭──────────╯ ││
    · │ │ │ │            ││
    · │ │ │ │            ╰┼── This is an inline within the nesting!
    · │ │ │ │             │
    · │ │ │ │             ╰── And another!
  7 │ │ │ │ │     | None => 0
    · │ │ │ │   │ │   │
    · │ │ ╰─┼───┴─┼───┼── This is another inner multi-line
    · │ │   │     │   │
    · │ │   ╰─────┴───┼── This is a *really* nested multi-line
    · │ │             │
    · │ ╰─────────────┴── This is an inner multi-line
  8 │ │       }
    · │       │
    · ╰───────┴── This is of type Nat
────╯
```
