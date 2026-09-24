//! Evaluates a note read from stdin and prints the result as JSON.
//! Handy for trying the engine without the GUI:  echo "2 + 2 =" | cargo run --example evaluate

use std::io::Read;

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("failed to read stdin");
    let out = math_note_lib::math::evaluate_note(&text);
    println!(
        "{}",
        serde_json::to_string(&out).expect("serialisation failed")
    );
}
