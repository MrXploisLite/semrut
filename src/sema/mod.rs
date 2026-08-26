use std::collections::HashMap;
use std::fmt;

use crate::parser::ast::*;
use thiserror::Error;

// ─── Errors ───────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SemaError {
    #[error("undefined variable `{name}` at {pos}")]
    UndefinedVar { name: String, pos: String },

    #[error("undefined function `{name}` at {pos}")]
    UndefinedFn { name: String, pos: String },

    #[error("undefined type `{name}` at {pos}")]
    UndefinedType { name: String, pos: String },

    #[error("type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },

    #[error("duplicate definition `{name}`")]
    DuplicateDef { name: String },

    #[error("wrong number of arguments: expected {expected}, got {got}")]
    WrongArgCount { expected: usize, got: usize },

    #[error("cannot assign to immutable variable `{name}`")]
    AssignToImmutable { name: String },

    #[error("condition must be bool, got {got}")]
    NonBoolCondition { got: String },

    #[error("return type mismatch: function returns {expected}, but got {got}")]
    ReturnTypeMismatch { expected: String, got: String },

    #[error("operator `{op}` cannot be applied to types {left} and {right}")]
    InvalidBinOp { op: String, left: String, right: String },

    #[error("operator `{op}` cannot be applied to type {operand}")]
    InvalidUnaryOp { op: String, operand: String },

    #[error("field `{field}` not found on type {ty}")]
    FieldNotFound { field: String, ty: String },

    #[error("type `{ty}` is not callable")]
    NotCallable { ty: String },

    #[error("array index must be integer, got {got}")]
    NonIntIndex { got: String },

    #[error("{msg}")]
    Other { msg: String },
}

type Result<T> = std::result::Result<T, SemaError>;

// ─── Checked Types ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    F16, F32, F64,
    Bool,
    Char,
    Str,
    Void,
    Ptr(Box<Ty>),
    Ref { mutable: bool, inner: Box<Ty> },
    Array(Box<Ty>, usize),
    Slice(Box<Ty>),
    Struct(String),
    Enum(String),
    Fn(Vec<Ty>, Box<Ty>),
    Generic(String, Vec<Ty>), // vec128<f32>
    GenericParam(String),     // T, U (type parameters)
    Never,  // undefined, return from diverging
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::I8 => write!(f, "i8"),
            Ty::I16 => write!(f, "i16"),
            Ty::I32 => write!(f, "i32"),
            Ty::I64 => write!(f, "i64"),
            Ty::I128 => write!(f, "i128"),
            Ty::U8 => write!(f, "u8"),
            Ty::U16 => write!(f, "u16"),
            Ty::U32 => write!(f, "u32"),
            Ty::U64 => write!(f, "u64"),
            Ty::U128 => write!(f, "u128"),
            Ty::F16 => write!(f, "f16"),
            Ty::F32 => write!(f, "f32"),
            Ty::F64 => write!(f, "f64"),
            Ty::Bool => write!(f, "bool"),
            Ty::Char => write!(f, "char"),
            Ty::Str => write!(f, "str"),
            Ty::Void => write!(f, "void"),
            Ty::Ptr(inner) => write!(f, "*{}", inner),
            Ty::Ref { mutable, inner } => {
                if *mutable { write!(f, "&mut {}", inner) }
                else { write!(f, "&{}", inner) }
            }
            Ty::Array(inner, len) => write!(f, "[{}; {}]", inner, len),
            Ty::Slice(inner) => write!(f, "[{}]", inner),
            Ty::Struct(name) => write!(f, "{}", name),
            Ty::Enum(name) => write!(f, "{}", name),
            Ty::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Ty::Generic(name, args) => {
                write!(f, "{}<", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", a)?;
                }
                write!(f, ">")
            }
            Ty::GenericParam(name) => write!(f, "{}", name),
            Ty::Never => write!(f, "never"),
        }
    }
}

impl Ty {
    pub fn is_int(&self) -> bool {
        matches!(self,
            Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::I128 |
            Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::U128
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Ty::F16 | Ty::F32 | Ty::F64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_int() || self.is_float()
    }

    /// Can `from` be coerced to `to`?
    pub fn can_coerce(&self, to: &Ty) -> bool {
        if self == to { return true; }
        // int -> float is ok
        if self.is_int() && to.is_float() { return true; }
        // smaller int -> larger int (same sign)
        match (self, to) {
            (Ty::I8, Ty::I16 | Ty::I32 | Ty::I64 | Ty::I128) => true,
            (Ty::I16, Ty::I32 | Ty::I64 | Ty::I128) => true,
            (Ty::I32, Ty::I64 | Ty::I128) => true,
            (Ty::I64, Ty::I128) => true,
            (Ty::U8, Ty::U16 | Ty::U32 | Ty::U64 | Ty::U128) => true,
            (Ty::U16, Ty::U32 | Ty::U64 | Ty::U128) => true,
            (Ty::U32, Ty::U64 | Ty::U128) => true,
            (Ty::U64, Ty::U128) => true,
            // signed -> unsigned (same or larger size)
            (Ty::I8, Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::U128) => true,
            (Ty::I16, Ty::U16 | Ty::U32 | Ty::U64 | Ty::U128) => true,
            (Ty::I32, Ty::U32 | Ty::U64 | Ty::U128) => true,
            (Ty::I64, Ty::U64 | Ty::U128) => true,
            (Ty::Never, _) => true,  // never coerces to anything
            _ => false,
        }
    }
}

// ─── Environment / Scope ─────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)] // fields mirror source surface for diagnostics
pub struct VarInfo {
    pub name: String,
    pub ty: Ty,
    pub mutable: bool,
}

#[derive(Debug, Clone)]
pub struct FnInfo {
    pub name: String,
    pub type_params: Vec<String>,
    /// Trait bounds per type parameter, parallel to `type_params`.
    /// Empty inner vec = unbounded. Used for bound checking at call sites.
    #[allow(dead_code)] // enforced at generic call sites (bound check)
    pub trait_bounds: Vec<Vec<String>>,
    pub params: Vec<(String, Ty)>,
    pub ret_type: Ty,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // type_params used when generic structs land
pub struct StructInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, Ty)>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<Ty>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // type_params used when generic enums land
pub struct EnumInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariantInfo>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // trait infra: wired up with trait solving
pub struct TraitInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub methods: Vec<FnInfo>,
}

#[derive(Debug, Clone)]
pub struct ConstInfo {
    pub name: String,
    pub ty: Ty,
    pub value: i64, // for now, only int consts
}

struct Env {
    scopes: Vec<HashMap<String, VarInfo>>,
    functions: HashMap<String, FnInfo>,
    structs: HashMap<String, StructInfo>,
    enums: HashMap<String, EnumInfo>,
    traits: HashMap<String, TraitInfo>,
    consts: HashMap<String, ConstInfo>,
    type_aliases: HashMap<String, Ty>,
    type_params: Vec<String>, // active type parameters in current scope
    self_type: Option<Ty>, // Self type for impl blocks
}

