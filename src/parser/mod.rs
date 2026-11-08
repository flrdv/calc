mod lexer;

use thiserror::Error as ThisError;
use std::fmt::{Display, Formatter};
use crate::number::Repr;
use crate::parser::lexer::{LType, Lexeme, Lexer};

pub use crate::parser::lexer::Pos;


#[derive(ThisError, Debug, Clone)]
pub enum ParsingError {
    #[error("nothing to parse")]
    NoCode,
    #[error("unexpected token")]
    UnexpectedToken(Pos),
    #[error("unexpected EOF")]
    UnexpectedEOF,
    #[error("cannot assign to an expression")]
    UnexpectedExpr(Pos),
    #[error("expected identifier")]
    ExpectedID(Pos),
    #[error("missing the closing paren")]
    MissingClosingParen(Pos),
    #[error("bad syntax: {1}")]
    BadSyntax(Pos, String),
}

use Node::*;

#[derive(Debug)]
pub enum Node {
    Number(Pos, Repr),
    Symbol(Pos, lasso::Spur),
    Unary(Pos, UOp, Box<Node>),
    BinOp(Op, Box<Node>, Box<Node>),
    Assign(Pos, lasso::Spur, Box<Node>),
    Lambda(Pos, Vec<lasso::Spur>, Box<Node>),
    WhereClause(Pos, Box<Node>, Vec<(lasso::Spur, Box<Node>)>),
}

impl Node {
    pub fn from(str: &str, intern: &mut lasso::Rodeo) -> Result<Box<Self>, ParsingError> {
        Parser::parse(str, intern)
    }

    pub fn pos(&self) -> Pos {
        match self {
            Number(pos, _) => *pos,
            Symbol(pos, _) => *pos,
            Unary(pos, _, _) => *pos,
            BinOp(_, lhs, rhs) => {
                let (lpos, rpos) = (lhs.as_ref().pos(), rhs.as_ref().pos());
                (lpos.0, rpos.0 - lpos.0 + rpos.1)
            },
            Assign(pos, _, _) => *pos,
            Lambda(pos, _, _) => *pos,
            WhereClause(pos, _, _) => *pos,
        }
    }

    /// Pretty-prints the AST as S-expressions.
    pub fn as_string(&self, intern: &lasso::Rodeo) -> String {
        match self {
            Number(_, val) => val.to_string(),
            Symbol(_, sym) => intern.resolve(sym).to_string(),
            Unary(_, op, val) => format!("({op} {})", val.as_string(intern)),
            BinOp(op, lhs, rhs) => {
                format!(
                    "({op} {} {})",
                    lhs.as_string(intern),
                    rhs.as_string(intern),
                )
            },
            Assign(_, sym, expr) => format!("(assign {} {})", intern.resolve(sym), expr.as_string(intern)),
            Lambda(_, args, body) => {
                let strargs = args.iter()
                    .map(|sym| intern.resolve(sym))
                    .collect::<Vec<&str>>()
                    .join(" ");
                format!("(λ{strargs} . {})", body.as_string(intern))
            }
            WhereClause(_, expr, args) => format!(
                "({} where ({}))",
                expr.as_string(intern),
                args.iter()
                    .map(|sym| format!("{}={}", intern.resolve(&sym.0), sym.1.as_string(intern)))
                    .collect::<Vec<String>>()
                    .join(", "),
            ),
        }
    }
}

#[derive(Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    NEq,
    Gt,
    Lt,
    Ge,
    Le,
    App,
}

impl Display for Op {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Pow => "^",
            Op::Eq => "==",
            Op::NEq => "!=",
            Op::Gt => ">",
            Op::Lt => "<",
            Op::Ge => ">=",
            Op::Le => "<=",
            Op::App => "apply",
        })
    }
}

#[derive(Debug)]
pub enum UOp {
    Neg,
    Pos,
    Not,
    Fac,
}

impl Display for UOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            UOp::Neg => "-",
            UOp::Pos => "+",
            UOp::Not => "~",
            UOp::Fac => "!",
        })
    }
}

struct Parser<'a> {
    lex: Lexer<'a>,
    intern: &'a mut lasso::Rodeo,
}

