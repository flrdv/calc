use lasso::Key;
use crate::number::Repr;

pub type Sym = usize;

#[derive(Debug, Clone)]
pub enum Inst {
    Store(Sym),
    Load(Sym),
    Push(Value),
    Apply,
    Ret,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Fac,
    Cmp,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Number(Repr),
    Lambda(
        usize,    /* CodeObject pointer */
        Vec<Sym>, /* arguments */
    ),
}

pub type CodeObject = Vec<Inst>;
pub type Program = Vec<CodeObject>;

fn co_to_string(intern: &lasso::Rodeo, co: &CodeObject) -> String {
    let mut lines = Vec::new();
    let resolve = |sym: &Sym| intern.try_resolve(&lasso::Spur::try_from_usize(*sym).unwrap()).or(Some("?")).unwrap();
    for inst in co {
        lines.push(match inst {
            Inst::Store(sym) => format!("store {}", resolve(sym)),
            Inst::Load(sym) => format!("load {}", resolve(sym)),
            Inst::Push(Value::Number(f)) => format!("push {f}"),
            Inst::Push(Value::Lambda(id, args)) => {
                let strargs = args.iter()
                    .map(resolve)
                    .collect::<Vec<&str>>()
                    .join(", ");
                format!("push codeobject({id}) {strargs}")
            }
            Inst::Apply => "apply".to_string(),
            Inst::Ret => "ret".to_string(),
            Inst::Not => "not".to_string(),
            Inst::Add => "add".to_string(),
            Inst::Sub => "sub".to_string(),
            Inst::Mul => "mul".to_string(),
            Inst::Div => "div".to_string(),
            Inst::Pow => "pow".to_string(),
            Inst::Fac => "fac".to_string(),
            Inst::Cmp => "cmp".to_string(),
            Inst::Gt => "gt".to_string(),
            Inst::Lt => "lt".to_string(),
            Inst::Ge => "ge".to_string(),
            Inst::Le => "le".to_string(),
        })
    }
    lines.join("\n")
}

pub fn program_to_string(program: &Program, intern: &lasso::Rodeo) -> String {
    let mut p = String::new();
    for (i, co) in program.iter().enumerate() {
        p.push_str(format!("{i}:\n").as_str());
        p.push_str((co_to_string(intern, co) + "\n").as_str());
    };

    p
}
