//! Evaluates an expression tree against the variables defined so far in a note.

use std::collections::HashMap;
use std::f64::consts::{E, PI, TAU};

use super::parser::{BinOp, Expr};
use super::MathError;

/// Variables defined in a note, remembered in order of first definition.
#[derive(Debug, Default, Clone)]
pub struct Env {
    values: HashMap<String, f64>,
    order: Vec<String>,
}

impl Env {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    pub fn set(&mut self, name: &str, value: f64) {
        if self.values.insert(name.to_string(), value).is_none() {
            self.order.push(name.to_string());
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> + '_ {
        self.order
            .iter()
            .map(move |name| (name.as_str(), self.values[name]))
    }
}

const FUNCTIONS: &[&str] = &[
    "sqrt", "cbrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh",
    "tanh", "ln", "log", "log2", "log10", "exp", "floor", "ceil", "round", "trunc", "sign", "deg",
    "rad", "min", "max", "sum", "avg", "mean", "pow", "root", "hypot",
];

pub fn is_function(name: &str) -> bool {
    FUNCTIONS.contains(&name)
}

pub fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" | "π" => Some(PI),
        "e" => Some(E),
        "tau" | "τ" => Some(TAU),
        _ => None,
    }
}

pub fn eval(expr: &Expr, env: &Env) -> Result<f64, MathError> {
    let value = match expr {
        Expr::Num(v) => *v,
        Expr::Var(name) => match env.get(name).or_else(|| constant(name)) {
            Some(v) => v,
            None => return Err(MathError::UnknownVar(name.clone())),
        },
        Expr::Neg(inner) => -eval(inner, env)?,
        Expr::Bin(op, lhs, rhs) => {
            let a = eval(lhs, env)?;
            let b = eval(rhs, env)?;
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b == 0.0 {
                        return Err(MathError::DivZero);
                    }
                    a / b
                }
                BinOp::Mod => {
                    if b == 0.0 {
                        return Err(MathError::DivZero);
                    }
                    a.rem_euclid(b)
                }
                BinOp::Pow => a.powf(b),
            }
        }
        Expr::Call(name, args) => {
            let values = args
                .iter()
                .map(|a| eval(a, env))
                .collect::<Result<Vec<_>, _>>()?;
            call(name, &values)?
        }
        Expr::Factorial(inner) => factorial(eval(inner, env)?)?,
        Expr::Percent(inner) => eval(inner, env)? / 100.0,
        Expr::Sqrt(inner) => {
            let v = eval(inner, env)?;
            if v < 0.0 {
                return Err(MathError::Domain("square root of a negative number".into()));
            }
            v.sqrt()
        }
    };
    if value.is_nan() {
        return Err(MathError::Domain("result is not a real number".into()));
    }
    Ok(value)
}

fn factorial(x: f64) -> Result<f64, MathError> {
    if x < 0.0 || x.fract() != 0.0 || x > 170.0 {
        return Err(MathError::Domain(
            "factorial needs a whole number from 0 to 170".into(),
        ));
    }
    Ok((1..=x as u64).fold(1.0, |acc, k| acc * k as f64))
}

fn nth_root(x: f64, n: f64) -> Result<f64, MathError> {
    if n == 0.0 {
        return Err(MathError::Domain("root() degree cannot be 0".into()));
    }
    if x < 0.0 {
        if n.fract() == 0.0 && (n as i64) % 2 != 0 {
            return Ok(-(-x).powf(1.0 / n));
        }
        return Err(MathError::Domain("even root of a negative number".into()));
    }
    Ok(x.powf(1.0 / n))
}