impl<'a> Parser<'a> {
    /*
    todo add bitshifts and ranges
    stmt -> expr                                                                 -- temporarily dummy
    expr -> comp "where" [id = comp ","]* id = comp | id [= id]* = expr | comp   -- where clause, definition
    comp -> addsub == addsub | addsub != addsub | addsub > addsub | ... | addsub -- comparisons
    addsub -> muldiv + muldiv | muldiv - muldiv | muldiv                         -- addition and subtraction
    muldiv -> app * app | app / app | app                                        -- mul and div
    un -> -un | +un | !un | app                                                  -- unary
    app <- (jux -> number exp) | (app -> exp exp) | exp                          -- function application or juxtaposition
    exp -> fac ^ exp | fac                                                       -- power of
    fac -> atom! | atom                                                          -- factorial
    atom -> int | decimal | id | \ id... . expr | ( expr )
     */

    pub fn parse(text: &'a str, intern: &'a mut lasso::Rodeo) -> Result<Box<Node>, ParsingError> {
        let mut p = Self {
            lex: Lexer::new(text),
            intern
        };

        if p.lex.empty() {
            Err(ParsingError::NoCode)
        } else {
            let stmt = p.stmt()?;
            if !p.lex.empty() {
                Err(ParsingError::UnexpectedToken(
                    p.lex
                        .next()
                        .map(|l| l.1)
                        .unwrap_or((text.len(), text.len()))
                ))
            } else {
                Ok(stmt)
            }
        }
    }

    fn stmt(&mut self) -> Result<Box<Node>, ParsingError> {
        self.expr()
    }

    fn expr(&mut self) -> Result<Box<Node>, ParsingError> {
        let left = self.comp()?;
        Ok(match self.lex.next().unwrap_or_default().2 {
            "=" => {
                let right = self.expr()?;
                match left.as_ref() {
                    Symbol(pos, sym) => Assign(combine_positions(*pos, right.pos()), *sym, right).into(),
                    _ => return Err(ParsingError::UnexpectedExpr(left.pos())),
                }
            },
            "where" => {
                let (pos, params) = self.parse_where_clause()?;
                WhereClause(combine_positions(left.pos(), pos), left, params).into()
            },
            otherwise => {
                self.lex.stepback(otherwise);
                left
            }
        })
    }

    fn parse_where_clause(&mut self) -> Result<(Pos, Vec<(lasso::Spur, Box<Node>)>), ParsingError> {
        let mut params = Vec::new();

        loop {
            let id = match self.lex.next() {
                Some(Lexeme(LType::Id, _, name)) => self.intern.get_or_intern(name),
                Some(l) => return Err(ParsingError::ExpectedID(l.1)),
                None => return Err(ParsingError::UnexpectedEOF),
            };

            match self.lex.next().unwrap_or_default() {
                Lexeme(_, _, "=") => {},
                l => return Err(ParsingError::ExpectedID(l.1))
            }

            params.push((id, self.expr()?));

            match self.lex.next().unwrap_or_default().2 {
                "," => {},
                l => {
                    self.lex.stepback(l);
                    break;
                }
            }
        }

        Ok((params.last().unwrap().1.pos(), params))
    }

    fn comp(&mut self) -> Result<Box<Node>, ParsingError> {
        let left = self.addsub()?;
        // todo n-ary comparisons
        Ok(match self.lex.next().unwrap_or_default().2 {
            "==" => BinOp(Op::Eq, left, self.addsub()?).into(),
            "!=" => BinOp(Op::NEq, left, self.addsub()?).into(),
            ">" => BinOp(Op::Gt, left, self.addsub()?).into(),
            "<" => BinOp(Op::Lt, left, self.addsub()?).into(),
            ">=" => BinOp(Op::Ge, left, self.addsub()?).into(),
            "<=" => BinOp(Op::Le, left, self.addsub()?).into(),
            otherwise => {
                self.lex.stepback(otherwise);
                left
            }
        })
    }

    fn addsub(&mut self) -> Result<Box<Node>, ParsingError>  {
        let mut left = self.muldiv()?;
        loop {
            match self.lex.next().unwrap_or_default().2 {
                "+" => left = BinOp(Op::Add, left, self.muldiv()?).into(),
                "-" => left = BinOp(Op::Sub, left, self.muldiv()?).into(),
                otherwise => {
                    self.lex.stepback(otherwise);
                    break Ok(left);
                }
            }
        }
    }

