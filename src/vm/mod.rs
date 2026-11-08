pub mod asm;

use lasso::Key;
use thiserror::Error;
use crate::vm::asm::{Inst, Program, Sym};
use crate::number::{ArithmeticError, Repr};

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("BUG: empty callstack, nowhere to return")]
    NoRetPtr,
    #[error("BUG: stack error: {0}")]
    EmptyStack(String),
    #[error("symbol not found: {0}")]
    SymNotFound(Sym),
    #[error("type error: {0}")]
    TypeError(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("runtime error: {0}")]
    ArithmeticError(ArithmeticError),
}

impl From<ArithmeticError> for RuntimeError {
    fn from(err: ArithmeticError) -> Self {
        RuntimeError::ArithmeticError(err)
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(Repr),
    Lambda(
        usize,             /* code object ptr */
        Vec<Sym>,          /* argument symbols */
        Vec<(Sym, Value)>, /* arguments */
    ),
}

impl Value {
    pub fn to_string(&self, intern: &lasso::Rodeo) -> String {
        match self {
            Value::Number(num) => num.to_string(),
            Value::Lambda(co_ptr, args, env) =>
                format!(
                    "{co_ptr}λ{} ({})",
                    args.iter()
                        .map(|sym| intern.resolve(&lasso::Spur::try_from_usize(*sym).unwrap()))
                        .collect::<Vec<&str>>()
                        .join(", "),
                    env.iter()
                        .map(|sym| format!("{}={}", sym.0, sym.1.to_string(intern)))
                        .collect::<Vec<String>>()
                        .join(", "),
                )
        }
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    vals: Vec<(Sym, Value)>,
    namespaces: Vec<usize>,
}

impl Env {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(n: usize) -> Self {
        const AMORTIZED_CALLSTACK_DEPTH: usize = 5;

        Env {
            vals: Vec::with_capacity(n),
            namespaces: Vec::with_capacity(AMORTIZED_CALLSTACK_DEPTH),
        }
    }

    pub fn get(&self, sym: Sym) -> Option<Value> {
        self.find(sym).map(|idx| self.vals[idx].1.clone())
    }

    pub fn set(&mut self, sym: Sym, value: Value) {
        match self.find(sym) {
            Some(idx) => self.vals[idx].1 = value,
            None => self.vals.push((sym, value)),
        }
    }

    pub fn set_shadowed(&mut self, sym: Sym, value: Value) {
        let scope_begin = self.namespaces.last().unwrap_or(&0);
        match self.find_at(*scope_begin, sym).map(|pos| pos + scope_begin) {
            Some(idx) => self.vals[idx].1 = value,
            None => self.vals.push((sym, value)),
        }
    }

    pub fn enter_scope(&mut self) {
        self.namespaces.push(self.vals.len())
    }

    pub fn leave_scope(&mut self) {
        self.vals.truncate(self.namespaces.pop().unwrap_or(self.vals.len()))
    }
    
    fn find(&self, sym: Sym) -> Option<usize> {
        self.find_at(0, sym)
    }

    fn find_at(&self, at: usize, sym: Sym) -> Option<usize> {
        self.vals.iter()
            .enumerate()
            .skip(at)
            .rfind(|(_, (name, _))| *name == sym)
            .map(|(idx, _)| idx)
    }
}

/// PC is a program counter. It is a two-dimensional vector, where x corresponds the
/// code object's index and y corresponds the current instruction's index.
type PC = (usize, usize);

pub struct VM {
    callstack: Vec<(usize, usize)>,
    vstack: Vec<Value>,
    names: Env,
}

impl VM {
    pub fn new() -> Self {
        VM {
            callstack: Vec::new(),
            vstack: Vec::new(),
            names: Env::new(),
        }
    }

