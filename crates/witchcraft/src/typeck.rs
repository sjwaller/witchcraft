//! The static type checker — where nativeness is enforced. It verifies only
//! STRUCTURAL properties (refinement bounds, variant validity, the discharge
//! rule, `enact` exhaustiveness). A successful check is never a claim that an
//! inferred value is correct (§8); that wording lives in the CLI.

use std::collections::HashMap;

use crate::ast::*;
use crate::error::Diagnostic;
use crate::types::{Type, Variant};

pub fn check_program(prog: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut checker = Checker::new();
    checker.run(prog);
    if checker.diags.is_empty() {
        Ok(())
    } else {
        Err(checker.diags)
    }
}

/// Build the declared-type table (best effort; resolution errors are surfaced by
/// the full checker, not here).
pub fn build_type_table(prog: &Program) -> HashMap<String, Type> {
    let mut table = HashMap::new();
    for item in &prog.items {
        if let Item::Type(td) = item {
            if let Ok(t) = resolve_type(&td.ty, &table) {
                table.insert(td.name.clone(), t);
            }
        }
    }
    table
}

/// Resolve a written type into a resolved `Type` against the declared-type table.
pub fn resolve_type(te: &TypeExpr, types: &HashMap<String, Type>) -> Result<Type, Diagnostic> {
    match te {
        TypeExpr::Named(name, span) => match name.as_str() {
            "spark" => Ok(Type::spark()),
            "glyph" => Ok(Type::Glyph),
            "bool" => Ok(Type::Bool),
            "oracle" => Ok(Type::Oracle),
            other => types
                .get(other)
                .cloned()
                .ok_or_else(|| Diagnostic::type_error(format!("unknown type `{}`", other), *span)),
        },
        TypeExpr::Refined { base, lo, hi, span } => {
            if base == "spark" {
                Ok(Type::Spark {
                    lo: Some(*lo),
                    hi: Some(*hi),
                })
            } else {
                Err(Diagnostic::type_error(
                    format!("only `spark` can be refined with a range, not `{}`", base),
                    *span,
                ))
            }
        }
        TypeExpr::Record(fields, _) => {
            let mut out = Vec::new();
            for (n, t) in fields {
                out.push((n.clone(), resolve_type(t, types)?));
            }
            Ok(Type::Record(out))
        }
        TypeExpr::OneOf(variants, _) => {
            let mut out = Vec::new();
            for v in variants {
                let mut fields = Vec::new();
                for (n, t) in &v.fields {
                    fields.push((n.clone(), resolve_type(t, types)?));
                }
                out.push(Variant {
                    name: v.name.clone(),
                    fields,
                });
            }
            Ok(Type::Sum(out))
        }
    }
}

struct Checker {
    types: HashMap<String, Type>,
    scopes: Vec<Vec<(String, Type)>>,
    diags: Vec<Diagnostic>,
}

impl Checker {
    fn new() -> Self {
        Checker {
            types: HashMap::new(),
            scopes: vec![Vec::new()],
            diags: Vec::new(),
        }
    }

    fn run(&mut self, prog: &Program) {
        // Pass 1: register declared types in source order.
        for item in &prog.items {
            if let Item::Type(td) = item {
                match resolve_type(&td.ty, &self.types) {
                    Ok(t) => {
                        self.types.insert(td.name.clone(), t);
                    }
                    Err(d) => self.diags.push(d),
                }
            }
        }
        // Pass 2: check fns and top-level statements.
        for item in &prog.items {
            match item {
                Item::Fn(f) => self.check_fn(f),
                Item::Stmt(s) => self.check_stmt(s),
                Item::Type(_) => {}
            }
        }
    }

