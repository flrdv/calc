mod parser;
mod vm;
mod number;
mod repl;
mod compiler;

fn main() {
    // let code = "(\\x . \\y . x+y) 5 10";
    // let code = "-5+4";
    // let code = "5! - 7/3";
    // let code = "a+b where a=1 b=3";
    let mut term = repl::terminal::Term::new();
    term.run();
}

#[cfg(test)]
mod tests {
    #[test]
    fn functions() {
        // todo test whether functions are applied correctly
    }
}