impl Env {
    fn new() -> Self {
        let mut env = Env {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            consts: HashMap::new(),
            type_aliases: HashMap::new(),
            type_params: Vec::new(),
            self_type: None,
        };

        // Register builtin types
        for name in &["i8","i16","i32","i64","i128","u8","u16","u32","u64","u128",
                       "f16","f32","f64","bool","char","str"] {
            env.type_aliases.insert(name.to_string(), parse_builtin_type(name).unwrap());
        }

        // Register builtin stdlib functions
        env.functions.insert("print".to_string(), FnInfo {
            name: "print".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![("fmt".to_string(), Ty::Str)],
            ret_type: Ty::Void,
        });
        env.functions.insert("print_int".to_string(), FnInfo {
            name: "print_int".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![("n".to_string(), Ty::I64)],
            ret_type: Ty::Void,
        });
        env.functions.insert("alloc".to_string(), FnInfo {
            name: "alloc".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![("size".to_string(), Ty::U64)],
            ret_type: Ty::Ptr(Box::new(Ty::U8)),
        });
        env.functions.insert("free".to_string(), FnInfo {
            name: "free".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![("ptr".to_string(), Ty::Ptr(Box::new(Ty::U8)))],
            ret_type: Ty::Void,
        });
        env.functions.insert("memcpy".to_string(), FnInfo {
            name: "memcpy".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![
                ("dst".to_string(), Ty::Ptr(Box::new(Ty::U8))),
                ("src".to_string(), Ty::Ptr(Box::new(Ty::U8))),
                ("n".to_string(), Ty::U64),
            ],
            ret_type: Ty::Ptr(Box::new(Ty::U8)),
        });
        env.functions.insert("memset".to_string(), FnInfo {
            name: "memset".to_string(),
            type_params: Vec::new(),
            trait_bounds: Vec::new(),
            params: vec![
                ("dst".to_string(), Ty::Ptr(Box::new(Ty::U8))),
                ("c".to_string(), Ty::I32),
                ("n".to_string(), Ty::U64),
            ],
            ret_type: Ty::Ptr(Box::new(Ty::U8)),
        });

        env
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn add_var(&mut self, name: String, ty: Ty, mutable: bool) -> Result<()> {
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(&name) {
            return Err(SemaError::DuplicateDef { name });
        }
        scope.insert(name.clone(), VarInfo { name: name.clone(), ty, mutable });
        Ok(())
    }

    fn lookup_var(&self, name: &str) -> Option<VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info.clone());
            }
        }
        None
    }

    fn add_fn(&mut self, info: FnInfo) -> Result<()> {
        if self.functions.contains_key(&info.name) {
            return Err(SemaError::DuplicateDef { name: info.name });
        }
        self.functions.insert(info.name.clone(), info);
        Ok(())
    }

    fn lookup_fn(&self, name: &str) -> Option<FnInfo> {
        self.functions.get(name).cloned()
    }

    fn add_struct(&mut self, info: StructInfo) -> Result<()> {
        if self.structs.contains_key(&info.name) {
            return Err(SemaError::DuplicateDef { name: info.name });
        }
        self.structs.insert(info.name.clone(), info);
        Ok(())
    }

    fn lookup_struct(&self, name: &str) -> Option<StructInfo> {
        self.structs.get(name).cloned()
    }

    fn add_enum(&mut self, info: EnumInfo) -> Result<()> {
        if self.enums.contains_key(&info.name) {
            return Err(SemaError::DuplicateDef { name: info.name });
        }
        self.enums.insert(info.name.clone(), info);
        Ok(())
    }

    fn lookup_enum(&self, name: &str) -> Option<EnumInfo> {
        self.enums.get(name).cloned()
    }

    fn add_trait(&mut self, info: TraitInfo) -> Result<()> {
        if self.traits.contains_key(&info.name) {
            return Err(SemaError::DuplicateDef { name: info.name });
        }
        self.traits.insert(info.name.clone(), info);
        Ok(())
    }

    fn add_const(&mut self, info: ConstInfo) -> Result<()> {
        if self.consts.contains_key(&info.name) {
            return Err(SemaError::DuplicateDef { name: info.name });
        }
        self.consts.insert(info.name.clone(), info);
        Ok(())
    }

    fn lookup_const(&self, name: &str) -> Option<ConstInfo> {
        self.consts.get(name).cloned()
    }

    fn resolve_type(&mut self, ty: &Type) -> Result<Ty> {
        match ty {
            Type::Named(s) => {
                if s == "Self" {
                    if let Some(ref st) = self.self_type {
                        return Ok(st.clone());
                    }
                }
                // Check if it's a type parameter
                if self.type_params.contains(s) {
                    return Ok(Ty::GenericParam(s.clone()));
                }
                if let Some(builtin) = parse_builtin_type(s) {
                    return Ok(builtin);
                }
                if let Some(alias) = self.type_aliases.get(s) {
                    return Ok(alias.clone());
                }
                if self.structs.contains_key(s) {
                    return Ok(Ty::Struct(s.clone()));
                }
                if self.enums.contains_key(s) {
                    return Ok(Ty::Enum(s.clone()));
                }
                Err(SemaError::UndefinedType {
                    name: s.clone(),
                    pos: "unknown".to_string(),
                })
            }
            Type::Ref { mutable, inner } => {
                let inner_ty = self.resolve_type(inner)?;
                Ok(Ty::Ref {
                    mutable: *mutable,
                    inner: Box::new(inner_ty),
                })
            }
            Type::Array { inner, len } => {
                let inner_ty = self.resolve_type(inner)?;
                Ok(Ty::Array(Box::new(inner_ty), *len))
            }
            Type::Slice { inner } => {
                let inner_ty = self.resolve_type(inner)?;
                Ok(Ty::Slice(Box::new(inner_ty)))
            }
            Type::Generic { name, args } => {
                let resolved_args: Vec<Ty> = args.iter()
                    .map(|a| self.resolve_type(a))
                    .collect::<Result<Vec<_>>>()?;
                // A generic struct written out with concrete arguments
                // (`Box<i32>`) names the specialized layout the struct literal
                // registers, so both sides agree on one concrete type.
                if let Some(info) = self.structs.get(name).cloned() {
                    if info.type_params.len() == resolved_args.len() {
                        let suffix: String = resolved_args
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join("_");
                        let mono = format!("{}__{}", name, suffix);
                        if !self.structs.contains_key(&mono) {
                            let subst: HashMap<String, Ty> = info
                                .type_params
                                .iter()
                                .cloned()
                                .zip(resolved_args.iter().cloned())
                                .collect();
                            let fields = info
                                .fields
                                .iter()
                                .map(|(n, t)| (n.clone(), self.resolve_type_with_subst(t, &subst)))
                                .collect();
                            self.structs.insert(
                                mono.clone(),
                                StructInfo {
                                    name: mono.clone(),
                                    type_params: Vec::new(),
                                    fields,
                                },
                            );
                        }
                        return Ok(Ty::Struct(mono));
                    }
                }
                Ok(Ty::Generic(name.clone(), resolved_args))
            }
        }
    }

    /// Does `ty` implement `trait_name`? Looks for a registered impl method
    /// mangled as "TraitName::TypeName". Trait impls register their methods
    /// that way in Pass 1, so this is the authoritative impl table.
    fn type_impls_trait(&self, ty: &Ty, trait_name: &str) -> bool {
        let ty_name = match ty {
            Ty::Struct(n) | Ty::Enum(n) => n.clone(),
            Ty::I8 => "i8".to_string(),
            Ty::I16 => "i16".to_string(),
            Ty::I32 => "i32".to_string(),
            Ty::I64 => "i64".to_string(),
            Ty::U8 => "u8".to_string(),
            Ty::U16 => "u16".to_string(),
            Ty::U32 => "u32".to_string(),
            Ty::U64 => "u64".to_string(),
            Ty::F32 => "f32".to_string(),
            Ty::F64 => "f64".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Char => "char".to_string(),
            _ => return false,
        };
        self.lookup_fn(&format!("{}::{}", trait_name, ty_name)).is_some()
    }

    // Resolve type with substitution map for monomorphization
    fn resolve_type_with_subst(&self, ty: &Ty, subst: &HashMap<String, Ty>) -> Ty {
        match ty {
            Ty::GenericParam(name) => {
                subst.get(name).cloned().unwrap_or(ty.clone())
            }
            Ty::Ptr(inner) => Ty::Ptr(Box::new(self.resolve_type_with_subst(inner, subst))),
            Ty::Ref { mutable, inner } => Ty::Ref {
                mutable: *mutable,
                inner: Box::new(self.resolve_type_with_subst(inner, subst)),
            },
            Ty::Array(inner, len) => Ty::Array(
                Box::new(self.resolve_type_with_subst(inner, subst)),
                *len,
            ),
            Ty::Slice(inner) => Ty::Slice(Box::new(self.resolve_type_with_subst(inner, subst))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|p| self.resolve_type_with_subst(p, subst)).collect(),
                Box::new(self.resolve_type_with_subst(ret, subst)),
            ),
            Ty::Generic(name, args) => Ty::Generic(
                name.clone(),
                args.iter().map(|a| self.resolve_type_with_subst(a, subst)).collect(),
            ),
            _ => ty.clone(),
        }
    }
}

fn parse_builtin_type(name: &str) -> Option<Ty> {
    match name {
        "i8" => Some(Ty::I8),
        "i16" => Some(Ty::I16),
        "i32" => Some(Ty::I32),
        "i64" => Some(Ty::I64),
        "i128" => Some(Ty::I128),
        "u8" => Some(Ty::U8),
        "u16" => Some(Ty::U16),
        "u32" => Some(Ty::U32),
        "u64" => Some(Ty::U64),
        "u128" => Some(Ty::U128),
        "f16" => Some(Ty::F16),
        "f32" => Some(Ty::F32),
        "f64" => Some(Ty::F64),
        "bool" => Some(Ty::Bool),
        "char" => Some(Ty::Char),
        "str" => Some(Ty::Str),
        _ => None,
    }
}

// ─── Checked Program ─────────────────────────────────────

#[allow(dead_code)] // enums/consts flow to codegen in a later pass
pub struct CheckedProgram {
    pub functions: Vec<CheckedFn>,
    pub structs: Vec<CheckedStruct>,
    pub enums: Vec<CheckedEnum>,
    pub consts: Vec<CheckedConst>,
    pub impls: Vec<CheckedImpl>,
}

