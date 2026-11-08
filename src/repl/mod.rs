pub mod terminal;

use crate::compiler::{CompileError, Compiler};
use crate::parser::{Node, ParsingError};
use crate::vm::{RuntimeError, Value, VM};
use crate::vm::asm::program_to_string;


pub enum Action {
    More,
    ParsingError(ParsingError),
    CompileError(CompileError),
    RuntimeError(RuntimeError),
    Result(Value)
}

pub struct Instance {
    vm: VM,
    compiler: Compiler,
    intern: lasso::Rodeo,
}

impl Instance {
    pub fn new() -> Self {
        Instance {
            vm: VM::new(),
            compiler: Compiler::new(),
            intern: lasso::Rodeo::new(),
        }
    }

    pub fn input(&mut self, line: &str) -> Action {
        let ast = match Node::from(line, &mut self.intern) {
            Ok(node) => node,
            Err(ParsingError::NoCode|ParsingError::UnexpectedEOF|ParsingError::MissingClosingParen(_)) => return Action::More,
            Err(err) => return Action::ParsingError(err)
        };

        let program = match self.compiler.compile(&ast) {
            Ok(p) => p,
            Err(err) => return Action::CompileError(err)
        };

        println!("{}", program_to_string(program, &self.intern));
        println!("Sexpr: {}", ast.as_string(&self.intern));
        match self.vm.exec(program) {
            Ok(val) => Action::Result(val),
            Err(err) => Action::RuntimeError(err),
        }
    }
}

