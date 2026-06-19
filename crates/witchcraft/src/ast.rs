//! Abstract syntax tree for the v0.1 surface.

use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Define(DefineDecl),
    Type(TypeDecl),
    Familiar(FamiliarDecl),
    Stmt(Stmt),
}

/// A `familiar` — a bounded, named composite (explicitly NOT a primitive, §5.5).
/// Its `permits` set is the elevation-worthy, checkable capability boundary: the
/// body is granted exactly these capabilities and no others.
#[derive(Clone, Debug)]
pub struct FamiliarDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub permits: Vec<Capability>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct DefineDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    /// Capabilities this function requires of its callers (`requires <cap>, ...`).
    /// Compile-time only; erased before lowering.
    pub requires: Vec<Capability>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// A capability identity: a `kind` plus an optional `param`
/// (e.g. `permit(escalate)`, `scope(tenant)`). Two capabilities are the same
/// only when both kind and param match.
#[derive(Clone, Debug)]
pub struct Capability {
    pub kind: String,
    pub param: Option<String>,
    pub span: Span,
}

impl Capability {
    /// Human-readable identity, e.g. `permit(escalate)` or `scope`.
    pub fn display(&self) -> String {
        match &self.param {
            Some(p) => format!("{}({})", self.kind, p),
            None => self.kind.clone(),
        }
    }

    /// Structural equality of identity (ignores span).
    pub fn same(&self, other: &Capability) -> bool {
        self.kind == other.kind && self.param == other.param
    }
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
    Speak {
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
    /// `with grant <caps> { ... }` — grants capabilities to the enclosed region.
    /// Compile-time only; erased to its body before lowering.
    Grant {
        caps: Vec<Capability>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `memory <name> { scope S, retention N unit, retrieval ..., audit ... }`
    MemoryDecl(MemoryDecl),
    /// `within <scope> { ... }` — grants the `scope(<scope>)` capability to the
    /// region (the consuming-primitive grant sugar from capability-effects D1).
    Within {
        scope: String,
        body: Vec<Stmt>,
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

/// A governed-memory declaration. `scope` is required (checked); the rest are
/// governance settings (retention is runtime-enforced, audit is a runtime log).
#[derive(Clone, Debug)]
pub struct MemoryDecl {
    pub name: String,
    pub scope: Option<String>,
    /// `(amount, unit)` — the unit is cosmetic; retention is enforced in logical
    /// ticks (see the interpreter's logical clock).
    pub retention: Option<(f64, String)>,
    pub retrieval: Vec<String>,
    pub audit_required: bool,
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
    /// `[a, b, c]` — a homogeneous list literal (used by embedding/memory retrieval).
    List {
        items: Vec<Expr>,
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
            | Expr::Variant { span: s, .. }
            | Expr::List { span: s, .. } => *s,
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