    fn muldiv(&mut self) -> Result<Box<Node>, ParsingError>  {
        let mut left = self.un()?;
        loop {
            match self.lex.next().unwrap_or_default().2 {
                "*" => left = BinOp(Op::Mul, left, self.un()?).into(),
                "/" => left = BinOp(Op::Div, left, self.un()?).into(),
                otherwise => {
                    self.lex.stepback(otherwise);
                    break Ok(left);
                }
            }
        }
    }

    fn un(&mut self) -> Result<Box<Node>, ParsingError>  {
        Ok(match self.lex.next().unwrap_or_default() {
            // todo urgently! pos must correspond THE WHOLE unary node, not just the operator
            Lexeme(_, pos, "+") => Unary(pos, UOp::Pos, self.un()?).into(),
            Lexeme(_, pos, "-") => Unary(pos, UOp::Neg, self.un()?).into(),
            Lexeme(_, pos, "!") => Unary(pos, UOp::Not, self.un()?).into(),
            otherwise => {
                self.lex.stepback(otherwise.2);
                return self.app()
            }
        })
    }

    fn app(&mut self) -> Result<Box<Node>, ParsingError>  {
        let mut left = self.exp()?;
        while let Ok(arg) = self.exp() {
            left = match left.as_ref() {
                Number(_, _) => BinOp(Op::Mul, left, arg),
                _ => BinOp(Op::App, left, arg),
            }.into()
        }

        Ok(left)
    }

    fn exp(&mut self) -> Result<Box<Node>, ParsingError>  {
        let left = self.fac()?;
        Ok(match self.lex.next().unwrap_or_default().2 {
            "^" => BinOp(Op::Pow, left, self.exp()?).into(),
            otherwise => {
                self.lex.stepback(otherwise);
                left
            }
        })
    }

    fn fac(&mut self) -> Result<Box<Node>, ParsingError>  {
        let left = self.atom()?;
        Ok(match self.lex.next().unwrap_or_default() {
            // todo!! again, unary's pos must correspond the whole, not just the operator's pos
            Lexeme(_, pos, "!") => Unary(pos, UOp::Fac, left).into(),
            otherwise => {
                self.lex.stepback(otherwise.2);
                left
            }
        })
    }

    fn atom(&mut self) -> Result<Box<Node>, ParsingError>  {
        let token = self.lex.next();
        if token.is_none() {
            return Err(ParsingError::UnexpectedEOF);
        }

        Ok(match token.unwrap() {
            Lexeme(LType::Id, pos, id) => Symbol(pos, self.intern.get_or_intern(id)).into(),
            Lexeme(LType::Int|LType::Decimal, pos, val) => match Repr::try_from(val) {
                Ok(num) => Number(pos, num).into(),
                Err(err) => return Err(ParsingError::BadSyntax(pos, err.to_string()))
            },
            Lexeme(LType::Binary, pos, val) => Self::parse_num_base_n(pos, val, 2).into(),
            Lexeme(LType::Octodecimal, pos, val) => Self::parse_num_base_n(pos, val, 8).into(),
            Lexeme(LType::Hexadecimal, pos, val) => Self::parse_num_base_n(pos, val, 16).into(),
            Lexeme(LType::Scientific, pos, val) => Self::parse_scientific(pos, val).into(),
            Lexeme(LType::Keyword, pos, val) => {
                self.lex.stepback(val);
                return Err(ParsingError::UnexpectedToken(pos));
            },
            Lexeme(LType::Op, pos, str) => match str {
                "\\" => return self.lambda(pos),
                "(" => {
                    let expr = self.expr()?;
                    if self.lex.next().unwrap_or_default() != ")" {
                        return Err(ParsingError::MissingClosingParen(pos));
                    };

                    expr
                }
                _ => {
                    self.lex.stepback(str);
                    return Err(ParsingError::UnexpectedToken(pos));
                }
            },
        })
    }

    fn lambda(&mut self, pos: Pos) -> Result<Box<Node>, ParsingError> {
        let mut args = vec![];

        loop {
            match self.lex.next_op().unwrap_or_default().2 {
                "." => {
                    let expr = self.expr()?;
                    return Ok(Lambda(combine_positions(pos, expr.pos()), args, expr).into())
                },
                tok => self.lex.stepback(tok), // will error later
            }

            match self.lex.next() {
                Some(Lexeme(LType::Id, _, id)) => args.push(self.intern.get_or_intern(id)),
                Some(Lexeme(_, pos, _)) => return Err(ParsingError::UnexpectedToken(pos)),
                None => return Err(ParsingError::UnexpectedEOF)
            }
        }
    }