#[derive(Clone)]
pub struct CheckedImpl {
    pub trait_name: Option<String>,
    pub target_type: Ty,
    pub methods: Vec<CheckedFn>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct CheckedFn {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<(String, Ty)>,
    pub ret_type: Ty,
    pub body: CheckedBlock,
    pub is_pub: bool,
}

#[derive(Clone)]
pub struct CheckedBlock {
    pub stmts: Vec<CheckedStmt>,
}

#[derive(Clone)]
pub enum CheckedStmt {
    Let {
        name: String,
        ty: Ty,
        value: Box<CheckedExpr>,
        /// Used by the borrow checker once assignment tracking lands.
        #[allow(dead_code)]
        mutable: bool,
    },
    /// The bool records statement position for diagnostics.
    Expr(Box<CheckedExpr>, #[allow(dead_code)] bool),
    Return(Option<Box<CheckedExpr>>),
    If {
        cond: Box<CheckedExpr>,
        then_block: CheckedBlock,
        else_block: Option<CheckedBlock>,
    },
    While {
        cond: Box<CheckedExpr>,
        body: CheckedBlock,
    },
    Loop {
        body: CheckedBlock,
    },
    For {
        var: String,
        start: Box<CheckedExpr>,
        end: Box<CheckedExpr>,
        body: CheckedBlock,
    },
    Block(CheckedBlock),
    Unsafe(CheckedBlock),
    Break,
    Continue,
}

#[derive(Clone)]
pub enum CheckedExpr {
    IntLit(i64, Ty),
    FloatLit(f64, Ty),
    StringLit(String),
    CharLit(char),
    BoolLit(bool),
    Undefined(Ty),
    Var(String, Ty),
    Binary {
        op: BinOp,
        left: Box<CheckedExpr>,
        right: Box<CheckedExpr>,
        result_ty: Ty,
    },
    Unary {
        op: UnaryOp,
        operand: Box<CheckedExpr>,
        result_ty: Ty,
    },
    Assign {
        target: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    RefExpr {
        mutable: bool,
        operand: Box<CheckedExpr>,
        result_ty: Ty,
    },
    Deref {
        operand: Box<CheckedExpr>,
        result_ty: Ty,
    },
    Call {
        callee: Box<CheckedExpr>,
        args: Vec<CheckedExpr>,
        result_ty: Ty,
        /// Set for generic calls after monomorphization planning:
        /// the specialized function name this call resolves to.
        mono_name: Option<String>,
    },
    MethodCall {
        receiver: Box<CheckedExpr>,
        /// Original (unmangled) name, used in diagnostics.
        #[allow(dead_code)]
        method: String,
        mangled: String,
        args: Vec<CheckedExpr>,
        result_ty: Ty,
    },
    StaticCall {
        type_name: String,
        method: String,
        args: Vec<CheckedExpr>,
        result_ty: Ty,
        #[allow(dead_code)]
        pos: String,
    },
    FieldAccess {
        receiver: Box<CheckedExpr>,
        field: String,
        result_ty: Ty,
    },
    PathAccess {
        type_name: String,
        name: String,
        result_ty: Ty,
    },
    StructLit {
        name: String,
        fields: Vec<(String, CheckedExpr)>,
        result_ty: Ty,
        #[allow(dead_code)]
        pos: String,
    },
    Index {
        target: Box<CheckedExpr>,
        index: Box<CheckedExpr>,
        result_ty: Ty,
    },
    AsmBlock {
        instructions: Vec<String>,
        outputs: Vec<(String, String)>,
        inputs: Vec<(String, String)>,
    },
    /// Constructed once explicit `as` casts are checked separately.
    #[allow(dead_code)]
    Cast {
        expr: Box<CheckedExpr>,
        target_ty: Ty,
    },
    Match {
        scrutinee: Box<CheckedExpr>,
        arms: Vec<CheckedMatchArm>,
        result_ty: Ty,
    },
}

#[derive(Clone)]
pub struct CheckedMatchArm {
    pub pattern: CheckedPattern,
    pub guard: Option<CheckedExpr>,
    pub body: CheckedExpr,
}

#[derive(Clone)]
pub enum CheckedPattern {
    Wildcard,
    Binding { name: String },
    EnumVariant {
        enum_name: String,
        variant: String,
        bindings: Vec<String>,
    },
    Literal { value: i64 },
}

#[derive(Clone)]
pub struct CheckedStruct {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
}

#[allow(dead_code)] // name/variants consumed by future enum lowering
#[derive(Clone)]
pub struct CheckedEnum {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
}

#[allow(dead_code)] // consts not yet lowered to MIR
#[derive(Clone)]
pub struct CheckedConst {
    pub name: String,
    pub ty: Ty,
    pub value: i64,
}

// ─── Type Checker ─────────────────────────────────────────


impl From<SemaError> for Vec<SemaError> {
    fn from(e: SemaError) -> Self {
        vec![e]
    }
}

pub type CheckResult = std::result::Result<CheckedProgram, Vec<SemaError>>;


/// Monomorphization: clone generic functions into concrete specializations.
///
/// For each generic fn, collect the distinct instantiations referenced by
/// `Call.mono_name` across the program, substitute GenericParam types with the
/// concrete ones in params/ret/body, rename to `<name>__<concrete>`, and emit
/// those. The generic originals are dropped (they were never emitted as real
/// functions anyway — codegen mapped GenericParam to a placeholder).
pub fn monomorphize(program: &mut CheckedProgram) {
    use std::collections::HashMap;

    // Collect instantiations per generic fn from all call sites.
    let mut wanted: HashMap<String, Vec<String>> = HashMap::new(); // fn name -> mono names
    for func in &program.functions {
        collect_mono_names(&func.body, &mut wanted);
    }

    let mut specialized: Vec<CheckedFn> = Vec::new();
    let generics: Vec<(String, Vec<String>, CheckedFn)> = program
        .functions
        .iter()
        .filter(|f| !f.type_params.is_empty())
        .map(|f| (f.name.clone(), f.type_params.clone(), f.clone()))
        .collect();
    for (fname, type_params, func) in &generics {
        let Some(mono_names) = wanted.get(fname) else {
            continue;
        };
        // Distinct suffixes map 1:1 to concrete type tuples; recover them by
        // re-deriving subst from the mono_name suffix.
        for mono in mono_names {
            let suffix = mono
                .strip_prefix(&format!("{}__", fname))
                .unwrap_or_default()
                .to_string();
            let ty_names: Vec<&str> = suffix.split('_').filter(|t| !t.is_empty()).collect();
            if ty_names.len() != type_params.len() {
                continue;
            }
            let subst: HashMap<String, Ty> = type_params
                .iter()
                .zip(ty_names.iter())
                .filter_map(|(tp, tn)| parse_ty_name(tn).map(|t| (tp.clone(), t)))
                .collect();

            let mut spec = func.clone();
            spec.name = mono.clone();
            spec.type_params.clear();
            substitute_fn(&mut spec, &subst);
            specialized.push(spec);
        }
    }

    // Drop generics that were fully specialized; keep any generic never called
    // (they compile to placeholder codegen but preserve semantics for direct refs).
    program.functions.retain(|f| f.type_params.is_empty());
    program.functions.extend(specialized);
}

fn collect_mono_names(block: &CheckedBlock, out: &mut HashMap<String, Vec<String>>) {
    for stmt in &block.stmts {
        collect_stmt(stmt, out);
    }
}

fn collect_stmt(stmt: &CheckedStmt, out: &mut HashMap<String, Vec<String>>) {
    match stmt {
        CheckedStmt::Let { value, .. } => collect_expr(value, out),
        CheckedStmt::Expr(e, _) => collect_expr(e, out),
        CheckedStmt::Return(Some(e)) => collect_expr(e, out),
        CheckedStmt::If { cond, then_block, else_block } => {
            collect_expr(cond, out);
            collect_mono_names(then_block, out);
            if let Some(b) = else_block {
                collect_mono_names(b, out);
            }
        }
        CheckedStmt::While { cond, body } => {
            collect_expr(cond, out);
            collect_mono_names(body, out);
        }
        CheckedStmt::Loop { body } => collect_mono_names(body, out),
        CheckedStmt::For { start, end, body, .. } => {
            collect_expr(start, out);
            collect_expr(end, out);
            collect_mono_names(body, out);
        }
        _ => {}
    }
}

fn collect_expr(expr: &CheckedExpr, out: &mut HashMap<String, Vec<String>>) {
    match expr {
        CheckedExpr::Call { callee, args, mono_name, .. } => {
            if let (CheckedExpr::Var(name, _), Some(mono)) = (callee.as_ref(), mono_name) {
                let entry = out.entry(name.clone()).or_default();
                if !entry.contains(mono) {
                    entry.push(mono.clone());
                }
            }
            for a in args {
                collect_expr(a, out);
            }
        }
        CheckedExpr::Binary { left, right, .. } => {
            collect_expr(left, out);
            collect_expr(right, out);
        }
        CheckedExpr::Unary { operand, .. } => collect_expr(operand, out),
        _ => {}
    }
}

fn parse_ty_name(name: &str) -> Option<Ty> {
    match name {
        "i8" => Some(Ty::I8),
        "i16" => Some(Ty::I16),
        "i32" => Some(Ty::I32),
        "i64" => Some(Ty::I64),
        "i128" => Some(Ty::I128),
        "u8" => Some(Ty::U8),
        "u16" => Some(Ty::U16),
        "u32" => Some(Ty::U32),
        "u64" => Some(Ty::U64),
        "u128" => Some(Ty::U128),
        "f32" => Some(Ty::F32),
        "f64" => Some(Ty::F64),
        "bool" => Some(Ty::Bool),
        "char" => Some(Ty::Char),
        _ => None,
    }
}

fn substitute_fn(func: &mut CheckedFn, subst: &HashMap<String, Ty>) {
    for (_, ty) in &mut func.params {
        *ty = substitute_ty(ty, subst);
    }
    func.ret_type = substitute_ty(&func.ret_type, subst);
    substitute_block(&mut func.body, subst);
}

fn substitute_ty(ty: &Ty, subst: &HashMap<String, Ty>) -> Ty {
    match ty {
        Ty::GenericParam(n) => subst.get(n).cloned().unwrap_or(ty.clone()),
        Ty::Ptr(inner) => Ty::Ptr(Box::new(substitute_ty(inner, subst))),
        Ty::Ref { mutable, inner } => Ty::Ref {
            mutable: *mutable,
            inner: Box::new(substitute_ty(inner, subst)),
        },
        Ty::Array(inner, len) => Ty::Array(Box::new(substitute_ty(inner, subst)), *len),
        Ty::Slice(inner) => Ty::Slice(Box::new(substitute_ty(inner, subst))),
        other => other.clone(),
    }
}

fn substitute_block(block: &mut CheckedBlock, subst: &HashMap<String, Ty>) {
    for stmt in &mut block.stmts {
        substitute_stmt(stmt, subst);
    }
}

fn substitute_stmt(stmt: &mut CheckedStmt, subst: &HashMap<String, Ty>) {
    match stmt {
        CheckedStmt::Let { ty, value, .. } => {
            *ty = substitute_ty(ty, subst);
            substitute_expr(value, subst);
        }
        CheckedStmt::Expr(e, _) => substitute_expr(e, subst),
        CheckedStmt::Return(Some(e)) => substitute_expr(e, subst),
        CheckedStmt::If { cond, then_block, else_block } => {
            substitute_expr(cond, subst);
            substitute_block(then_block, subst);
            if let Some(b) = else_block {
                substitute_block(b, subst);
            }
        }
        CheckedStmt::While { cond, body } => {
            substitute_expr(cond, subst);
            substitute_block(body, subst);
        }
        CheckedStmt::Loop { body } => substitute_block(body, subst),
        CheckedStmt::For { start, end, body, .. } => {
            substitute_expr(start, subst);
            substitute_expr(end, subst);
            substitute_block(body, subst);
        }
        _ => {}
    }
}

fn substitute_expr(expr: &mut CheckedExpr, subst: &HashMap<String, Ty>) {
    match expr {
        CheckedExpr::IntLit(_, ty) | CheckedExpr::FloatLit(_, ty) => {
            *ty = substitute_ty(ty, subst);
        }
        CheckedExpr::Var(_, ty) => *ty = substitute_ty(ty, subst),
        CheckedExpr::Binary { left, right, result_ty, .. } => {
            substitute_expr(left, subst);
            substitute_expr(right, subst);
            *result_ty = substitute_ty(result_ty, subst);
        }
        CheckedExpr::Unary { operand, result_ty, .. } => {
            substitute_expr(operand, subst);
            *result_ty = substitute_ty(result_ty, subst);
        }
        CheckedExpr::Call { callee, args, result_ty, .. } => {
            substitute_expr(callee, subst);
            for a in args {
                substitute_expr(a, subst);
            }
            *result_ty = substitute_ty(result_ty, subst);
        }
        CheckedExpr::Undefined(ty) => {
            *ty = substitute_ty(ty, subst);
        }
        _ => {}
    }
}

pub fn check(program: &Program) -> CheckResult {
    let mut env = Env::new();

    // Pass 1: Register all top-level items
    for item in &program.items {
        match item {
            Item::Fn(fn_item) => {
                // Set type params for resolution
                env.type_params = fn_item.type_params.iter().map(|tp| tp.name.clone()).collect();
                let params: Vec<(String, Ty)> = fn_item.params.iter()
                    .map(|p| env.resolve_type(&p.ty).map(|ty| (p.name.clone(), ty)))
                    .collect::<Result<Vec<_>>>()?;
                let ret_type = match &fn_item.ret_type {
                    Some(t) => env.resolve_type(t)?,
                    None => Ty::Void,
                };
                env.type_params.clear();
                env.add_fn(FnInfo {
                    name: fn_item.name.clone(),
                    type_params: fn_item.type_params.iter().map(|tp| tp.name.clone()).collect(),
                        trait_bounds: fn_item.type_params.iter().map(|tp| tp.bounds.clone()).collect(),
                    params: params.clone(),
                    ret_type: ret_type.clone(),
                })?;
            }
            Item::Struct(s_item) => {
                env.type_params = s_item.type_params.clone();
                let fields: Vec<(String, Ty)> = s_item.fields.iter()
                    .map(|f| env.resolve_type(&f.ty).map(|ty| (f.name.clone(), ty)))
                    .collect::<Result<Vec<_>>>()?;
                env.type_params.clear();
                env.add_struct(StructInfo {
                    name: s_item.name.clone(),
                    type_params: s_item.type_params.clone(),
                    fields: fields.clone(),
                })?;
            }
            Item::Enum(e_item) => {
                env.type_params = e_item.type_params.clone();
                let variants: Vec<EnumVariantInfo> = e_item.variants.iter()
                    .map(|v| {
                        let fields: Vec<Ty> = v.fields.iter()
                            .map(|f| env.resolve_type(f))
                            .collect::<Result<Vec<_>>>()?;
                        Ok(EnumVariantInfo { name: v.name.clone(), fields })
                    })
                    .collect::<Result<Vec<_>>>()?;
                env.type_params.clear();
                env.add_enum(EnumInfo {
                    name: e_item.name.clone(),
                    type_params: e_item.type_params.clone(),
                    variants,
                })?;
            }
            Item::Const(c_item) => {
                let ty = env.resolve_type(&c_item.ty)?;
                let value = match &c_item.value {
                    Expr::IntLit(n) => *n as i64,
                    _ => 0,
                };
                env.add_const(ConstInfo {
                    name: c_item.name.clone(),
                    ty,
                    value,
                })?;
            }
            Item::Trait(t_item) => {
                env.type_params = t_item.type_params.clone();
                let methods: Vec<FnInfo> = t_item.methods.iter()
                    .map(|m| {
                        let params: Vec<(String, Ty)> = m.params.iter()
                            .map(|p| env.resolve_type(&p.ty).map(|ty| (p.name.clone(), ty)))
                            .collect::<Result<Vec<_>>>()?;
                        let ret_type = match &m.ret_type {
                            Some(t) => env.resolve_type(t)?,
                            None => Ty::Void,
                        };
                        Ok(FnInfo {
                            name: m.name.clone(),
                            type_params: Vec::new(),
                            trait_bounds: Vec::new(),
                            params,
                            ret_type,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                env.type_params.clear();
                env.add_trait(TraitInfo {
                    name: t_item.name.clone(),
                    type_params: t_item.type_params.clone(),
                    methods,
                })?;
            }
            Item::Impl(impl_item) => {
                // Validate target type exists
                let target_ty = env.resolve_type(&impl_item.target_type)?;
                // Set Self type for method resolution
                env.self_type = Some(target_ty.clone());
                // Set type params from impl block
                env.type_params = impl_item.type_params.clone();
                // Register methods
                for method in &impl_item.methods {
                    let params: Vec<(String, Ty)> = method.params.iter()
                        .map(|p| env.resolve_type(&p.ty).map(|ty| (p.name.clone(), ty)))
                        .collect::<Result<Vec<_>>>()?;
                    let ret_type = match &method.ret_type {
                        Some(t) => env.resolve_type(t)?,
                        None => Ty::Void,
                    };
                    // For trait impls, register TWO entries in Pass 1:
                    //   "Trait::Type"    -> impl-table marker for bound checks
                    //   "Trait::method"  -> method body entry (Pass 2 looks this up)
                    //   (the trait declaration already registered the signature;
                    //    overwrite it with the impl's concrete signature)
                    // For inherent impls: mangle as TypeName::method
                    if let Some(ref trait_name) = impl_item.trait_name {
                        env.functions.insert(
                            format!("{}::{}", trait_name, target_ty),
                            FnInfo {
                                name: String::new(),
                                type_params: Vec::new(),
                                trait_bounds: Vec::new(),
                                params: Vec::new(),
                                ret_type: Ty::Void,
                            },
                        );
                    }
                    let mangled = if let Some(ref trait_name) = impl_item.trait_name {
                        format!("{}::{}", trait_name, method.name)
                    } else {
                        format!("{}::{}", target_ty, method.name)
                    };
                    env.add_fn(FnInfo {
                        name: mangled,
                        type_params: impl_item.type_params.clone(),
                        trait_bounds: Vec::new(),
                        params,
                        ret_type,
                    })?;
                }
                env.type_params.clear();
                env.self_type = None;
            }
        }
    }

    // Pass 2: Check each item. Function-level errors are collected so the
    // compiler reports every broken function in one run instead of one at a time.
    let mut functions = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut consts = Vec::new();
    let mut impls = Vec::new();
    let mut deferred_errors: Vec<SemaError> = Vec::new();

    for item in &program.items {
        match item {
            Item::Fn(fn_item) => {
                let fn_info = env.lookup_fn(&fn_item.name).unwrap();
                env.type_params = fn_info.type_params.clone();
                env.push_scope();
                for (name, ty) in &fn_info.params {
                    if let Err(e) = env.add_var(name.clone(), ty.clone(), true) {
                        deferred_errors.push(e);
                        env.pop_scope();
                        continue;
                    }
                }
                match check_block(&mut env, &fn_item.body, &fn_info.ret_type) {
                    Ok(body) => {
                        functions.push(CheckedFn {
                            name: fn_item.name.clone(),
                            type_params: fn_info.type_params.clone(),
                            params: fn_info.params.clone(),
                            ret_type: fn_info.ret_type.clone(),
                            body,
                            is_pub: fn_item.is_pub,
                        });
                    }
                    Err(e) => deferred_errors.push(e),
                }
                env.pop_scope();
                env.type_params.clear();
            }
            Item::Struct(s_item) => {
                let info = env.lookup_struct(&s_item.name).unwrap();
                structs.push(CheckedStruct {
                    name: s_item.name.clone(),
                    fields: info.fields,
                });
            }
            Item::Enum(e_item) => {
                let info = env.lookup_enum(&e_item.name).unwrap();
                enums.push(CheckedEnum {
                    name: e_item.name.clone(),
                    variants: info.variants,
                });
            }
            Item::Const(c_item) => {
                let info = env.lookup_const(&c_item.name).unwrap();
                consts.push(CheckedConst {
                    name: c_item.name.clone(),
                    ty: info.ty,
                    value: info.value,
                });
            }
            Item::Impl(impl_item) => {
                let target_ty = env.resolve_type(&impl_item.target_type)?;
                let mut checked_methods = Vec::new();
                for method in &impl_item.methods {
                    // Pass 1 registered trait impls as "Trait::Type" (impl table)
                    // and the method body under "Trait::method" / "Type::method".
                    let mangled = if let Some(ref trait_name) = impl_item.trait_name {
                        format!("{}::{}", trait_name, method.name)
                    } else {
                        format!("{}::{}", target_ty, method.name)
                    };
                    let fn_info = env.lookup_fn(&mangled).ok_or_else(|| {
                        SemaError::UndefinedFn {
                            name: mangled.clone(),
                            pos: "impl".to_string(),
                        }
                    })?;
                    env.push_scope();
                    for (name, ty) in &fn_info.params {
                        env.add_var(name.clone(), ty.clone(), true)?;
                    }
                    let body = check_block(&mut env, &method.body, &fn_info.ret_type)?;
                    env.pop_scope();
                    checked_methods.push(CheckedFn {
                        name: method.name.clone(),
                        type_params: fn_info.type_params.clone(),
                        params: fn_info.params,
                        ret_type: fn_info.ret_type,
                        body,
                        is_pub: false,
                    });
                }
                impls.push(CheckedImpl {
                    trait_name: impl_item.trait_name.clone(),
                    target_type: target_ty,
                    methods: checked_methods,
                });
            }
            Item::Trait(_t_item) => {
                // Traits are already registered in Pass 1
            }
        }
    }


    if !deferred_errors.is_empty() {
        return Err(deferred_errors);
    }

    // Emit specialized struct layouts created while checking generic struct
    // literals (Box__i32 and friends), so codegen can resolve their types.
    let declared: std::collections::HashSet<String> =
        structs.iter().map(|s: &CheckedStruct| s.name.clone()).collect();
    let mut specialized: Vec<CheckedStruct> = env
        .structs
        .values()
        .filter(|info| !declared.contains(&info.name) && info.name.contains("__"))
        .map(|info| CheckedStruct {
            name: info.name.clone(),
            fields: info.fields.clone(),
        })
        .collect();
    specialized.sort_by(|a, b| a.name.cmp(&b.name));
    structs.extend(specialized);

    Ok(CheckedProgram {
        functions,
        structs,
        enums,
        consts,
        impls,
    })
}

fn check_block(env: &mut Env, block: &Block, expected_ret: &Ty) -> Result<CheckedBlock> {
    let mut stmts = Vec::new();

    for stmt in &block.stmts {
        let checked = check_stmt(env, stmt, expected_ret)?;
        stmts.push(checked);
    }

    Ok(CheckedBlock { stmts })
}

fn check_stmt(env: &mut Env, stmt: &Stmt, expected_ret: &Ty) -> Result<CheckedStmt> {
    match stmt {
        Stmt::Let(let_stmt) => {
            let value = check_expr(env, &let_stmt.value)?;
            let ty = match &let_stmt.ty {
                Some(t) => env.resolve_type(t)?,
                None => get_expr_type(&value)?,
            };

            // Check if value type is compatible
            let value_ty = get_expr_type(&value)?;
            if !value_ty.can_coerce(&ty) {
                return Err(SemaError::TypeMismatch {
                    expected: ty.to_string(),
                    got: value_ty.to_string(),
                });
            }

            // Coerce literal types
            let val_ty2 = get_expr_type(&value)?;
            let value = match &value {
                CheckedExpr::IntLit(n, _) if val_ty2.can_coerce(&ty) => {
                    CheckedExpr::IntLit(*n, ty.clone())
                }
                _ => value,
            };

            env.add_var(let_stmt.name.clone(), ty.clone(), let_stmt.mutable)?;

            Ok(CheckedStmt::Let {
                name: let_stmt.name.clone(),
                ty,
                value: Box::new(value),
                mutable: let_stmt.mutable,
            })
        }

        Stmt::Expr(expr, has_semi) => {
            let checked = check_expr(env, expr)?;
            Ok(CheckedStmt::Expr(Box::new(checked), *has_semi))
        }

        Stmt::Return(ret_stmt) => {
            let checked_value = match &ret_stmt.value {
                Some(e) => {
                    let val = check_expr(env, e)?;
                    let val_ty = get_expr_type(&val)?;
                    if !val_ty.can_coerce(expected_ret) {
                        return Err(SemaError::ReturnTypeMismatch {
                            expected: expected_ret.to_string(),
                            got: val_ty.to_string(),
                        });
                    }
                    // Coerce literal types
                    let val = match &val {
                        CheckedExpr::IntLit(n, t) if t.can_coerce(expected_ret) => {
                            CheckedExpr::IntLit(*n, expected_ret.clone())
                        }
                        _ => val,
                    };
                    Some(Box::new(val))
                }
                None => {
                    if expected_ret != &Ty::Void {
                        return Err(SemaError::ReturnTypeMismatch {
                            expected: expected_ret.to_string(),
                            got: Ty::Void.to_string(),
                        });
                    }
                    None
                }
            };
            Ok(CheckedStmt::Return(checked_value))
        }

        Stmt::If(if_stmt) => {
            let cond = check_expr(env, &if_stmt.cond)?;
            let cond_ty = get_expr_type(&cond)?;
            if cond_ty != Ty::Bool {
                return Err(SemaError::NonBoolCondition { got: cond_ty.to_string() });
            }

            let then_block = check_block(env, &if_stmt.then_block, expected_ret)?;
            let else_block = match &if_stmt.else_block {
                Some(b) => Some(check_block(env, b, expected_ret)?),
                None => None,
            };

            Ok(CheckedStmt::If {
                cond: Box::new(cond),
                then_block,
                else_block,
            })
        }

        Stmt::While(while_stmt) => {
            let cond = check_expr(env, &while_stmt.cond)?;
            let cond_ty = get_expr_type(&cond)?;
            if cond_ty != Ty::Bool {
                return Err(SemaError::NonBoolCondition { got: cond_ty.to_string() });
            }
            let body = check_block(env, &while_stmt.body, expected_ret)?;
            Ok(CheckedStmt::While {
                cond: Box::new(cond),
                body,
            })
        }

        Stmt::Loop(loop_stmt) => {
            let body = check_block(env, &loop_stmt.body, expected_ret)?;
            Ok(CheckedStmt::Loop { body })
        }

        Stmt::For(for_stmt) => {
            let start = check_expr(env, &for_stmt.start)?;
            let end = check_expr(env, &for_stmt.end)?;
            let start_ty = get_expr_type(&start)?;
            env.push_scope();
            env.add_var(for_stmt.var.clone(), start_ty, false)?;
            let body = check_block(env, &for_stmt.body, expected_ret)?;
            env.pop_scope();
            Ok(CheckedStmt::For {
                var: for_stmt.var.clone(),
                start: Box::new(start),
                end: Box::new(end),
                body,
            })
        }

        Stmt::Block(block) => {
            env.push_scope();
            let checked = check_block(env, block, expected_ret)?;
            env.pop_scope();
            Ok(CheckedStmt::Block(checked))
        }

        Stmt::Unsafe(block) => {
            env.push_scope();
            let checked = check_block(env, block, expected_ret)?;
            env.pop_scope();
            Ok(CheckedStmt::Unsafe(checked))
        }

        Stmt::Break => Ok(CheckedStmt::Break),
        Stmt::Continue => Ok(CheckedStmt::Continue),
    }
}

fn check_expr(env: &mut Env, expr: &Expr) -> Result<CheckedExpr> {
    match expr {
        Expr::IntLit(n) => {
            // Default to i32 for integer literals — most common case
            // Will be coerced if context expects different type
            if *n <= i32::MAX as u64 {
                Ok(CheckedExpr::IntLit(*n as i64, Ty::I32))
            } else {
                Ok(CheckedExpr::IntLit(*n as i64, Ty::I64))
            }
        }
        Expr::FloatLit(n) => {
            Ok(CheckedExpr::FloatLit(*n, Ty::F64))
        }
        Expr::StringLit(s) => {
            Ok(CheckedExpr::StringLit(s.clone()))
        }
        Expr::CharLit(c) => {
            Ok(CheckedExpr::CharLit(*c))
        }
        Expr::BoolLit(b) => {
            Ok(CheckedExpr::BoolLit(*b))
        }
        Expr::Undefined => {
            // undefined has "never" type — coerces to anything
            Ok(CheckedExpr::Undefined(Ty::Never))
        }
        Expr::Var(name) => {
            // Check const first
            if let Some(const_info) = env.lookup_const(name) {
                return Ok(CheckedExpr::IntLit(const_info.value, const_info.ty));
            }
            // Then variable
            if let Some(var_info) = env.lookup_var(name) {
                Ok(CheckedExpr::Var(name.clone(), var_info.ty))
            } else {
                Err(SemaError::UndefinedVar {
                    name: name.clone(),
                    pos: "unknown".to_string(),
                })
            }
        }

        Expr::Binary { op, left, right } => {
            let l = check_expr(env, left)?;
            let r = check_expr(env, right)?;
            let l_ty = get_expr_type(&l)?;
            let r_ty = get_expr_type(&r)?;

            let result_ty = check_bin_op(op, &l_ty, &r_ty)?;

            Ok(CheckedExpr::Binary {
                op: op.clone(),
                left: Box::new(l),
                right: Box::new(r),
                result_ty,
            })
        }

        Expr::Unary { op, operand } => {
            let inner = check_expr(env, operand)?;
            let inner_ty = get_expr_type(&inner)?;
            let result_ty = check_unary_op(op, &inner_ty)?;

            Ok(CheckedExpr::Unary {
                op: op.clone(),
                operand: Box::new(inner),
                result_ty,
            })
        }

        Expr::Assign(target, value) => {
            let checked_target = check_expr(env, target)?;
            let checked_value = check_expr(env, value)?;

            // Check mutability
            if let CheckedExpr::Var(name, _) = &checked_target {
                if let Some(var) = env.lookup_var(name) {
                    if !var.mutable {
                        return Err(SemaError::AssignToImmutable { name: name.clone() });
                    }
                }
            }

            let target_ty = get_expr_type(&checked_target)?;
            let value_ty = get_expr_type(&checked_value)?;

            if !value_ty.can_coerce(&target_ty) {
                return Err(SemaError::TypeMismatch {
                    expected: target_ty.to_string(),
                    got: value_ty.to_string(),
                });
            }

            Ok(CheckedExpr::Assign {
                target: Box::new(checked_target),
                value: Box::new(checked_value),
            })
        }

        Expr::RefExpr { mutable, operand } => {
            let inner = check_expr(env, operand)?;
            let inner_ty = get_expr_type(&inner)?;
            Ok(CheckedExpr::RefExpr {
                mutable: *mutable,
                operand: Box::new(inner),
                result_ty: Ty::Ref {
                    mutable: *mutable,
                    inner: Box::new(inner_ty),
                },
            })
        }

        Expr::Deref(operand) => {
            let inner = check_expr(env, operand)?;
            let inner_ty = get_expr_type(&inner)?;
            match inner_ty {
                Ty::Ptr(pointee) | Ty::Ref { inner: pointee, .. } => {
                    Ok(CheckedExpr::Deref {
                        operand: Box::new(inner),
                        result_ty: (*pointee).clone(),
                    })
                }
                _ => Err(SemaError::InvalidUnaryOp {
                    op: "*".to_string(),
                    operand: inner_ty.to_string(),
                }),
            }
        }

        Expr::Call { callee, args } => {
            // Check if callee is a variable — if so, look up as function first
            let mut fn_info = match callee.as_ref() {
                Expr::Var(name) => {
                    // Try function lookup first
                    if let Some(info) = env.lookup_fn(name) {
                        info
                    } else if let Some(var) = env.lookup_var(name) {
                        // It's a variable — check if it's a function type
                        match &var.ty {
                            Ty::Fn(params, ret) => FnInfo {
                                name: name.clone(),
                                type_params: Vec::new(),
                                trait_bounds: Vec::new(),
                                params: params.iter().enumerate()
                                    .map(|(i, ty)| (format!("arg{}", i), ty.clone()))
                                    .collect(),
                                ret_type: (**ret).clone(),
                            },
                            _ => {
                                return Err(SemaError::NotCallable { ty: var.ty.to_string() });
                            }
                        }
                    } else {
                        return Err(SemaError::UndefinedFn {
                            name: name.clone(),
                            pos: "unknown".to_string(),
                        });
                    }
                }
                _ => {
                    let checked_callee = check_expr(env, callee)?;
                    let callee_ty = get_expr_type(&checked_callee)?;
                    return Err(SemaError::NotCallable { ty: callee_ty.to_string() });
                }
            };

            // If function has type parameters, infer them from arguments
            let mut subst: HashMap<String, Ty> = HashMap::new();
            let was_generic = !fn_info.type_params.is_empty();
            if !fn_info.type_params.is_empty() {
                // First, check all arguments to get their types
                let mut arg_tys: Vec<(CheckedExpr, Ty)> = Vec::new();
                for arg in args {
                    let checked_arg = check_expr(env, arg)?;
                    let arg_ty = get_expr_type(&checked_arg)?;
                    arg_tys.push((checked_arg, arg_ty));
                }

                // Infer type parameters from arguments. Untyped int literals are
                // skipped: they default to i64 and would wrongly pin T when the
                // call context wants a narrower type (`let n: i32 = id(40)`).
                // The literal gets coerced to the concrete param type afterwards.
                for (i, (checked_arg, arg_ty)) in arg_tys.iter().enumerate() {
                    if matches!(checked_arg, CheckedExpr::IntLit(_, _)) {
                        continue;
                    }
                    if i < fn_info.params.len() {
                        let param_ty = &fn_info.params[i].1;
                        infer_types(param_ty, arg_ty, &mut subst)?;
                    }
                }

                // Default any still-uninferred params so the check below passes;
                // the concrete value comes from the coerced literal.
                for tp in &fn_info.type_params.clone() {
                    subst.entry(tp.clone()).or_insert(Ty::I32);
                }

                // Enforce declared trait bounds: every concrete type substituted
                // for a bounded param must implement each bound trait.
                for (idx, tp) in fn_info.type_params.iter().enumerate() {
                    let Some(concrete) = subst.get(tp) else { continue };
                    let Some(bounds) = fn_info.trait_bounds.get(idx) else { continue };
                    for b in bounds {
                        if !env.type_impls_trait(concrete, b) {
                            return Err(SemaError::Other {
                                msg: format!(
                                    "`{}` does not implement trait `{}` (required by `{}` bound)",
                                    concrete, b, tp
                                ),
                            });
                        }
                    }
                }

                // Check that all type parameters were inferred
                for tp in &fn_info.type_params {
                    if !subst.contains_key(tp) {
                        return Err(SemaError::Other {
                            msg: format!("cannot infer type parameter `{}`", tp),
                        });
                    }
                }

                // Apply substitution to params and ret_type
                let concrete_params: Vec<(String, Ty)> = fn_info.params.iter()
                    .map(|(n, ty)| (n.clone(), env.resolve_type_with_subst(ty, &subst)))
                    .collect();
                let concrete_ret = env.resolve_type_with_subst(&fn_info.ret_type, &subst);
                fn_info.params = concrete_params;
                fn_info.ret_type = concrete_ret;
            }

            if args.len() != fn_info.params.len() {
                return Err(SemaError::WrongArgCount {
                    expected: fn_info.params.len(),
                    got: args.len(),
                });
            }

            let mut checked_args = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let checked_arg = check_expr(env, arg)?;
                let arg_ty = get_expr_type(&checked_arg)?;
                let expected_ty = &fn_info.params[i].1;
                if !arg_ty.can_coerce(expected_ty) {
                    return Err(SemaError::TypeMismatch {
                        expected: expected_ty.to_string(),
                        got: arg_ty.to_string(),
                    });
                }
                // Coerce int literals to the concrete param type so codegen
                // emits the right width (e.g. display(40) with T = i32).
                let coerced_arg = match (checked_arg, expected_ty) {
                    (CheckedExpr::IntLit(n, _), Ty::I32) => CheckedExpr::IntLit(n, Ty::I32),
                    (CheckedExpr::IntLit(n, _), Ty::I64) => CheckedExpr::IntLit(n, Ty::I64),
                    (other, _) => other,
                };
                checked_args.push(coerced_arg);
            }

            // Plan monomorphization: if the fn was generic, name the
            // specialization after its concrete parameter types.
            let mono_name = if was_generic {
                let suffix: String = fn_info
                    .params
                    .iter()
                    .map(|(_, ty)| ty.to_string())
                    .collect::<Vec<_>>()
                    .join("_");
                Some(format!("{}__{}", fn_info.name, suffix))
            } else {
                None
            };

            Ok(CheckedExpr::Call {
                callee: Box::new(CheckedExpr::Var(fn_info.name.clone(), Ty::Fn(
                    fn_info.params.iter().map(|(_, ty)| ty.clone()).collect(),
                    Box::new(fn_info.ret_type.clone()),
                ))),
                args: checked_args,
                result_ty: fn_info.ret_type.clone(),
                mono_name,
            })
        }

        Expr::MethodCall { receiver, method, args } => {
            let checked_receiver = check_expr(env, receiver)?;
            let receiver_ty = get_expr_type(&checked_receiver)?;

            let lookup_name = match &receiver_ty {
                Ty::Struct(name) => format!("{}::{}", name, method),
                _ => format!("??::{}", method),
            };

            let fn_info = env.lookup_fn(&lookup_name);
            let result_ty = fn_info.as_ref().map(|f| f.ret_type.clone()).unwrap_or(Ty::Void);

            // LLVM-safe name (used by MIR codegen)
            let mangled = match &receiver_ty {
                Ty::Struct(name) => format!("{}_{}", name, method),
                _ => format!("??_{}", method),
            };

            let mut checked_args = Vec::new();
            for arg in args {
                checked_args.push(check_expr(env, arg)?);
            }

            Ok(CheckedExpr::MethodCall {
                receiver: Box::new(checked_receiver),
                method: method.clone(),
                mangled,
                args: checked_args,
                result_ty,
            })
        }

        Expr::FieldAccess { receiver, field } => {
            let checked_receiver = check_expr(env, receiver)?;
            let receiver_ty = get_expr_type(&checked_receiver)?;

            match &receiver_ty {
                Ty::Struct(name) => {
                    if let Some(struct_info) = env.lookup_struct(name) {
                        for (fname, fty) in &struct_info.fields {
                            if fname == field {
                                return Ok(CheckedExpr::FieldAccess {
                                    receiver: Box::new(checked_receiver),
                                    field: field.clone(),
                                    result_ty: fty.clone(),
                                });
                            }
                        }
                    }
                    Err(SemaError::FieldNotFound {
                        field: field.clone(),
                        ty: receiver_ty.to_string(),
                    })
                }
                _ => Err(SemaError::FieldNotFound {
                    field: field.clone(),
                    ty: receiver_ty.to_string(),
                }),
            }
        }

        Expr::StaticCall { type_name, method, args, pos } => {
            // Look up mangled function: TypeName::method
            let mangled = format!("{}::{}", type_name, method);
            let fn_info = env.lookup_fn(&mangled).ok_or_else(|| {
                SemaError::UndefinedFn {
                    name: mangled.clone(),
                    pos: pos.clone(),
                }
            })?;

            let mut checked_args = Vec::new();
            for arg in args {
                checked_args.push(check_expr(env, arg)?);
            }

            // TODO: validate arg types against params

            Ok(CheckedExpr::StaticCall {
                type_name: type_name.clone(),
                method: method.clone(),
                args: checked_args,
                result_ty: fn_info.ret_type.clone(),
                pos: pos.clone(),
            })
        }

        Expr::PathAccess { type_name, name } => {
            // Could be a const, enum variant, etc. For now, treat as unknown.
            // TODO: resolve properly
            Ok(CheckedExpr::PathAccess {
                type_name: type_name.clone(),
                name: name.clone(),
                result_ty: Ty::I64, // placeholder
            })
        }

        Expr::StructLit { name, fields, pos } => {
            let struct_info = env.lookup_struct(name).ok_or_else(|| {
                SemaError::UndefinedType {
                    name: name.clone(),
                    pos: pos.clone(),
                }
            })?;

            // Generic structs: infer type arguments from the field values, then
            // check against the substituted field types. The literal's type
            // becomes the specialized name (Box__i32) so field access and MIR
            // struct lookup work on a concrete layout.
            let mut subst: HashMap<String, Ty> = HashMap::new();
            if !struct_info.type_params.is_empty() {
                for (field_name, field_expr) in fields {
                    let Some((_, declared)) = struct_info
                        .fields
                        .iter()
                        .find(|(n, _)| n == field_name)
                    else {
                        continue;
                    };
                    let checked = check_expr(env, field_expr)?;
                    // Untyped int literals default to i32; let a concrete field
                    // pin the parameter instead of the literal's default.
                    if matches!(checked, CheckedExpr::IntLit(_, _))
                        && matches!(declared, Ty::GenericParam(_))
                    {
                        continue;
                    }
                    let val_ty = get_expr_type(&checked)?;
                    infer_types(declared, &val_ty, &mut subst)?;
                }
                for tp in &struct_info.type_params {
                    subst.entry(tp.clone()).or_insert(Ty::I32);
                }
            }

            let concrete_fields: Vec<(String, Ty)> = struct_info
                .fields
                .iter()
                .map(|(n, ty)| (n.clone(), env.resolve_type_with_subst(ty, &subst)))
                .collect();

            let mut checked_fields = Vec::new();
            for (field_name, field_expr) in fields {
                let checked = check_expr(env, field_expr)?;
                let field_def = concrete_fields.iter().find(|(n, _)| n == field_name);
                match field_def {
                    Some((_, expected_ty)) => {
                        let val_ty = get_expr_type(&checked)?;
                        if !val_ty.can_coerce(expected_ty) {
                            return Err(SemaError::TypeMismatch {
                                expected: expected_ty.to_string(),
                                got: val_ty.to_string(),
                            });
                        }
                        // Coerce literal types
                        let checked = match &checked {
                            CheckedExpr::IntLit(n, _) if val_ty.can_coerce(expected_ty) => {
                                CheckedExpr::IntLit(*n, expected_ty.clone())
                            }
                            _ => checked,
                        };
                        checked_fields.push((field_name.clone(), checked));
                    }
                    None => {
                        return Err(SemaError::FieldNotFound {
                            field: field_name.clone(),
                            ty: name.clone(),
                        });
                    }
                }
            }

            // Register the specialized layout so later field accesses resolve.
            let concrete_name = if struct_info.type_params.is_empty() {
                name.clone()
            } else {
                let suffix: String = struct_info
                    .type_params
                    .iter()
                    .map(|tp| {
                        subst
                            .get(tp)
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "i32".to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("_");
                let mono = format!("{}__{}", name, suffix);
                if env.lookup_struct(&mono).is_none() {
                    env.add_struct(StructInfo {
                        name: mono.clone(),
                        type_params: Vec::new(),
                        fields: concrete_fields.clone(),
                    })?;
                }
                mono
            };

            let result_ty = Ty::Struct(concrete_name.clone());
            Ok(CheckedExpr::StructLit {
                name: concrete_name,
                fields: checked_fields,
                result_ty,
                pos: pos.clone(),
            })
        }

        Expr::Match { scrutinee, arms } => {
            let checked_scrutinee = check_expr(env, scrutinee)?;
            let scrutinee_ty = get_expr_type(&checked_scrutinee)?;

            // First pass: check all arms, determine result type
            let mut prechecked: Vec<(CheckedPattern, Option<CheckedExpr>, CheckedExpr, Ty)> = Vec::new();
            let mut result_ty: Option<Ty> = None;

            for arm in arms {
                let checked_pattern = check_pattern(env, &arm.pattern, &scrutinee_ty)?;
                let checked_guard = match &arm.guard {
                    Some(g) => {
                        let cg = check_expr(env, g)?;
                        let guard_ty = get_expr_type(&cg)?;
                        if guard_ty != Ty::Bool {
                            return Err(SemaError::NonBoolCondition { got: guard_ty.to_string() });
                        }
                        Some(cg)
                    }
                    None => None,
                };
                let checked_body = check_expr(env, &arm.body)?;
                let body_ty = get_expr_type(&checked_body)?;

                // Determine result type - prefer non-literal types
                if let Some(ref rt) = result_ty {
                    if *rt != body_ty {
                        // Try to coerce: if one is a literal int, use the other
                        let can_coerce = matches!(&checked_body, CheckedExpr::IntLit(_, _))
                            || prechecked.last().map(|(_, _, b, _)| matches!(b, CheckedExpr::IntLit(_, _))).unwrap_or(false);
                        if !can_coerce {
                            return Err(SemaError::Other {
                                msg: format!("match arm type mismatch: expected `{}`, got `{}`", rt, body_ty),
                            });
                        }
                        // Keep the non-literal type
                        if !matches!(&checked_body, CheckedExpr::IntLit(_, _)) {
                            result_ty = Some(body_ty.clone());
                        }
                    }
                } else {
                    result_ty = Some(body_ty.clone());
                }

                prechecked.push((checked_pattern, checked_guard, checked_body, body_ty));
            }

            // Second pass: coerce literal ints to result type
            let final_ty = result_ty.unwrap_or(Ty::Void);
            let mut checked_arms = Vec::new();
            for (pattern, guard, body, _body_ty) in prechecked {
                let coerced_body = if let (CheckedExpr::IntLit(n, _), Ty::I64) = (&body, &final_ty) {
                    CheckedExpr::IntLit(*n, Ty::I64)
                } else if let (CheckedExpr::IntLit(n, _), Ty::I32) = (&body, &final_ty) {
                    CheckedExpr::IntLit(*n, Ty::I32)
                } else {
                    body
                };

                checked_arms.push(CheckedMatchArm {
                    pattern,
                    guard,
                    body: coerced_body,
                });
            }

            Ok(CheckedExpr::Match {
                scrutinee: Box::new(checked_scrutinee),
                arms: checked_arms,
                result_ty: final_ty,
            })
        }

        Expr::Index { target, index } => {
            let checked_target = check_expr(env, target)?;
            let checked_index = check_expr(env, index)?;
            let target_ty = get_expr_type(&checked_target)?;
            let index_ty = get_expr_type(&checked_index)?;

            if !index_ty.is_int() {
                return Err(SemaError::NonIntIndex { got: index_ty.to_string() });
            }

            let elem_ty = match target_ty {
                Ty::Array(inner, _) | Ty::Slice(inner) => (*inner).clone(),
                _ => {
                    return Err(SemaError::Other {
                        msg: format!("cannot index type `{}`", target_ty),
                    });
                }
            };

            Ok(CheckedExpr::Index {
                target: Box::new(checked_target),
                index: Box::new(checked_index),
                result_ty: elem_ty,
            })
        }

        Expr::AsmBlock(asm) => {
            Ok(CheckedExpr::AsmBlock {
                instructions: asm.instructions.clone(),
                outputs: asm.outputs.clone(),
                inputs: asm.inputs.clone(),
            })
        }
    }
}

fn int_rank(ty: &Ty) -> u8 {
    match ty {
        Ty::I8 | Ty::U8 => 1,
        Ty::I16 | Ty::U16 => 2,
        Ty::I32 | Ty::U32 => 3,
        Ty::I64 | Ty::U64 => 4,
        Ty::I128 | Ty::U128 => 5,
        _ => 0,
    }
}

fn check_bin_op(op: &BinOp, left: &Ty, right: &Ty) -> Result<Ty> {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            if left.is_numeric() && right.is_numeric() {
                if left.is_float() || right.is_float() {
                    return Ok(Ty::F64);
                }
                // Pick the wider integer type
                if int_rank(left) >= int_rank(right) {
                    return Ok(left.clone());
                } else {
                    return Ok(right.clone());
                }
            }
            Err(SemaError::InvalidBinOp {
                op: format!("{:?}", op),
                left: left.to_string(),
                right: right.to_string(),
            })
        }
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if left.is_numeric() && right.is_numeric() {
                return Ok(Ty::Bool);
            }
            if left == right && (left == &Ty::Bool || left == &Ty::Char) {
                return Ok(Ty::Bool);
            }
            Err(SemaError::InvalidBinOp {
                op: format!("{:?}", op),
                left: left.to_string(),
                right: right.to_string(),
            })
        }
        BinOp::And | BinOp::Or => {
            if left == &Ty::Bool && right == &Ty::Bool {
                return Ok(Ty::Bool);
            }
            Err(SemaError::InvalidBinOp {
                op: format!("{:?}", op),
                left: left.to_string(),
                right: right.to_string(),
            })
        }
    }
}

fn check_unary_op(op: &UnaryOp, operand: &Ty) -> Result<Ty> {
    match op {
        UnaryOp::Neg => {
            if operand.is_numeric() {
                return Ok(operand.clone());
            }
            Err(SemaError::InvalidUnaryOp {
                op: "-".to_string(),
                operand: operand.to_string(),
            })
        }
        UnaryOp::Not => {
            if operand == &Ty::Bool {
                return Ok(Ty::Bool);
            }
            Err(SemaError::InvalidUnaryOp {
                op: "!".to_string(),
                operand: operand.to_string(),
            })
        }
    }
}

// Infer type parameters by matching a generic type against a concrete type
fn infer_types(generic: &Ty, concrete: &Ty, subst: &mut HashMap<String, Ty>) -> Result<()> {
    match (generic, concrete) {
        (Ty::GenericParam(name), _) => {
            if let Some(existing) = subst.get(name) {
                if existing != concrete {
                    return Err(SemaError::Other {
                        msg: format!("type parameter `{}` inferred as both `{}` and `{}`", name, existing, concrete),
                    });
                }
            } else {
                subst.insert(name.clone(), concrete.clone());
            }
            Ok(())
        }
        (Ty::Ptr(g_inner), Ty::Ptr(c_inner)) => infer_types(g_inner, c_inner, subst),
        (Ty::Ref { mutable: gm, inner: g_inner }, Ty::Ref { mutable: cm, inner: c_inner }) => {
            if gm != cm {
                return Err(SemaError::Other {
                    msg: "ref mutability mismatch".to_string(),
                });
            }
            infer_types(g_inner, c_inner, subst)
        }
        (Ty::Array(g_inner, gl), Ty::Array(c_inner, cl)) => {
            if gl != cl {
                return Err(SemaError::Other {
                    msg: "array length mismatch".to_string(),
                });
            }
            infer_types(g_inner, c_inner, subst)
        }
        (Ty::Slice(g_inner), Ty::Slice(c_inner)) => infer_types(g_inner, c_inner, subst),
        (Ty::Generic(g_name, g_args), Ty::Generic(c_name, c_args)) => {
            if g_name != c_name || g_args.len() != c_args.len() {
                return Err(SemaError::Other {
                    msg: format!("generic type mismatch: {} vs {}", generic, concrete),
                });
            }
            for (g, c) in g_args.iter().zip(c_args.iter()) {
                infer_types(g, c, subst)?;
            }
            Ok(())
        }
        _ => {
            // Concrete types must match exactly
            if generic != concrete {
                Err(SemaError::TypeMismatch {
                    expected: generic.to_string(),
                    got: concrete.to_string(),
                })
            } else {
                Ok(())
            }
        }
    }
}

fn check_pattern(env: &mut Env, pattern: &crate::parser::ast::Pattern, scrutinee_ty: &Ty) -> Result<CheckedPattern> {
    match pattern {
        crate::parser::ast::Pattern::Wildcard => Ok(CheckedPattern::Wildcard),
        crate::parser::ast::Pattern::Binding { name } => {
            // Bind the variable in current scope
            env.add_var(name.clone(), scrutinee_ty.clone(), true)?;
            Ok(CheckedPattern::Binding { name: name.clone() })
        }
        crate::parser::ast::Pattern::EnumVariant { enum_name, variant, bindings } => {
            // Resolve enum name if empty (inferred from scrutinee type)
            let enum_name = if enum_name.is_empty() {
                match scrutinee_ty {
                    Ty::Enum(name) => name.clone(),
                    _ => {
                        return Err(SemaError::Other {
                            msg: format!("cannot match variant on non-enum type `{}`", scrutinee_ty),
                        });
                    }
                }
            } else {
                enum_name.clone()
            };

            let enum_info = env.lookup_enum(&enum_name).ok_or_else(|| {
                SemaError::UndefinedType {
                    name: enum_name.clone(),
                    pos: "pattern".to_string(),
                }
            })?;

            let variant_info = enum_info.variants.iter().find(|v| v.name == *variant).ok_or_else(|| {
                SemaError::Other {
                    msg: format!("variant `{}` not found in enum `{}`", variant, enum_name),
                }
            })?;

            if bindings.len() != variant_info.fields.len() {
                return Err(SemaError::Other {
                    msg: format!(
                        "variant `{}` has {} fields, but pattern has {} bindings",
                        variant, variant_info.fields.len(), bindings.len()
                    ),
                });
            }

            // Bind each variable with its corresponding field type
            for (bind, field_ty) in bindings.iter().zip(&variant_info.fields) {
                if bind != "_" {
                    env.add_var(bind.clone(), field_ty.clone(), true)?;
                }
            }

            Ok(CheckedPattern::EnumVariant {
                enum_name,
                variant: variant.clone(),
                bindings: bindings.clone(),
            })
        }
        crate::parser::ast::Pattern::Literal { value } => {
            Ok(CheckedPattern::Literal { value: *value })
        }
    }
}

fn get_expr_type(expr: &CheckedExpr) -> Result<Ty> {
    match expr {
        CheckedExpr::IntLit(_, ty) => Ok(ty.clone()),
        CheckedExpr::FloatLit(_, ty) => Ok(ty.clone()),
        CheckedExpr::StringLit(_) => Ok(Ty::Str),
        CheckedExpr::CharLit(_) => Ok(Ty::Char),
        CheckedExpr::BoolLit(_) => Ok(Ty::Bool),
        CheckedExpr::Undefined(ty) => Ok(ty.clone()),
        CheckedExpr::Var(_, ty) => Ok(ty.clone()),
        CheckedExpr::Binary { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::Unary { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::Assign { value, .. } => get_expr_type(value),
        CheckedExpr::RefExpr { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::Deref { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::Call { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::MethodCall { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::StaticCall { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::FieldAccess { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::PathAccess { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::Index { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::StructLit { result_ty, .. } => Ok(result_ty.clone()),
        CheckedExpr::AsmBlock { .. } => Ok(Ty::Void),
        CheckedExpr::Cast { target_ty, .. } => Ok(target_ty.clone()),
        CheckedExpr::Match { result_ty, .. } => Ok(result_ty.clone()),
    }
}
