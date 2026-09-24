//! The maths engine: turns the text of a note into per-line results.
//!
//! Each line is inspected for an `=` sign. What comes before it is evaluated
//! (best effort, ignoring leading prose), what comes after decides what to do:
//!
//! * `2 + 2 =`        show the value
//! * `2 + 2 = a`      assign the value to `a` and show it
//! * `a = 2 + 2`      define `a` (also `Let a = ...`)
//! * `2 + 2 = 4`      check the claim, show ✓ or the real value
//!
//! Variables are remembered top to bottom, so later lines can use earlier ones.

pub mod eval;
pub mod format;
pub mod lexer;
pub mod parser;

use std::fmt;

use serde::Serialize;

use eval::Env;
use format::format_number;
use lexer::{lex, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum MathError {
    Parse(String),
    UnknownVar(String),
    DivZero,
    Domain(String),
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::Parse(msg) | MathError::Domain(msg) => f.write_str(msg),
            MathError::UnknownVar(name) => write!(f, "unknown variable {name}"),
            MathError::DivZero => f.write_str("division by zero"),
        }
    }
}

impl std::error::Error for MathError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultKind {
    /// A computed number to show in grey after the line.
    Value,
    /// The user's own result matched: `2 + 2 = 4`.
    Check,
    /// The user's own result was wrong; `text` carries the real value.
    Mismatch,
    /// The line looked like maths but could not be evaluated.
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LineResult {
    /// Zero-based line index in the note.
    pub line: usize,
    pub kind: ResultKind,
    /// Exactly what to render after the user's text.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteEvaluation {
    pub results: Vec<LineResult>,
    /// Every variable defined in the note, in order of first definition.
    pub variables: Vec<Variable>,
}

pub fn evaluate_note(text: &str) -> NoteEvaluation {
    let mut env = Env::new();
    let results = text
        .lines()
        .enumerate()
        .filter_map(|(line, content)| {
            process_line(content, &mut env).map(|(kind, text)| LineResult { line, kind, text })
        })
        .collect();
    let variables = env
        .iter()
        .map(|(name, value)| Variable {
            name: name.to_string(),
            value: format_number(value),
        })
        .collect();
    NoteEvaluation { results, variables }
}

/// Byte offset of the first `=` that acts as "equals" (not part of `==`, `<=`, `>=`, `!=`, `:=`, `=>`).
fn find_equals(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    (0..bytes.len()).find(|&i| {
        if bytes[i] != b'=' {
            return false;
        }
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        let next = bytes.get(i + 1).copied().unwrap_or(b' ');
        !matches!(prev, b'<' | b'>' | b'!' | b':' | b'=') && !matches!(next, b'=' | b'>')
    })
}

fn process_line(line: &str, env: &mut Env) -> Option<(ResultKind, String)> {
    let eq = find_equals(line)?;
    let left = line[..eq].trim();
    let right = line[eq + 1..].trim();
    if left.is_empty() {
        return None;
    }
    let left_tokens = lex(left);
    let right_tokens = lex(right);
    if left_tokens.is_empty() {
        return None;
    }

    let left_name = trailing_name(&left_tokens);
    let right_name = single_name(&right_tokens);

    // `expr =` — show the value.
    if right_tokens.is_empty() {
        return match best_effort(&left_tokens, env, Direction::Suffix) {
            Ok(ev) if ev.literal => None,
            Ok(ev) => Some((ResultKind::Value, format_number(ev.value))),
            Err(e) => error_if_mathy(&left_tokens, e),
        };
    }

    // `expr = name` — assign the value to the name on the right.
    if let Some(target) = right_name {
        if let Some(source) = left_name {
            // `a = b`: copy whichever side is already known onto the other.
            if let Some(v) = lookup(env, source) {
                env.set(target, v);
                return Some(equals(v));
            }
            if let Some(v) = lookup(env, target) {
                env.set(source, v);
                return Some(equals(v));
            }
            return None;
        }
        return match best_effort(&left_tokens, env, Direction::Suffix) {
            Ok(ev) => {
                env.set(target, ev.value);
                if ev.literal {
                    None
                } else {
                    Some(equals(ev.value))
                }
            }
            Err(e) => error_if_mathy(&left_tokens, e),
        };
    }

    // `name = expr` — define the name on the left.
    if let Some(name) = left_name {
        return match best_effort(&right_tokens, env, Direction::Prefix) {
            Ok(ev) => {
                env.set(name, ev.value);
                if ev.literal {
                    None
                } else {
                    Some(equals(ev.value))
                }
            }
            Err(e) => error_if_mathy(&right_tokens, e),
        };
    }

    // `expr = expr` — check the user's claim.
    match best_effort(&left_tokens, env, Direction::Suffix) {
        Ok(ev) if ev.literal => None,
        Ok(ev) => match best_effort(&right_tokens, env, Direction::Prefix) {
            Ok(claim) if approx_eq(ev.value, claim.value) => Some((ResultKind::Check, "✓".into())),
            Ok(_) => Some((
                ResultKind::Mismatch,
                format!("✗ {}", format_number(ev.value)),
            )),
            Err(_) => Some(equals(ev.value)),
        },
        Err(e) => error_if_mathy(&left_tokens, e),
    }
}

fn equals(v: f64) -> (ResultKind, String) {
    (ResultKind::Value, format!("= {}", format_number(v)))
}

fn lookup(env: &Env, name: &str) -> Option<f64> {
    env.get(name).or_else(|| eval::constant(name))
}

/// `a`, or `Let a` / `The area a`: an identifier preceded only by prose words.
fn trailing_name(tokens: &[Token]) -> Option<&str> {
    let (last, rest) = tokens.split_last()?;
    match last {
        Token::Ident(name) if rest.iter().all(Token::is_prose) => Some(name),
        _ => None,
    }
}

fn single_name(tokens: &[Token]) -> Option<&str> {
    match tokens {
        [Token::Ident(name)] => Some(name),
        _ => None,
    }
}

/// Only report errors for lines that contain an actual operator; otherwise prose
/// such as "total = the sum of things" would light up with "unknown variable".
fn error_if_mathy(tokens: &[Token], err: MathError) -> Option<(ResultKind, String)> {
    tokens
        .iter()
        .any(Token::is_operator)
        .then(|| (ResultKind::Error, err.to_string()))
}

fn approx_eq(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= 1e-9 * scale || format_number(a) == format_number(b)
}

#[derive(Clone, Copy)]
enum Direction {
    /// Drop leading tokens: "The area is 3 * 4" → "3 * 4".
    Suffix,
    /// Drop trailing tokens: "3 * 4 where the unit is cm" → "3 * 4".
    Prefix,
}

struct Evaluated {
    value: f64,
    /// The evaluated part was a bare number literal, so nothing was really computed.
    literal: bool,
}

/// Evaluates the tokens, and if that fails, progressively trims prose off one end.
/// A trim must remove only prose (no operators) and leave something that still starts
/// or ends like a complete expression, so "2 + b" with an unknown `b` stays an error
/// instead of silently becoming "2", and "a b + 1" never turns into "+ 1".
fn best_effort(tokens: &[Token], env: &Env, direction: Direction) -> Result<Evaluated, MathError> {
    let n = tokens.len();
    let mut parse_err: Option<MathError> = None;
    let mut eval_err: Option<MathError> = None;

    for skip in 0..n {
        let (kept, trimmed) = match direction {
            Direction::Suffix => (&tokens[skip..], &tokens[..skip]),
            Direction::Prefix => (&tokens[..n - skip], &tokens[n - skip..]),
        };
        if !trimmed.is_empty() && !is_clean_cut(kept, trimmed, direction) {
            continue;
        }
        match parser::parse(kept) {
            Err(e) => {
                if parse_err.is_none() {
                    parse_err = Some(e);
                }
            }
            Ok(expr) => match eval::eval(&expr, env) {
                Ok(value) => {
                    let literal = matches!(kept, [Token::Num(_)]);
                    return Ok(Evaluated { value, literal });
                }
                Err(e) => {
                    if eval_err.is_none() {
                        eval_err = Some(e);
                    }
                }
            },
        }
    }
    Err(eval_err
        .or(parse_err)
        .unwrap_or_else(|| MathError::Parse("nothing to calculate".into())))
}

fn is_clean_cut(kept: &[Token], trimmed: &[Token], direction: Direction) -> bool {
    if trimmed.iter().any(Token::is_operator) {
        return false;
    }
    match direction {
        Direction::Suffix => match kept {
            [Token::Num(_) | Token::Ident(_) | Token::LParen | Token::Sqrt, ..] => true,
            // "The result is -5 * 2": a negative number right after plain words.
            [Token::Minus, Token::Num(_), ..] => trimmed.iter().all(Token::is_prose),
            _ => false,
        },
        Direction::Prefix => matches!(
            kept.last(),
            Some(Token::Num(_) | Token::Ident(_) | Token::RParen | Token::Bang | Token::Percent)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        evaluate_note(text)
            .results
            .into_iter()
            .map(|r| format!("{}:{:?}:{}", r.line, r.kind, r.text))
            .collect()
    }

    #[test]
    fn plain_calculation() {
        assert_eq!(lines("2 + 2 ="), ["0:Value:4"]);
        assert_eq!(lines("2 + 2 =   "), ["0:Value:4"]);
    }

    #[test]
    fn assignment_and_reference() {
        assert_eq!(lines("2 + 2 = a\n2 + a ="), ["0:Value:= 4", "1:Value:6"]);
        assert_eq!(
            lines("3 * 4 = area\n\narea / 2 ="),
            ["0:Value:= 12", "2:Value:6"]
        );
    }

    #[test]
    fn definition_syntax() {
        assert_eq!(lines("a = 5\na * 2 ="), ["1:Value:10"]);
        assert_eq!(lines("x = 2 + 3"), ["0:Value:= 5"]);
        assert_eq!(lines("Let r = 3\n2 r ="), ["1:Value:6"]);
        assert_eq!(lines("Let x = 5 cm\nx * 2 ="), ["1:Value:10"]);
        assert_eq!(
            lines("y = 3x + 2 where x is above"),
            ["0:Error:unknown variable x"]
        );
        assert_eq!(lines("x = 2\ny = 3x + 2 where x is above"), ["1:Value:= 8"]);
    }

    #[test]
    fn copying_variables() {
        assert_eq!(lines("a = 5\na = b\nb ="), ["1:Value:= 5", "2:Value:5"]);
        assert_eq!(lines("b = 7\na = b\na ="), ["1:Value:= 7", "2:Value:7"]);
        assert!(lines("foo = bar").is_empty());
    }

    #[test]
    fn reassignment_uses_latest_value() {
        assert_eq!(lines("a = 1\na = a + 1\na ="), ["1:Value:= 2", "2:Value:2"]);
        assert_eq!(
            lines("a = 1\na =\na = 10\na ="),
            ["1:Value:1", "3:Value:10"]
        );
    }

    #[test]
    fn checks() {
        assert_eq!(lines("2 + 2 = 4"), ["0:Check:✓"]);
        assert_eq!(lines("2 + 2 = 5"), ["0:Mismatch:✗ 4"]);
        assert_eq!(lines("0.1 + 0.2 = 0.3"), ["0:Check:✓"]);
        assert_eq!(lines("2 + 2 = 3 + 1"), ["0:Check:✓"]);
        assert_eq!(lines("3 * 4 = twelve"), ["0:Value:= 12"]);
        assert_eq!(lines("2 + 2 = 4 apples"), ["0:Check:✓"]);
    }

    #[test]
    fn prose_before_formula_is_skipped() {
        assert_eq!(lines("The area is 3 * 4 ="), ["0:Value:12"]);
        assert_eq!(lines("Total: 10 / 4 ="), ["0:Value:2.5"]);
        assert_eq!(lines("Step 2: 1 + 1 = 2"), ["0:Check:✓"]);
        assert_eq!(
            lines("The area = 3 * 4\narea ="),
            ["0:Value:= 12", "1:Value:12"]
        );
        assert_eq!(lines("The result is -5 * 2 ="), ["0:Value:-10"]);
        assert_eq!(lines("3 * 4 = 12 (see above)"), ["0:Check:✓"]);
    }

    #[test]
    fn prose_is_left_alone() {
        assert!(lines("Note: nothing to see here").is_empty());
        assert!(lines("total = the sum of things").is_empty());
        assert!(lines("42 =").is_empty());
        assert!(lines("x =").is_empty());
        assert!(lines("a == b\nif a <= b\nx => y").is_empty());
        assert!(lines("= 5").is_empty());
        assert!(lines("The answer is 42 =").is_empty());
    }

    #[test]
    fn errors_only_for_mathy_lines() {
        assert_eq!(lines("2 + b ="), ["0:Error:unknown variable b"]);
        assert_eq!(lines("a b + 1 ="), ["0:Error:unknown variable a"]);
        assert_eq!(lines("2 b - 1 ="), ["0:Error:unknown variable b"]);
        assert_eq!(
            lines("z = 2 + b where b is unknown"),
            ["0:Error:unknown variable b"]
        );
        assert_eq!(lines("1 / 0 ="), ["0:Error:division by zero"]);
        assert_eq!(lines("x = 2 + b"), ["0:Error:unknown variable b"]);
        assert_eq!(
            lines("2 * (3 + ="),
            ["0:Error:unexpected end of expression"]
        );
        assert_eq!(lines("Note: 2 + b ="), ["0:Error:unknown variable b"]);
        assert_eq!(
            lines("(-1)! ="),
            ["0:Error:factorial needs a whole number from 0 to 170"]
        );
    }

    #[test]
    fn operators_and_functions() {
        assert_eq!(lines("sqrt(16) ="), ["0:Value:4"]);
        assert_eq!(lines("√16 ="), ["0:Value:4"]);
        assert_eq!(lines("2 pi ="), ["0:Value:6.28318530718"]);
        assert_eq!(lines("-2^2 ="), ["0:Value:-4"]);
        assert_eq!(lines("2^3^2 ="), ["0:Value:512"]);
        assert_eq!(lines("a = 4\n1/2a ="), ["1:Value:0.125"]);
        assert_eq!(lines("5! ="), ["0:Value:120"]);
        assert_eq!(lines("50% ="), ["0:Value:0.5"]);
        assert_eq!(lines("10 mod 3 ="), ["0:Value:1"]);
        assert_eq!(lines("2 x 3 ="), ["0:Value:6"]);
        assert_eq!(lines("2 * (3 + 4) ="), ["0:Value:14"]);
        assert_eq!(lines("[1 + 2] * 3 ="), ["0:Value:9"]);
        assert_eq!(lines("max(1, 5, 3) ="), ["0:Value:5"]);
        assert_eq!(lines("2e3 + 1 ="), ["0:Value:2001"]);
        assert_eq!(lines("3 × 4 ÷ 2 ="), ["0:Value:6"]);
        assert_eq!(lines("(1+2)(3+4) ="), ["0:Value:21"]);
        assert_eq!(lines("2 ** 10 ="), ["0:Value:1024"]);
    }

    #[test]
    fn number_formatting() {
        assert_eq!(lines("1/3 ="), ["0:Value:0.333333333333"]);
        assert_eq!(lines("0.1 + 0.2 ="), ["0:Value:0.3"]);
        assert_eq!(lines("1e20 * 1 ="), ["0:Value:1e20"]);
        assert_eq!(lines("1e-7 * 1 ="), ["0:Value:1e-7"]);
        assert_eq!(lines("2^0.5 ="), ["0:Value:1.41421356237"]);
    }

    #[test]
    fn variables_are_reported_in_definition_order() {
        let out = evaluate_note("b = 1\na = b + 1\nb = 10");
        assert_eq!(
            out.variables,
            vec![
                Variable {
                    name: "b".into(),
                    value: "10".into()
                },
                Variable {
                    name: "a".into(),
                    value: "2".into()
                },
            ]
        );
    }
}
