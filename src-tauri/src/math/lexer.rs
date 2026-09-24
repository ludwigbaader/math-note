//! Tokenizer for the math expressions found inside a note line.

use super::format::format_number;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    Percent,
    Mod,
    Sqrt,
    LParen,
    RParen,
    Comma,
    /// A character that has no meaning in a formula (e.g. `:` or `?`).
    Invalid(char),
}

impl Token {
    /// True for tokens that only make sense inside a calculation. A line containing
    /// one of these is "mathy" enough that an evaluation error is worth showing.
    pub fn is_operator(&self) -> bool {
        matches!(
            self,
            Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::Caret
                | Token::Bang
                | Token::Percent
                | Token::Mod
                | Token::Sqrt
        )
    }

    /// True for tokens that look like ordinary words rather than parts of a formula.
    pub fn is_prose(&self) -> bool {
        matches!(self, Token::Ident(_) | Token::Invalid(_))
    }

    pub fn describe(&self) -> String {
        match self {
            Token::Num(v) => format!("number {}", format_number(*v)),
            Token::Ident(s) => format!("'{}'", s),
            Token::Plus => "'+'".into(),
            Token::Minus => "'-'".into(),
            Token::Star => "'*'".into(),
            Token::Slash => "'/'".into(),
            Token::Caret => "'^'".into(),
            Token::Bang => "'!'".into(),
            Token::Percent => "'%'".into(),
            Token::Mod => "'mod'".into(),
            Token::Sqrt => "'√'".into(),
            Token::LParen => "'('".into(),
            Token::RParen => "')'".into(),
            Token::Comma => "','".into(),
            Token::Invalid(c) => format!("character '{}'", c),
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn lex(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut out = Vec::new();

    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Numbers: 12, 3.5, .5, 2e3, 1.5E-4
        if c.is_ascii_digit() || (c == '.' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < n && chars[i] == '.' {
                i += 1;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < n && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < n && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < n && chars[j].is_ascii_digit() {
                    while j < n && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let text: String = chars[start..i].iter().collect();
            out.push(
                text.parse::<f64>()
                    .map(Token::Num)
                    .unwrap_or(Token::Invalid(c)),
            );
            continue;
        }

        // Identifiers: variables, function names, the `mod` keyword.
        if is_ident_start(c) {
            let start = i;
            while i < n && is_ident_continue(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            out.push(if word == "mod" {
                Token::Mod
            } else {
                Token::Ident(word)
            });
            continue;
        }

        let token = match c {
            '+' => Token::Plus,
            '-' | '−' | '–' => Token::Minus,
            '*' if i + 1 < n && chars[i + 1] == '*' => {
                i += 1;
                Token::Caret
            }
            '*' | '×' | '·' | '⋅' => Token::Star,
            '/' | '÷' => Token::Slash,
            '^' => Token::Caret,
            '!' => Token::Bang,
            '%' => Token::Percent,
            '√' => Token::Sqrt,
            '(' | '[' | '{' => Token::LParen,
            ')' | ']' | '}' => Token::RParen,
            ',' => Token::Comma,
            '∞' => Token::Num(f64::INFINITY),
            other => Token::Invalid(other),
        };
        out.push(token);
        i += 1;
    }

    fold_x_as_multiply(&mut out);
    out
}

/// People write `2 x 3` in casual notes. When a lone `x` sits between a number (or a
/// closing bracket) and a number, it is a multiplication sign, not a variable.
fn fold_x_as_multiply(tokens: &mut [Token]) {
    for i in 1..tokens.len().saturating_sub(1) {
        let is_x = matches!(&tokens[i], Token::Ident(s) if s == "x" || s == "X");
        if is_x
            && matches!(tokens[i - 1], Token::Num(_) | Token::RParen)
            && matches!(tokens[i + 1], Token::Num(_))
        {
            tokens[i] = Token::Star;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_numbers_and_operators() {
        assert_eq!(
            lex("1.5 + .5 * 2e3"),
            vec![
                Token::Num(1.5),
                Token::Plus,
                Token::Num(0.5),
                Token::Star,
                Token::Num(2000.0)
            ]
        );
    }

    #[test]
    fn exponent_marker_needs_digits() {
        assert_eq!(lex("2e"), vec![Token::Num(2.0), Token::Ident("e".into())]);
    }

    #[test]
    fn unicode_operators_and_words() {
        assert_eq!(
            lex("3 × π ÷ 2 − 1"),
            vec![
                Token::Num(3.0),
                Token::Star,
                Token::Ident("π".into()),
                Token::Slash,
                Token::Num(2.0),
                Token::Minus,
                Token::Num(1.0)
            ]
        );
        assert_eq!(
            lex("10 mod 3"),
            vec![Token::Num(10.0), Token::Mod, Token::Num(3.0)]
        );
        assert_eq!(
            lex("a: b"),
            vec![
                Token::Ident("a".into()),
                Token::Invalid(':'),
                Token::Ident("b".into())
            ]
        );
    }

    #[test]
    fn x_between_numbers_is_multiplication() {
        assert_eq!(
            lex("2 x 3"),
            vec![Token::Num(2.0), Token::Star, Token::Num(3.0)]
        );
        assert_eq!(lex("2 x"), vec![Token::Num(2.0), Token::Ident("x".into())]);
        assert_eq!(lex("x 3"), vec![Token::Ident("x".into()), Token::Num(3.0)]);
    }
}
