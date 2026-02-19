use std::fmt::Display;

impl Display for super::Program<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program [")?;
        for global in &self.globals {
            writeln!(f, "{global}")?;
        }
        for fun in &self.functions {
            writeln!(f, "{fun}")?;
        }
        writeln!(f, "]")
    }
}

impl Display for super::GlobalVar<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "    global {}", self.name)?;
        if let Some(generation) = self.generation {
            write!(f, "{generation}")?;
        }
        if let Some(def) = self.def {
            write!(f, " <- {def}")?;
        }
        write!(f, ",")
    }
}

impl Display for super::Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "    fn {}:", self.name)?;
        for inst in &self.insts {
            writeln!(f, "{inst}")?;
        }
        write!(f, "    }},")
    }
}

impl Display for super::BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            Self::Add => "+",
            Self::Div => "/",
            Self::Mul => "*",
            Self::Rem => "%",
            Self::Sub => "-",
            Self::Shl => "<<",
            Self::Sar => ">>",
            Self::G => ">",
            Self::GE => ">=",
            Self::L => "<",
            Self::LE => "<=",
            Self::E => "==",
            Self::NE => "!=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
        };
        write!(f, "{op}")
    }
}

impl Display for super::UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Compliment => "~",
            Self::Negate => "-",
            Self::Not => "!",
        };
        write!(f, "{c}")
    }
}

impl Display for super::Val<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Const(imm) => write!(f, "${imm}"),
            Self::Temp(id) => write!(f, ".tmp{id}"),
            Self::Fn(id) | Self::GlobalVar(id) => write!(f, "{id}"),
            Self::LocalStaticLoad(id) => write!(f, "ls{id}"),
        }
    }
}

impl Display for super::Inst<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !matches!(self, super::Inst::Label(_)) {
            write!(f, "        ")?;
        }
        match self {
            Self::Binary { op, lhs, rhs, dst } => {
                write!(f, "{dst} <- {lhs} {op} {rhs}")
            }
            Self::Call { operand, args, dst } => {
                write!(f, "{dst} <- {operand}(")?;
                if let Some(first) = args.first() {
                    write!(f, "{first}")?;
                    for param in &args[1..] {
                        write!(f, ", {param}")?;
                    }
                }
                write!(f, ")")
            }
            Self::Copy { src, dst } => write!(f, "{dst} <- {src}"),
            Self::Jump(label) => write!(f, "jmp {label}"),
            Self::JumpIfNotZero(src, label) => write!(f, "jnz {src} {label}"),
            Self::JumpIfZero(src, label) => write!(f, "jz {src} {label}"),
            Self::Label(label) => write!(f, "    {label}"),
            Self::Ret(src) => write!(f, "ret {src}"),
            Self::Unary { op, src, dst } => write!(f, "{dst} <- {op}({src})"),
        }
    }
}
