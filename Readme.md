# calc

Functional expression-based language. It is intended to be used as a scientific
calculator.

# Run
In `./calc`, simply run `cargo run`. This will start a REPL session.

# Operations
The language in the current state supports:
- Long arithmetic
- All numbers are represented as rationals, making floating-point operations exact 
- All arithmetic, logical, comparison operations + factorials (e.g. `5! == 120`)
- Implied multiplication (e.g. `5x^2`)
- Function application (e.g. `add 1 2`)
- Variable and multivariable assignations (e.g. `a = b = c = 1`)
- Where-expressions (e.g. `a+b where a=2b, b=1`)
- Lambdas and currying (including scoping) (e.g. `((\x y.x+y) 1) 2`)

# TBD
- Irrationals and a built-in RRA for arbitrary-precision floating-point approximation
- Ternary and generally n-ary comparison chains (e.g. `0 < a <= b < 10`)
- Conditionals and pattern-matching (like in Haskell)
- Modules and non-REPL mode
- Strings, tuples and other standard collections
- Type system

# Functions
All functions are represented via lazy lambdas. Therefore, you can do:
```
add = \x y . x+y
add5 = add 5

add5 2 == 7
```
(considering each line a separate input in a REPL session)