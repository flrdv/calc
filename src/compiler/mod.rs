mod structs;

use std::collections::{HashMap, HashSet};
use lasso::Key;
use thiserror::Error;
use crate::number::Repr;
use crate::parser;
use crate::parser::{Node, Op, UOp};
use crate::vm::asm::{CodeObject, Inst, Program, Value, Sym};

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("bindings \"{1}\" and \"{3}\" are codependent")]
    RecursiveParams(parser::Pos, String, parser::Pos, String),
}

pub struct Compiler {
    program: Program,
    intern: lasso::Rodeo,
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            program: Vec::new(),
            intern: lasso::Rodeo::new(),
        }
    }

    pub fn compile(&mut self, tree: &Node) -> Result<&Program, CompileError> {
        let mut co = CodeObject::new();
        self.compile_co(&mut co, tree)?;
        self.program.push(co);
        Ok(&self.program)
    }

    fn compile_co(&mut self, co: &mut CodeObject, node: &Node) -> Result<(), CompileError> {
        match node {
            Node::Number(_, r) => co.push(Inst::Push(Value::Number(r.clone()))),
            Node::Symbol(_, name) => co.push(Inst::Load(name.into_usize())),
            Node::Unary(_, UOp::Pos, expr) => return self.compile_co(co, expr),
            Node::Unary(_, UOp::Neg, expr) => {
                self.compile_co(co, expr)?;
                co.push(Inst::Push(Value::Number(Repr::from(-1))));
                co.push(Inst::Mul);
            }
            Node::Unary(_, UOp::Not, expr) => {
                self.compile_co(co, expr)?;
                co.push(Inst::Not);
            }
            Node::Unary(_, UOp::Fac, expr) => {
                self.compile_co(co, expr)?;
                co.push(Inst::Fac);
            }
            Node::BinOp(op, lhs, rhs) => {
                self.compile_co(co, rhs)?;
                self.compile_co(co, lhs)?;
                self.compile_operation(co, op)?;
            },
            Node::WhereClause(_, expr, args) => {
                let mut map: HashMap<Sym, &Node> = HashMap::with_capacity(args.len());
                let argsyms: Vec<(Sym, &Node)> = args.iter()
                    .map(|(key, expr)| (key.into_usize(), expr.as_ref()))
                    .collect();
                map.extend(argsyms);

                let order = match Self::where_params_ordering(&map) {
                    Ok(order) => order,
                    Err((s1, s2)) => return Err(CompileError::RecursiveParams(
                        map.get(&s1).unwrap().pos(), self.intern.resolve(&Key::try_from_usize(s1).unwrap()).to_string(),
                        map.get(&s2).unwrap().pos(), self.intern.resolve(&Key::try_from_usize(s2).unwrap()).to_string(),
                    ))
                };

                let mut lambda_co = CodeObject::new();
                self.compile_co(&mut lambda_co, expr)?;
                lambda_co.push(Inst::Ret);
                let co_ptr = self.program.len();
                self.program.push(lambda_co);

                for sym in &order {
                    self.compile_co(co, map.get(sym).unwrap())?;
                    co.push(Inst::Store(*sym));
                }

                for sym in order.iter().rev() {
                    co.push(Inst::Load(*sym));
                }

                co.push(Inst::Push(Value::Lambda(co_ptr, order)));
                (0..map.len()).for_each(|_| co.push(Inst::Apply));
            },
            Node::Assign(_, sym, expr) => {
                self.compile_co(co, expr)?;
                co.push(Inst::Store(sym.into_usize()));
            }
            Node::Lambda(_, args, expr) => {
                let mut lambda_co = CodeObject::new();
                self.compile_co(&mut lambda_co, expr)?;
                lambda_co.push(Inst::Ret);
                self.program.push(lambda_co);
                co.push(Inst::Push(Value::Lambda(
                    self.program.len() - 1,
                    args.iter().map(|sym| sym.into_usize()).collect(),
                )))
            }
        };
        Ok(())
    }

    fn compile_operation(&mut self, co: &mut CodeObject, op: &Op) -> Result<(), CompileError> {
        match op {
            Op::Add => co.push(Inst::Add),
            Op::Sub => co.push(Inst::Sub),
            Op::Mul => co.push(Inst::Mul),
            Op::Div => co.push(Inst::Div),
            Op::Pow => co.push(Inst::Pow),
            Op::Eq => co.push(Inst::Cmp),
            Op::NEq => {
                co.push(Inst::Cmp);
                co.push(Inst::Not);
            }
            Op::Gt => co.push(Inst::Gt),
            Op::Lt => co.push(Inst::Lt),
            Op::Ge => co.push(Inst::Ge),
            Op::Le => co.push(Inst::Le),
            Op::App => co.push(Inst::Apply),
        }
        Ok(())
    }

    fn where_params_ordering(params: &HashMap<Sym, &Node>) -> Result<Vec<Sym>, (Sym, Sym)> {
        let mut population = Vec::with_capacity(params.len());

        for (sym, node) in params.iter() {
            let mut deps = HashSet::new();
            let mut exclusion = HashSet::new();
            Self::dependent_variables(node, &mut deps, &params, &mut exclusion);
            population.push((*sym, deps))
        };

        structs::topological_sort(population)
    }

    fn dependent_variables(expr: &Node, deps: &mut HashSet<Sym>, subjects: &HashMap<Sym, &Node>, exclude: &mut HashSet<Sym>) {
        match expr {
            Node::Symbol(_, name) =>
                if subjects.contains_key(&name.into_usize()) && !exclude.contains(&name.into_usize()) {
                    deps.insert(name.into_usize());
                },
            Node::Unary(_, _, rhs) => Self::dependent_variables(rhs, deps, subjects, exclude),
            Node::BinOp(_, lhs, rhs) => {
                Self::dependent_variables(lhs, deps, subjects, exclude);
                Self::dependent_variables(rhs, deps, subjects, exclude);
            },
            Node::Lambda(_, args, rhs) => {
                exclude.extend(args.iter().map(|key| key.into_usize()));
                Self::dependent_variables(rhs, deps, subjects, exclude);
            },
            Node::WhereClause(_, lhs, args) => {
                exclude.extend(args.iter().map(|(key, _)| key.into_usize()));
                Self::dependent_variables(lhs, deps, subjects, exclude);
            },
            Node::Assign(_, _, rhs) => Self::dependent_variables(rhs, deps, subjects, exclude),
            Node::Number(_, _) => {},
        };
    }
}