    pub fn exec(&mut self, program: &Program) -> Result<Value, RuntimeError> {
        let mut pc: PC = (program.len() - 1, 0);

        while pc.1 < program[pc.0].len() {
            match &program[pc.0][pc.1] {
                Inst::Store(sym) => match self.vstack.pop() {
                    Some(value) => {
                        self.names.set(*sym, value.clone());
                        self.vstack.push(value);
                    },
                    None => return Err(RuntimeError::EmptyStack("try to store from an empty stack".to_string())),
                },
                Inst::Load(sym) => match self.names.get(*sym) {
                    Some(value) => self.vstack.push(value),
                    None => return Err(RuntimeError::SymNotFound(*sym)),
                },
                Inst::Push(asm::Value::Number(frac)) => self.vstack.push(Value::Number(frac.clone())),
                Inst::Push(asm::Value::Lambda(co_ptr, args)) => {
                    let mut arguments = args.clone();
                    arguments.reverse();
                    let env = Vec::with_capacity(arguments.len());
                    self.vstack.push(
                        Value::Lambda(
                            *co_ptr,
                            arguments,
                            env,
                        )
                    );
                },
                Inst::Apply => {
                    if self.vstack.len() < 2 {
                        return Err(RuntimeError::EmptyStack("not enough arguments on stack for application".to_string()))
                    }

                    let (fun, arg) = self.pop_two()?;
                    match fun {
                        Value::Lambda(co_ptr, mut args, mut env) => {
                            if !args.is_empty() {
                                env.push((args.pop().unwrap(), arg));
                            }
                            if args.is_empty() {
                                self.names.enter_scope();

                                self.push_args(env);
                                self.callstack.push(pc);
                                pc = (co_ptr, 0);
                                continue
                            }
                            self.vstack.push(Value::Lambda(co_ptr, args, env))
                        },
                        otherwise => return Err(RuntimeError::TypeError(
                            format!("cannot apply {arg:?}: {otherwise:?} is not a function")
                        )),
                    }
                },
                Inst::Ret => {
                    self.names.leave_scope();
                    match self.callstack.pop() {
                        Some(resume_at) => pc = resume_at,
                        None => return Err(RuntimeError::NoRetPtr)
                    }
                },
                Inst::Not => {
                    match self.pop_one()? {
                        Value::Number(n) => self.vstack.push(Value::Number(n)),
                        Value::Lambda(_, _, _) => return Err(RuntimeError::NotImplemented("cannot negate functions".to_string())),
                    }
                }
                Inst::Add => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.vstack.push(Value::Number(lhs+rhs)),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} + {b:?}")))
                    }
                }
                Inst::Sub => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.vstack.push(Value::Number(lhs-rhs)),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} - {b:?}")))
                    }
                }
                Inst::Mul => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.vstack.push(Value::Number(lhs*rhs)),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} * {b:?}")))
                    }
                }
                Inst::Div => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.vstack.push(Value::Number(lhs/rhs)),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} / {b:?}")))
                    }
                }
                Inst::Pow => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.vstack.push(Value::Number(lhs.pow(rhs)?)),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} ^ {b:?}")))
                    }
                }
                Inst::Fac => {
                    match self.pop_one()? {
                        Value::Number(n) => self.vstack.push(Value::Number(n.factorial()?)),
                        n => return Err(RuntimeError::TypeError(format!("cannot factorialize {n:?}")))
                    }
                }
                Inst::Cmp => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.push_bool(lhs == rhs),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} == {b:?}")))
                    }
                }
                Inst::Gt => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.push_bool(lhs > &rhs),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} > {b:?}")))
                    }
                }
                Inst::Lt => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.push_bool(lhs < &rhs),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} < {b:?}")))
                    }
                }
                Inst::Ge => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.push_bool(lhs >= &rhs),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} >= {b:?}")))
                    }
                }
                Inst::Le => {
                    match self.pop_two()? {
                        (Value::Number(lhs), Value::Number(rhs)) => self.push_bool(lhs <= &rhs),
                        (a, b) => return Err(RuntimeError::TypeError(format!("{a:?} <= {b:?}")))
                    }
                }
            };
            pc.1 += 1;
        };

        match self.vstack.pop() {
            Some(value) => Ok(value),
            None => Err(RuntimeError::EmptyStack("stack underflow".to_string())),
        }
    }

    fn pop_one(&mut self) -> Result<Value, RuntimeError> {
        match self.vstack.pop() {
            Some(value) => Ok(value),
            None => Err(RuntimeError::EmptyStack("not enough values on stack".to_string()))
        }
    }

    fn pop_two(&mut self) -> Result<(Value, Value), RuntimeError> {
        let lhs = self.pop_one()?;
        let rhs = self.pop_one()?;
        Ok((lhs, rhs))
    }

    fn push_bool(&mut self, b: bool) {
        self.vstack.push(Value::Number(Repr::from(b)))
    }

    fn push_args(&mut self, vals: Vec<(Sym, Value)>) {
        for (sym, v) in vals {
            self.names.set_shadowed(sym, v.clone());
            self.vstack.push(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vm::*;
    use crate::parser;
    use crate::compiler::Compiler;

    fn compile(code: &str) -> Program {
        let mut compiler = Compiler::new();
        compiler
            .compile(parser::Node::from(code, &mut lasso::Rodeo::new()).unwrap().as_ref())
            .unwrap()
            .to_vec()
    }

    fn want_int(n: Repr, code: &str) {
        let mut vm = VM::new();
        let program = compile(code);
        match vm.exec(&program) {
            Ok(Value::Number(r)) => assert_eq!(r, n),
            Ok(_) => panic!("expected a number"),
            Err(err) => panic!("error: {err}"),
        }
    }

    #[test]
    fn lambda() {
        want_int(Repr::from(3), "(\\x y.x+y) 1 2");
        want_int(Repr::from(3), "(\\x.\\y.x+y) 1 2");
    }
}

#[cfg(test)]
mod env_tests {
    use crate::number::Repr;
    use crate::vm::{Env, Value};

    fn expect_number(val: Value, num: isize) {
        if let Value::Number(r) = val {
            assert_eq!(r, &Repr::from(num))
        } else {
            panic!("got random-ass data")
        }
    }

    #[test]
    fn insertion_and_lookup() {
        let mut env = Env::with_capacity(1);
        assert!(env.get(0).is_none());
        env.set(0, Value::Number(Repr::from(5)));
        expect_number(env.get(0).unwrap(), 5);
    }

    #[test]
    fn destruct_on_scope_leave() {
        let mut env = Env::with_capacity(2);
        env.set(0, Value::Number(Repr::from(1)));
        env.enter_scope();
        env.set(1, Value::Number(Repr::from(2)));
        expect_number(env.get(0).unwrap(), 1);
        expect_number(env.get(1).unwrap(), 2);
        env.leave_scope();
        assert!(env.get(1).is_none());
        expect_number(env.get(0).unwrap(), 1);
    }

    #[test]
    fn shadow_names() {
        let mut env = Env::with_capacity(2);
        env.set(0, Value::Number(Repr::from(1)));
        env.enter_scope();
        env.set_shadowed(0, Value::Number(Repr::from(2)));
        expect_number(env.get(0).unwrap(), 2);
        env.leave_scope();
        expect_number(env.get(0).unwrap(), 1);
    }

    #[test]
    fn lookup_parent_scope() {
        let mut env = Env::with_capacity(2);
        env.set(0, Value::Number(Repr::from(1)));
        env.enter_scope();
        env.set_shadowed(1, Value::Number(Repr::from(2)));
        expect_number(env.get(0).unwrap(), 1);
        expect_number(env.get(1).unwrap(), 2);
    }
}
