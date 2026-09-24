//! Recursive-descent parser turning tokens into a small expression tree.
//!
//! Precedence, lowest to highest:
//!   `+ -`  →  `* / mod`  →  unary `-`  →  juxtaposition (`2a`, `2(3)`)  →  `^`  →  postfix `! %`

use super::eval::is_function;
use super::lexer::Token;
use super::MathError;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Factorial(Box<Expr>),
    Percent(Box<Expr>),
    Sqrt(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

/// Parses a complete token sequence. Every token must be consumed.
pub fn parse(tokens: &[Token]) -> Result<Expr, MathError> {
    if tokens.is_empty() {
        return Err(MathError::Parse("nothing to calculate".into()));
    }
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.expr()?;
    match parser.peek() {
        None => Ok(expr),
        Some(t) => Err(MathError::Parse(format!("unexpected {}", t.describe()))),
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<&'a Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, token: &Token) -> bool {
        if self.peek() == Some(token) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Expr, MathError> {
        self.additive()
    }

    fn additive(&mut self) -> Result<Expr, MathError> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.multiplicative()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn multiplicative(&mut self) -> Result<Expr, MathError> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                Some(Token::Mod) => BinOp::Mod,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, MathError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.implicit(),
        }
    }

    /// Juxtaposition means multiplication: `2a`, `2(3+4)`, `2 pi r`, `3√2`.
    /// It binds tighter than `*` and `/`, so `1/2a` is `1/(2a)`.
    fn implicit(&mut self) -> Result<Expr, MathError> {
        let mut lhs = self.power()?;
        while matches!(
            self.peek(),
            Some(Token::Ident(_)) | Some(Token::LParen) | Some(Token::Sqrt)
        ) {
            let rhs = self.power()?;
            lhs = Expr::Bin(BinOp::Mul, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn power(&mut self) -> Result<Expr, MathError> {
        let base = self.postfix()?;
        if self.eat(&Token::Caret) {
            let exponent = self.exponent()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exponent)));
        }
        Ok(base)
    }

    /// Right-associative exponent that may carry its own sign: `2^-1`, `2^3^2`.
    fn exponent(&mut self) -> Result<Expr, MathError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(Expr::Neg(Box::new(self.exponent()?)))
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.exponent()
            }
            _ => self.power(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, MathError> {
        let mut expr = self.primary()?;
        loop {
            match self.peek() {
                Some(Token::Bang) => {
                    self.pos += 1;
                    expr = Expr::Factorial(Box::new(expr));
                }
                Some(Token::Percent) => {
                    self.pos += 1;
                    expr = Expr::Percent(Box::new(expr));
                }
                _ => return Ok(expr),
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, MathError> {
        match self.bump() {
            Some(Token::Num(v)) => Ok(Expr::Num(*v)),
            Some(Token::Ident(name)) => {
                if self.peek() == Some(&Token::LParen) && is_function(name) {
                    self.pos += 1;
                    Ok(Expr::Call(name.clone(), self.arguments()?))
                } else {
                    Ok(Expr::Var(name.clone()))
                }
            }
            Some(Token::LParen) => {
                let inner = self.expr()?;
                if self.eat(&Token::RParen) {
                    Ok(inner)
                } else {
                    Err(MathError::Parse("missing closing parenthesis".into()))
                }
            }
            Some(Token::Sqrt) => Ok(Expr::Sqrt(Box::new(self.power()?))),
            Some(other) => Err(MathError::Parse(format!("unexpected {}", other.describe()))),
            None => Err(MathError::Parse("unexpected end of expression".into())),
        }
    }

    /// Parses `a, b, c)` once the opening parenthesis of a call has been consumed.
    fn arguments(&mut self) -> Result<Vec<Expr>, MathError> {
        let mut args = Vec::new();
        if self.eat(&Token::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.expr()?);
            if self.eat(&Token::Comma) {
                continue;
            }
            if self.eat(&Token::RParen) {
                return Ok(args);
            }
            return Err(match self.peek() {
                Some(t) => {
                    MathError::Parse(format!("unexpected {} in function arguments", t.describe()))
                }
                None => MathError::Parse("missing closing parenthesis".into()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::lexer::lex;
    use super::*;

    fn p(src: &str) -> Expr {
        parse(&lex(src)).unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::Bin(op, Box::new(a), Box::new(b))
    }

    #[test]
    fn precedence() {
        assert_eq!(
            p("1 + 2 * 3"),
            bin(
                BinOp::Add,
                Expr::Num(1.0),
                bin(BinOp::Mul, Expr::Num(2.0), Expr::Num(3.0))
            )
        );
        assert_eq!(
            p("-2^2"),
            Expr::Neg(Box::new(bin(BinOp::Pow, Expr::Num(2.0), Expr::Num(2.0))))
        );
        assert_eq!(
            p("2^3^2"),
            bin(
                BinOp::Pow,
                Expr::Num(2.0),
                bin(BinOp::Pow, Expr::Num(3.0), Expr::Num(2.0))
            )
        );
    }

    #[test]
    fn implicit_multiplication_binds_tighter_than_division() {
        assert_eq!(
            p("1/2a"),
            bin(
                BinOp::Div,
                Expr::Num(1.0),
                bin(BinOp::Mul, Expr::Num(2.0), Expr::Var("a".into()))
            )
        );
    }

    #[test]
    fn calls_and_postfix() {
        assert_eq!(
            p("max(1, 2)!"),
            Expr::Factorial(Box::new(Expr::Call(
                "max".into(),
                vec![Expr::Num(1.0), Expr::Num(2.0)]
            )))
        );
        assert_eq!(p("20%"), Expr::Percent(Box::new(Expr::Num(20.0))));
    }

    #[test]
    fn rejects_leftovers() {
        assert!(parse(&lex("2 3")).is_err());
        assert!(parse(&lex("(2")).is_err());
        assert!(parse(&lex("")).is_err());
    }
}
