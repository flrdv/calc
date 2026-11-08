use std::io;
use std::io::Write;
use crate::repl::{Action, Instance};
use crate::{number, vm};

pub struct Term {
    inst: Instance
}

impl Term {
    pub fn new() -> Self {
        Self {
            inst: Instance::new(),
        }
    }

    pub fn run(&mut self) {
        let mut line = String::with_capacity(128);
        let mut willing_more = false;

        loop {
            if !willing_more {
                print!(">> ");
                let _ = io::stdout().flush();
                line.clear();
            }

            io::stdin().read_line(&mut line).unwrap();

            match self.inst.input(line.as_str()) {
                Action::More => willing_more = true,
                Action::ParsingError(err) => println!("error during parsing: {err}"),
                Action::CompileError(err) => println!("error during compiling: {err}"),
                Action::RuntimeError(err) => println!("runtime error: {err}"),
                Action::Result(val) => {
                    print!("{} = ", line.trim());
                    match val {
                        vm::Value::Number(frac) => match frac.approximate() {
                            number::Approx::Exact => println!("{}", frac.normalize()),
                            number::Approx::ExactF64(f) => println!("{} = {f}", frac.normalize()),
                            number::Approx::PeriodicF64(f) => println!("{} ≈ {f}", frac.normalize())
                        },
                        vm::Value::Lambda(_co_ptr, _argsyms, _env) => println!("lambda")
                    }
                },
            }
        }
    }
}