    fn push(&mut self) {
        self.scopes.push(Vec::new());
    }
    fn pop(&mut self) {
        self.scopes.pop();
    }
    fn define(&mut self, name: &str, ty: Type) {
        self.scopes.last_mut().unwrap().push((name.to_string(), ty));
    }
    fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some((_, t)) = scope.iter().rev().find(|(n, _)| n == name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn check_fn(&mut self, f: &FnDecl) {
        self.push();
        for p in &f.params {
            let t = match &p.ty {
                Some(te) => resolve_type(te, &self.types).unwrap_or(Type::Unknown),
                None => Type::Unknown,
            };
            self.define(&p.name, t);
        }
        self.check_block(&f.body);
        self.pop();
    }

    fn check_block(&mut self, body: &[Stmt]) {
        self.push();
        for s in body {
            self.check_stmt(s);
        }
        self.pop();
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let {
                name, ty, value, ..
            }
            | Stmt::Var {
                name, ty, value, ..
            } => {
                let bound = match ty {
                    Some(te) => match resolve_type(te, &self.types) {
                        Ok(t) => {
                            self.check_against(value, &t);
                            t
                        }
                        Err(d) => {
                            self.diags.push(d);
                            Type::Unknown
                        }
                    },
                    None => self.infer(value),
                };
                self.define(name, bound);
            }
            Stmt::Assign { value, .. } => {
                let _ = self.infer(value);
            }
            Stmt::Print { value, .. } => {
                let _ = self.infer(value);
            }
            Stmt::While { cond, body, .. } => {
                let _ = self.infer(cond);
                self.check_block(body);
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let _ = self.infer(cond);
                self.check_block(then_branch);
                if let Some(eb) = else_branch {
                    self.check_block(eb);
                }
            }
            Stmt::Summon { name, .. } => {
                self.define(name, Type::Oracle);
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    let _ = self.infer(v);
                }
            }
            Stmt::Divine(d) => self.check_divine(d),
            Stmt::Enact {
                subject,
                arms,
                span,
            } => self.check_enact(subject, arms, *span),
            Stmt::Expr(e) => {
                let _ = self.infer(e);
            }
        }
    }

    fn check_divine(&mut self, d: &DivineStmt) {
        let out = resolve_type(&d.out_ty, &self.types).unwrap_or(Type::Unknown);
        // The oracle must be an oracle value.
        match self.lookup(&d.oracle) {
            Some(Type::Oracle) | Some(Type::Unknown) => {}
            Some(other) => self.diags.push(Diagnostic::type_error(
                format!("`{}` is a {}, not an oracle", d.oracle, other.display()),
                d.oracle_span,
            )),
            None => self.diags.push(Diagnostic::type_error(
                format!("unknown oracle `{}`", d.oracle),
                d.oracle_span,
            )),
        }
        for input in &d.inputs {
            let _ = self.infer(input);
        }
        if let Some(fb) = &d.fallback {
            let _ = self.infer(fb);
        }
        // Discharge present -> plain T; absent -> Inferred<T> (must be discharged later).
        let bound = if d.threshold.is_some() {
            out
        } else {
            Type::Inferred(Box::new(out))
        };
        self.define(&d.name, bound);
    }

    fn check_enact(&mut self, subject: &Expr, arms: &[EnactArm], span: crate::span::Span) {
        let st = self.infer(subject);
        let variants = match st {
            Type::Inferred(_) => {
                self.diags.push(Diagnostic::type_error(
                    "this value is inferred and must be discharged (with confidence >= …) before `enact`",
                    subject.span(),
                ));
                return;
            }
            Type::Sum(v) => v,
            Type::Unknown => return, // host-dynamic; nothing structural to check
            other => {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "`enact` requires a variant (one_of) value, found {}",
                        other.display()
                    ),
                    subject.span(),
                ));
                return;
            }
        };

        // Exhaustiveness: arms must cover exactly the declared variants.
        for arm in arms {
            if !variants.iter().any(|v| v.name == arm.variant) {
                self.diags.push(Diagnostic::type_error(
                    format!("unknown variant `{}` in `enact`", arm.variant),
                    arm.span,
                ));
            }
        }
        for v in &variants {
            if !arms.iter().any(|a| a.variant == v.name) {
                self.diags.push(Diagnostic::type_error(
                    format!("non-exhaustive `enact`: missing variant `{}`", v.name),
                    span,
                ));
            }
        }

        // Check each arm body with its bindings in scope.
        for arm in arms {
            self.push();
            if let Some(v) = variants.iter().find(|v| v.name == arm.variant) {
                for (i, b) in arm.bindings.iter().enumerate() {
                    let t = v
                        .fields
                        .get(i)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(Type::Unknown);
                    self.define(b, t);
                }
            }
            for s in &arm.body {
                self.check_stmt(s);
            }
            self.pop();
        }
    }

    /// Directed check of an expression against an expected type, with clear
    /// messages for refinements, variants, and the discharge rule.
    fn check_against(&mut self, e: &Expr, expected: &Type) {
        // Refinement bound check on a numeric literal.
        if let (Expr::Number(n, span), Type::Spark { lo, hi }) = (e, expected) {
            let lo_ok = lo.is_none_or(|l| *n >= l);
            let hi_ok = hi.is_none_or(|h| *n <= h);
            if !(lo_ok && hi_ok) {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "value {} is outside the refinement bound {}",
                        crate::value::fmt_num(*n),
                        expected.display()
                    ),
                    *span,
                ));
            }
            return;
        }
        // Variant against an expected sum type.
        if let (Expr::Variant { name, fields, span }, Type::Sum(variants)) = (e, expected) {
            match variants.iter().find(|v| &v.name == name) {
                None => self.diags.push(Diagnostic::type_error(
                    format!("unknown variant `{}` for type {}", name, expected.display()),
                    *span,
                )),
                Some(v) => {
                    for (i, (_, fty)) in v.fields.iter().enumerate() {
                        if let Some((_, fe)) = fields.get(i) {
                            self.check_against(fe, fty);
                        }
                    }
                }
            }
            return;
        }
        let actual = self.infer(e);
        if !actual.assignable_to(expected) {
            // Distinguish the discharge case for a clearer message.
            if matches!(actual, Type::Inferred(_)) && !matches!(expected, Type::Inferred(_)) {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "inferred value must be discharged (with confidence >= …) before use as {}",
                        expected.display()
                    ),
                    e.span(),
                ));
            } else {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "expected {}, found {}",
                        expected.display(),
                        actual.display()
                    ),
                    e.span(),
                ));
            }
        }
    }

    fn infer(&mut self, e: &Expr) -> Type {
        match e {
            Expr::Number(n, _) => Type::Spark {
                lo: Some(*n),
                hi: Some(*n),
            },
            Expr::Bool(_, _) => Type::Bool,
            Expr::Str(segs, _) => {
                for seg in segs {
                    if let StrSeg::Interp(inner) = seg {
                        let _ = self.infer(inner);
                    }
                }
                Type::Glyph
            }
            Expr::Ident(name, _) => self.lookup(name).unwrap_or(Type::Unknown),
            Expr::Unary { op, rhs, .. } => {
                let _ = self.infer(rhs);
                match op {
                    UnOp::Neg => Type::spark(),
                    UnOp::Not => Type::Bool,
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let _ = self.infer(lhs);
                let _ = self.infer(rhs);
                use BinOp::*;
                match op {
                    Add | Sub | Mul | Div => Type::spark(),
                    Lt | Le | Gt | Ge | Eq | Ne | And | Or => Type::Bool,
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    let _ = self.infer(a);
                }
                Type::Unknown
            }
            Expr::Method { recv, args, .. } => {
                let _ = self.infer(recv);
                for a in args {
                    let _ = self.infer(a);
                }
                Type::Unknown
            }
            Expr::Field { recv, field, span } => {
                let rt = self.infer(recv);
                match rt {
                    Type::Inferred(_) => {
                        self.diags.push(Diagnostic::type_error(
                            "this value is inferred and must be discharged before its fields are read",
                            *span,
                        ));
                        Type::Unknown
                    }
                    Type::Record(fields) => fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(Type::Unknown),
                    _ => Type::Unknown,
                }
            }
            Expr::Variant { .. } => Type::Unknown,
        }
    }
}
