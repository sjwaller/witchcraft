//! Abstract syntax tree for the v0.1 surface.

use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Fn(FnDecl),
    Type(TypeDecl),
    Stmt(Stmt),
}

#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

/// A type as written in source.
#[derive(Clone, Debug)]
pub enum TypeExpr {
    /// `spark`, `glyph`, `oracle`, or a user-declared type name.
    Named(String, Span),
    /// `spark in lo..hi`
    Refined {
        base: String,
        lo: f64,
        hi: f64,
        span: Span,
    },
    /// `{ field: T, ... }`
    Record(Vec<(String, TypeExpr)>, Span),
    /// `one_of { A, B(field: T), ... }`
    OneOf(Vec<VariantDef>, Span),
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(_, s)
            | TypeExpr::Refined { span: s, .. }
            | TypeExpr::Record(_, s)
            | TypeExpr::OneOf(_, s) => *s,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<(String, TypeExpr)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    Var {
        name: String,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    Print {
        value: Expr,
        span: Span,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    If {
        cond: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    /// `oracle name = summon "model-id"`
    Summon {
        name: String,
        model: String,
        span: Span,
    },
    Divine(DivineStmt),
    Enact {
        subject: Expr,
        arms: Vec<EnactArm>,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Expr(Expr),
}

/// `divine name: OutType from (inputs) using oracle with confidence >= θ fallback E`
#[derive(Clone, Debug)]
pub struct DivineStmt {
    pub name: String,
    pub out_ty: TypeExpr,
    pub inputs: Vec<Expr>,
    pub oracle: String,
    pub oracle_span: Span,
    /// Discharge clause is grammar-optional; the type system enforces discharge.
    pub threshold: Option<f64>,
    pub fallback: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnactArm {
    pub variant: String,
    pub bindings: Vec<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64, Span),
    Bool(bool, Span),
    /// A `glyph` literal made of literal text and interpolated expressions.
    Str(Vec<StrSeg>, Span),
    Ident(String, Span),
    Unary {
        op: UnOp,
        rhs: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// `recv.method(args)` — e.g. `oracle.embed(...)` (reserved for later) and field access reuse.
    Method {
        recv: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    Field {
        recv: Box<Expr>,
        field: String,
        span: Span,
    },
    /// `Variant(field: expr, ...)` or `Variant`
    Variant {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s)
            | Expr::Bool(_, s)
            | Expr::Str(_, s)
            | Expr::Ident(_, s)
            | Expr::Unary { span: s, .. }
            | Expr::Binary { span: s, .. }
            | Expr::Call { span: s, .. }
            | Expr::Method { span: s, .. }
            | Expr::Field { span: s, .. }
            | Expr::Variant { span: s, .. } => *s,
        }
    }
}

#[derive(Clone, Debug)]
pub enum StrSeg {
    Lit(String),
    Interp(Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}
