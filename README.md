# Math Note

A local desktop notes app that does the arithmetic for you. Type freely; whenever a
line ends with `=`, the maths before it is evaluated and the result appears in grey
after your text. Results can be named and reused anywhere in the note.

Built with [Tauri 2](https://tauri.app) (Rust backend, WebView frontend) and SQLite.

## Syntax

| You write                      | You see                       | Notes                                          |
|--------------------------------|-------------------------------|------------------------------------------------|
| `2 + 2 =`                      | `2 + 2 = 4`                   | The `4` is rendered in grey.                   |
| `3 * 4 = area`                 | `3 * 4 = area = 12`           | Stores `area`; later lines can use it.         |
| `area / 2 =`                   | `area / 2 = 6`                |                                                |
| `r = 5`                        | `r = 5`                       | Defines `r`. Also `Let r = 5` or `r = 5 cm`.   |
| `2 pi r =`                     | `2 pi r = 31.4159265359`      | Juxtaposition multiplies: `2a`, `2(3+4)`.      |
| `The total is 3 * 4 =`         | `... 3 * 4 = 12`              | Words before a formula are ignored.            |
| `7 * 8 = 56`                   | `7 * 8 = 56 ✓`                | Your own answers are checked.                  |
| `7 * 8 = 54`                   | `7 * 8 = 54 ✗ 56`             |                                                |
| `2 + b =` (b undefined)        | `2 + b = unknown variable b`  | Errors only show on lines that look like maths.|

Operators: `+ - * / ^ ** mod`, postfix `!` (factorial) and `%` (percent), `√x`, and
the Unicode forms `× ÷ − ·`. Brackets `() [] {}` all group. `2 x 3` is read as
multiplication when `x` sits between two numbers.

Functions: `sqrt cbrt abs sin cos tan asin acos atan atan2 sinh cosh tanh ln log log2
log10 exp floor ceil round trunc sign deg rad min max sum avg pow root hypot`.
Constants: `pi` (`π`), `e`, `tau`. A variable with the same name shadows a constant.

Variables are evaluated top to bottom; redefining one changes every line below it.
The status bar lists the variables currently defined in the note.

## Running

Requirements: Rust (stable), Node.js, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```sh
npm install        # installs the Tauri CLI
npm run dev        # builds and launches the app
npm run build      # produces a distributable bundle in src-tauri/target/release/bundle
```

Notes are stored in a SQLite database in the platform app-data directory, for example
`~/Library/Application Support/com.mathnote.desktop/notes.db` on macOS.

Rust unit tests cover the maths engine and the database layer:

```sh
cd src-tauri && cargo test
```

To try the engine without the GUI, pipe a note into the example binary:

```sh
cd src-tauri && echo "3 * 4 = area
area / 2 =" | cargo run --example evaluate
```

## Layout

```
src/                      frontend: index.html, style.css, main.js (no bundler)
src-tauri/src/lib.rs      Tauri setup and commands
src-tauri/src/db.rs       SQLite storage
src-tauri/src/menu.rs     native menu (⌘N new note, ⌘W close tab, ⌘S save)
src-tauri/src/math/       lexer → parser → evaluator, plus the per-line note logic
```

The editor is a transparent `<textarea>` over a `<pre>` "mirror" that draws the same
text plus the grey results. The whole note is re-evaluated in Rust on every keystroke.