fn call(name: &str, args: &[f64]) -> Result<f64, MathError> {
    fn need(name: &str, args: &[f64], n: usize) -> Result<(), MathError> {
        if args.len() == n {
            Ok(())
        } else {
            let plural = if n == 1 { "" } else { "s" };
            Err(MathError::Domain(format!(
                "{name}() expects {n} argument{plural}"
            )))
        }
    }
    fn need_some(name: &str, args: &[f64]) -> Result<(), MathError> {
        if args.is_empty() {
            Err(MathError::Domain(format!(
                "{name}() needs at least one argument"
            )))
        } else {
            Ok(())
        }
    }
    fn positive(name: &str, x: f64) -> Result<f64, MathError> {
        if x > 0.0 {
            Ok(x)
        } else {
            Err(MathError::Domain(format!(
                "{name}() needs a positive number"
            )))
        }
    }
    let unary = |f: fn(f64) -> f64| -> Result<f64, MathError> {
        need(name, args, 1)?;
        Ok(f(args[0]))
    };

    let value = match name {
        "sqrt" => {
            need(name, args, 1)?;
            if args[0] < 0.0 {
                return Err(MathError::Domain("square root of a negative number".into()));
            }
            args[0].sqrt()
        }
        "cbrt" => unary(f64::cbrt)?,
        "abs" => unary(f64::abs)?,
        "sin" => unary(f64::sin)?,
        "cos" => unary(f64::cos)?,
        "tan" => unary(f64::tan)?,
        "asin" => unary(f64::asin)?,
        "acos" => unary(f64::acos)?,
        "atan" => unary(f64::atan)?,
        "sinh" => unary(f64::sinh)?,
        "cosh" => unary(f64::cosh)?,
        "tanh" => unary(f64::tanh)?,
        "exp" => unary(f64::exp)?,
        "floor" => unary(f64::floor)?,
        "ceil" => unary(f64::ceil)?,
        "round" => unary(f64::round)?,
        "trunc" => unary(f64::trunc)?,
        "deg" => unary(f64::to_degrees)?,
        "rad" => unary(f64::to_radians)?,
        "sign" => {
            need(name, args, 1)?;
            let x = args[0];
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        "ln" => {
            need(name, args, 1)?;
            positive(name, args[0])?.ln()
        }
        "log" => match args.len() {
            1 => positive(name, args[0])?.log10(),
            2 => {
                let x = positive(name, args[0])?;
                let base = positive(name, args[1])?;
                if base == 1.0 {
                    return Err(MathError::Domain("log() base cannot be 1".into()));
                }
                x.ln() / base.ln()
            }
            _ => return Err(MathError::Domain("log() expects 1 or 2 arguments".into())),
        },
        "log2" => {
            need(name, args, 1)?;
            positive(name, args[0])?.log2()
        }
        "log10" => {
            need(name, args, 1)?;
            positive(name, args[0])?.log10()
        }
        "atan2" => {
            need(name, args, 2)?;
            args[0].atan2(args[1])
        }
        "pow" => {
            need(name, args, 2)?;
            args[0].powf(args[1])
        }
        "hypot" => {
            need(name, args, 2)?;
            args[0].hypot(args[1])
        }
        "root" => {
            need(name, args, 2)?;
            nth_root(args[0], args[1])?
        }
        "min" => {
            need_some(name, args)?;
            args.iter().copied().fold(f64::INFINITY, f64::min)
        }
        "max" => {
            need_some(name, args)?;
            args.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        }
        "sum" => {
            need_some(name, args)?;
            args.iter().sum()
        }
        "avg" | "mean" => {
            need_some(name, args)?;
            args.iter().sum::<f64>() / args.len() as f64
        }
        _ => return Err(MathError::UnknownVar(name.to_string())),
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::super::lexer::lex;
    use super::super::parser::parse;
    use super::*;

    fn run(src: &str, env: &Env) -> Result<f64, MathError> {
        eval(&parse(&lex(src))?, env)
    }

    #[test]
    fn arithmetic() {
        let env = Env::new();
        assert_eq!(run("1 + 2 * 3", &env), Ok(7.0));
        assert_eq!(run("(1 + 2) * 3", &env), Ok(9.0));
        assert_eq!(run("-7 mod 3", &env), Ok(2.0));
        assert_eq!(run("5!", &env), Ok(120.0));
        assert_eq!(run("50%", &env), Ok(0.5));
        assert_eq!(run("root(27, 3)", &env), Ok(3.0));
        assert_eq!(run("1 / 0", &env), Err(MathError::DivZero));
        assert!(matches!(run("sqrt(-1)", &env), Err(MathError::Domain(_))));
        assert!(matches!(run("(-8)^0.5", &env), Err(MathError::Domain(_))));
    }

    #[test]
    fn variables_shadow_constants() {
        let mut env = Env::new();
        assert!((run("2 pi", &env).unwrap() - TAU).abs() < 1e-12);
        env.set("pi", 3.0);
        assert_eq!(run("2 pi", &env), Ok(6.0));
        assert_eq!(run("q", &env), Err(MathError::UnknownVar("q".into())));
    }

    #[test]
    fn env_keeps_definition_order() {
        let mut env = Env::new();
        env.set("b", 1.0);
        env.set("a", 2.0);
        env.set("b", 3.0);
        let names: Vec<_> = env.iter().collect();
        assert_eq!(names, vec![("b", 3.0), ("a", 2.0)]);
    }
}