    fn parse_num_base_n(pos: Pos, s: &str, base: u32) -> Node {
        Number(pos, Repr::Int(dashu::Integer::from_str_radix(s, base).unwrap()))
    }

    fn parse_scientific(pos: Pos, str: &str) -> Node {
        Number(pos, Repr::try_from(str).unwrap().normalize())
    }
}

fn combine_positions(a: Pos, b: Pos) -> Pos {
    (a.0, b.0 - a.0 + b.1)
}

#[cfg(test)]
mod test_parser {
    use super::*;

    fn to_sexpr(program: &str) -> String {
        let mut intern = lasso::Rodeo::new();
        match Parser::parse(program, &mut intern) {
            Ok(node) => node.as_string(&intern),
            Err(err) => format!("(error {err})"),
        }
    }

    #[test]
    fn atoms() {
        assert_eq!("123", to_sexpr("123"));
        assert_eq!("123456/1000", to_sexpr("123.456"));
        assert_eq!("5/10", to_sexpr(".5"));
        assert_eq!("100000", to_sexpr("1e5"));
        assert_eq!("100000", to_sexpr("1e+5"));
        assert_eq!("1/100000", to_sexpr("1e-5"));
        assert_eq!("110000", to_sexpr("1.1e5"));
        assert_eq!("abc", to_sexpr("abc"));
        assert_eq!("5", to_sexpr("(5)"))
    }

    #[test]
    fn operators() {
        for tc in vec!["==", "!=", ">=", "<=", ">", "<", "+", "-", "*", "/", "^"] {
            assert_eq!(
                format!("({tc} 1 2)"),
                to_sexpr(format!("1 {tc} 2").as_str())
            );
        }
    }

    #[test]
    fn lambda() {
        assert_eq!("(λx y . (+ 1 2))", to_sexpr("\\x y . 1+2"));
        assert_eq!("(λ . (+ 1 2))", to_sexpr("\\. 1+2"));
        assert_eq!("(error unexpected EOF)", to_sexpr("\\."));
        assert_eq!("(error unexpected EOF)", to_sexpr("\\x y ."));
    }

    #[test]
    fn juxtaposition() {
        assert_eq!("(+ (* 5 (^ x 2)) (* 2 x))", to_sexpr("5x^2 + 2x"));
    }

    #[test]
    fn unary() {
        assert_eq!("(+ x)", to_sexpr("+x"));
        assert_eq!("(- x)", to_sexpr("-x"));
        assert_eq!("(~ x)", to_sexpr("!x"));
        assert_eq!("(! x)", to_sexpr("x!"));
        assert_eq!("(- (- x))", to_sexpr("--x"));
    }

    #[test]
    fn binop() {
        assert_eq!("(+ (+ 1 2) 3)", to_sexpr("1+2+3"));
        assert_eq!("(* (* 1 2) 3)", to_sexpr("1*2*3"));
        assert_eq!("(^ 2 (^ 3 4))", to_sexpr("2^3^4"));
    }

    #[test]
    fn application() {
        assert_eq!("(apply f x)", to_sexpr("f x"));
        assert_eq!("(+ (apply (apply f (* 5 x)) y) 3)", to_sexpr("f (5x) y+3"));
        assert_eq!("(- f x)", to_sexpr("f -x"));
        assert_eq!("(apply f (- x))", to_sexpr("f (-x)"));
        assert_eq!("(- x (apply f 5))", to_sexpr("x-f 5"));
        assert_eq!("(apply (apply f 5) 10)", to_sexpr("f 5 10"))
    }

    #[test]
    fn definition() {
        assert_eq!("(assign x 5)", to_sexpr("x = 5"));
        assert_eq!("(error cannot assign to an expression)", to_sexpr("1+2 = 3"));
        assert_eq!("(assign a (assign b (assign c 10)))", to_sexpr("a=b=c=10"));
        assert_eq!("(assign f (λx . (* 2 x)))", to_sexpr("f = \\x . 2x"));
        assert_eq!("(assign f (λx . (* 2 x)))", to_sexpr("f = \\x.2x"));
    }

    #[test]
    fn where_clause() {
        assert_eq!("((+ a b) where (a=(* 2 b), b=3))", to_sexpr("a+b where a=2b, b=3"))
    }
}
