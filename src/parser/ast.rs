use std::fmt;

// ─── Program ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

// ─── Items ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnItem),
    Struct(StructItem),
    Enum(EnumItem),
    Const(ConstItem),
    Impl(ImplItem),
    Trait(TraitItem),
}

#[derive(Debug, Clone)]
pub struct TraitItem {
    pub name: String,
    pub type_params: Vec<String>,
    pub methods: Vec<FnItem>, // method signatures (body optional)
}

#[derive(Debug, Clone)]
pub struct ImplItem {
    pub trait_name: Option<String>, // Some("TraitName") for `impl Trait for Type`
    pub target_type: Type,
    pub type_params: Vec<String>,  // <T> for impl<T>
    pub methods: Vec<FnItem>,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    /// Trait names this parameter must implement (`T: Show + Clone`).
    pub bounds: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FnItem {
    pub name: String,
    /// Generic params with their declared trait bounds (<T: Show>, plain <U> = no bounds).
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret_type: Option<Type>,
    pub body: Block,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct StructItem {
    pub name: String,
    pub type_params: Vec<String>,  // <T, U>
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct EnumItem {
    pub name: String,
    pub type_params: Vec<String>,  // <T>
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Type>, // None for unit variants, Some(types) for tuple variants
}

#[derive(Debug, Clone)]
pub struct ConstItem {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
}

// ─── Types ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Type {
    Named(String),                  // i32, u64, bool, etc.
    Ref { mutable: bool, inner: Box<Type> },  // &T, &mut T
    Array { inner: Box<Type>, len: usize },   // [T; N]
    Slice { inner: Box<Type> },              // [T]
    Generic { name: String, args: Vec<Type> }, // vec128<f32>
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Named(s) => write!(f, "{}", s),
            Type::Ref { mutable, inner } => {
                if *mutable {
                    write!(f, "&mut {}", inner)
                } else {
                    write!(f, "&{}", inner)
                }
            }
            Type::Array { inner, len } => write!(f, "[{}; {}]", inner, len),
            Type::Slice { inner } => write!(f, "[{}]", inner),
            Type::Generic { name, args } => {
                write!(f, "{}<", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ">")
            }
        }
    }
}

// ─── Statements ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    Expr(Expr, bool), // bool = has semicolon
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    Loop(LoopStmt),
    For(ForStmt),
    Break,
    Continue,
    Block(Block),
    Unsafe(Block),
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub var: String,
    pub start: Expr,
    pub end: Expr,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct LetStmt {
    pub name: String,
    pub ty: Option<Type>,
    pub value: Expr,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub cond: Expr,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct LoopStmt {
    pub body: Block,
}

// ─── Expressions ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(u64),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),
    BoolLit(bool),
    Undefined,
    Var(String),

    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Assign(Box<Expr>, Box<Expr>),
    RefExpr {
        mutable: bool,
        operand: Box<Expr>,
    },
    Deref(Box<Expr>),

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    StaticCall {
        type_name: String,
        method: String,
        args: Vec<Expr>,
        pos: String,
    },
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
    },
    PathAccess {
        type_name: String,
        name: String,
    },
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
        pos: String,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },

    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    AsmBlock(AsmBlock),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,                    // _
    Binding { name: String },    // x
    EnumVariant {                // Option::Some(x)
        enum_name: String,
        variant: String,
        bindings: Vec<String>,
    },
    Literal { value: i64 },      // 0, 1, etc.
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg, Not,
}

#[derive(Debug, Clone)]
pub struct AsmBlock {
    pub instructions: Vec<String>,
    pub outputs: Vec<(String, String)>, // (constraint, var)
    pub inputs: Vec<(String, String)>,
}
