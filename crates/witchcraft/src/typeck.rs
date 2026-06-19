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
        TypeExpr::List { elem, lo, hi, .. } => Ok(Type::List {
            elem: Box::new(resolve_type(elem, types)?),
            lo: *lo,
            hi: *hi,
        }),
    }
}

struct Checker {
    types: HashMap<String, Type>,
    scopes: Vec<Vec<(String, Type)>>,
    /// Capabilities each `fn` declares it requires of its callers. Built before
    /// checking so call-site requirements resolve regardless of definition order.
    fn_requires: HashMap<String, Vec<Capability>>,
    /// Capabilities granted in the context currently being checked. A `fn` body
    /// starts with its declared `requires`; a `with grant` region appends to it.
    active_caps: Vec<Capability>,
    /// Oracle name → model id, so `oracle.embed(...)` resolves its space.
    oracles: HashMap<String, String>,
    /// Memory name → scope name, so reads/writes require `scope(<scope>)`.
    memories: HashMap<String, String>,
    /// The familiar currently being checked (for permit-violation diagnostics and
    /// the in-familiar `invoke <oracle>` rule).
    current_familiar: Option<String>,
    diags: Vec<Diagnostic>,
}

impl Checker {
    fn new() -> Self {
        Checker {
            types: HashMap::new(),
            scopes: vec![Vec::new()],
            fn_requires: HashMap::new(),
            active_caps: Vec::new(),
            oracles: HashMap::new(),
            memories: HashMap::new(),
            current_familiar: None,
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
        // Pass 1b: record every function's required-capability set (part of its
        // checked signature) so calls are checked transitively, order-free.
        for item in &prog.items {
            if let Item::Define(f) = item {
                self.fn_requires.insert(f.name.clone(), f.requires.clone());
            }
        }
        // Pass 1c: record oracle model ids and memory scopes so `oracle.embed`
        // resolves its space and memory accesses resolve their scope capability.
        for item in &prog.items {
            match item {
                Item::Stmt(s) => self.collect_resources(std::slice::from_ref(s)),
                Item::Define(f) => self.collect_resources(&f.body),
                Item::Familiar(fam) => self.collect_resources(&fam.body),
                Item::Type(_) => {}
            }
        }
        // Pass 2: check fns, familiars, and top-level statements.
        for item in &prog.items {
            match item {
                Item::Define(f) => self.check_fn(f),
                Item::Familiar(fam) => self.check_familiar(fam),
                Item::Stmt(s) => self.check_stmt(s),
                Item::Type(_) => {}
            }
        }
    }

    fn check_familiar(&mut self, fam: &FamiliarDecl) {
        // The permits ARE the grants: the body is checked with exactly these
        // capabilities active and no others (the bounded boundary, §5.4).
        let saved_caps = std::mem::replace(&mut self.active_caps, fam.permits.clone());
        let saved_fam = self.current_familiar.replace(fam.name.clone());
        self.push();
        for p in &fam.params {
            let ty =
                p.ty.as_ref()
                    .and_then(|t| resolve_type(t, &self.types).ok())
                    .unwrap_or(Type::Unknown);
            self.define(&p.name, ty);
        }
        // §10 firebreak: a familiar is single-pass in v0.1 — no free-running loop.
        self.reject_unbounded_iteration(&fam.body, &fam.name);
        self.check_block(&fam.body);
        self.pop();
        self.current_familiar = saved_fam;
        self.active_caps = saved_caps;
    }

    /// A familiar body may not contain an unbounded loop in v0.1 (single-pass).
    fn reject_unbounded_iteration(&mut self, stmts: &[Stmt], fam: &str) {
        for s in stmts {
            match s {
                Stmt::While { span, .. } => {
                    self.diags.push(Diagnostic::type_error(
                        format!(
                            "familiar `{}` may not contain an unbounded loop (it is single-pass in v0.1; no declared finite stopping condition)",
                            fam
                        ),
                        *span,
                    ));
                }
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.reject_unbounded_iteration(then_branch, fam);
                    if let Some(eb) = else_branch {
                        self.reject_unbounded_iteration(eb, fam);
                    }
                }
                Stmt::Within { body, .. } | Stmt::Grant { body, .. } => {
                    self.reject_unbounded_iteration(body, fam)
                }
                Stmt::Enact { arms, .. } => {
                    for a in arms {
                        self.reject_unbounded_iteration(&a.body, fam);
                    }
                }
                _ => {}
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

    fn check_fn(&mut self, f: &DefineDecl) {
        self.push();
        for p in &f.params {
            let t = match &p.ty {
                Some(te) => resolve_type(te, &self.types).unwrap_or(Type::Unknown),
                None => Type::Unknown,
            };
            self.define(&p.name, t);
        }
        // The body is checked as if the declared capabilities are granted: the
        // `requires` clause is the obligation the body discharges onto callers.
        let saved = std::mem::replace(&mut self.active_caps, f.requires.clone());
        self.check_block(&f.body);
        self.active_caps = saved;
        self.pop();
    }

    /// Recursively record oracle model ids and memory scopes (both are name-bound
    /// resources whose identity the checker needs regardless of definition order).
    fn collect_resources(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Summon { name, model, .. } => {
                    self.oracles.insert(name.clone(), model.clone());
                }
                Stmt::MemoryDecl(m) => {
                    if let Some(scope) = &m.scope {
                        self.memories.insert(m.name.clone(), scope.clone());
                    }
                }
                Stmt::While { body, .. } | Stmt::Within { body, .. } => {
                    self.collect_resources(body)
                }
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.collect_resources(then_branch);
                    if let Some(eb) = else_branch {
                        self.collect_resources(eb);
                    }
                }
                Stmt::Grant { body, .. } => self.collect_resources(body),
                Stmt::Enact { arms, .. } => {
                    for a in arms {
                        self.collect_resources(&a.body);
                    }
                }
                _ => {}
            }
        }
    }

    /// Is a capability currently granted in the active context?
    fn cap_granted(&self, cap: &Capability) -> bool {
        self.active_caps.iter().any(|c| c.same(cap))
    }

    /// Check that calling an operation is permitted: every capability it requires
    /// must be granted in the active context, or it is a compile-time error.
    fn check_call_caps(&mut self, callee: &str, span: crate::span::Span) {
        let required = match self.fn_requires.get(callee) {
            Some(r) if !r.is_empty() => r.clone(),
            _ => return,
        };
        for cap in &required {
            if !self.cap_granted(cap) {
                self.diags
                    .push(Diagnostic::type_error(self.cap_error(callee, cap), span));
            }
        }
    }

    /// Phrase a missing-capability error, naming the enclosing familiar (a
    /// permit-violation) when one is being checked.
    fn cap_error(&self, op: &str, cap: &Capability) -> String {
        match &self.current_familiar {
            Some(fam) => format!(
                "permit violation: familiar `{}` performs `{}`, which requires `{}` — not in its `permits`",
                fam,
                op,
                cap.display()
            ),
            None => format!(
                "`{}` requires capability `{}`, which is not granted in this context",
                op,
                cap.display()
            ),
        }
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
            Stmt::Speak { value, .. } => {
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
            Stmt::MemoryDecl(m) => {
                if m.scope.is_none() {
                    self.diags.push(Diagnostic::type_error(
                        format!("memory `{}` must declare a `scope`", m.name),
                        m.span,
                    ));
                }
            }
            // `within <scope>` grants the scope capability to its body only.
            Stmt::Within { scope, body, .. } => {
                self.active_caps.push(Capability {
                    kind: "scope".to_string(),
                    param: Some(scope.clone()),
                    span: crate::span::Span::new(0, 0),
                });
                self.check_block(body);
                self.active_caps.pop();
            }
            // A grant region adds its capabilities to the active context for the
            // enclosed body only; they do not leak past the region.
            Stmt::Grant { caps, body, .. } => {
                let added = caps.len();
                self.active_caps.extend(caps.iter().cloned());
                self.check_block(body);
                self.active_caps.truncate(self.active_caps.len() - added);
            }
            Stmt::Expr(e) => {
                let _ = self.infer(e);
            }
        }
    }

    fn check_divine(&mut self, d: &DivineStmt) {
        let out = resolve_type(&d.out_ty, &self.types).unwrap_or(Type::Unknown);
        if !matches!(out, Type::Unknown) {
            if let Err(diag) = crate::grammar::compile(&out, false, d.out_ty.span()) {
                self.diags.push(diag);
            }
        }
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
        // Inside a familiar, invoking an oracle requires the `invoke <oracle>`
        // permit (the bounded boundary; ambient code outside a familiar is
        // unrestricted, preserving the bootstrap divine semantics).
        if self.current_familiar.is_some() {
            let cap = Capability {
                kind: "invoke".to_string(),
                param: Some(d.oracle.clone()),
                span: d.oracle_span,
            };
            if !self.cap_granted(&cap) {
                self.diags.push(Diagnostic::type_error(
                    self.cap_error(&format!("divine using {}", d.oracle), &cap),
                    d.oracle_span,
                ));
            }
        }
        for input in &d.inputs {
            let _ = self.infer(input);
        }
        if let Some(fb) = &d.fallback {
            self.check_against(fb, &out);
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
        if let (Expr::Record { fields, span }, Type::Record(_)) = (e, expected) {
            self.check_record_literal(fields, expected, *span);
            return;
        }
        if let (Expr::List { items, span }, Type::List { elem, lo, hi }) = (e, expected) {
            if let (Some(lo), Some(hi)) = (lo, hi) {
                let n = items.len() as f64;
                if n < *lo || n > *hi {
                    self.diags.push(Diagnostic::type_error(
                        format!(
                            "list literal length {} is outside bounds {}",
                            crate::value::fmt_num(n),
                            expected.display()
                        ),
                        *span,
                    ));
                }
            }
            for it in items {
                self.check_against(it, elem);
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

    fn check_record_literal(
        &mut self,
        fields: &[(String, Expr)],
        expected: &Type,
        span: crate::span::Span,
    ) {
        let Type::Record(expected_fields) = expected else {
            return;
        };
        for (name, expr) in fields {
            match expected_fields.iter().find(|(n, _)| n == name) {
                Some((_, fty)) => self.check_against(expr, fty),
                None => self.diags.push(Diagnostic::type_error(
                    format!("unknown field `{}` in record literal", name),
                    expr.span(),
                )),
            }
        }
        for (name, _) in expected_fields {
            if !fields.iter().any(|(n, _)| n == name) {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "missing field `{}` in record literal for type {}",
                        name,
                        expected.display()
                    ),
                    span,
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
            Expr::Call {
                callee, args, span, ..
            } => {
                let arg_types: Vec<Type> = args.iter().map(|a| self.infer(a)).collect();
                if let Some(t) = self.infer_builtin(callee, &arg_types, *span) {
                    return t;
                }
                self.check_call_caps(callee, *span);
                Type::Unknown
            }
            Expr::Method {
                recv,
                method,
                args,
                span,
            } => {
                let recv_ty = self.infer(recv);
                for a in args {
                    let _ = self.infer(a);
                }
                // `oracle.embed(...)` -> embedding@<oracle space>.
                if method == "embed" {
                    if let Type::Oracle = recv_ty {
                        let space = match recv.as_ref() {
                            Expr::Ident(name, _) => self.oracles.get(name).cloned(),
                            _ => None,
                        };
                        return match space {
                            Some(s) => Type::Embedding(s),
                            None => Type::Embedding("<oracle>".to_string()),
                        };
                    }
                }
                // Governed-memory access requires the memory's scope capability.
                if let Expr::Ident(mem_name, _) = recv.as_ref() {
                    if let Some(scope) = self.memories.get(mem_name).cloned() {
                        return self.check_memory_access(mem_name, &scope, method, *span);
                    }
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
            Expr::Record { fields, .. } => {
                let mut out = Vec::with_capacity(fields.len());
                for (n, fe) in fields {
                    out.push((n.clone(), self.infer(fe)));
                }
                Type::Record(out)
            }
            Expr::List { items, .. } => {
                let mut elem = Type::Unknown;
                for it in items {
                    let t = self.infer(it);
                    if elem == Type::Unknown {
                        elem = t;
                    }
                }
                Type::List {
                    elem: Box::new(elem),
                    lo: None,
                    hi: None,
                }
            }
        }
    }

    /// Type built-in operations (`similarity`, `nearest`). Returns `None` for a
    /// non-builtin name so normal call checking proceeds. The space-typed
    /// embedding rules live here: cross-space comparison is a compile error.
    fn infer_builtin(
        &mut self,
        callee: &str,
        args: &[Type],
        span: crate::span::Span,
    ) -> Option<Type> {
        match callee {
            "similarity" => {
                if let [a, b] = args {
                    self.require_same_space(a, b, span);
                }
                Some(Type::spark())
            }
            "nearest" => {
                // nearest(query: embedding@S, candidates: [embedding@S], k) -> [embedding@S]
                if let [query, candidates, _k] = args {
                    let cand_elem = match candidates {
                        Type::List { elem, .. } => (**elem).clone(),
                        _ => Type::Unknown,
                    };
                    self.require_same_space(query, &cand_elem, span);
                    let space = match query {
                        Type::Embedding(s) => Some(s.clone()),
                        _ => match &cand_elem {
                            Type::Embedding(s) => Some(s.clone()),
                            _ => None,
                        },
                    };
                    return Some(Type::List {
                        elem: Box::new(match space {
                            Some(s) => Type::Embedding(s),
                            None => Type::Unknown,
                        }),
                        lo: None,
                        hi: None,
                    });
                }
                Some(Type::List {
                    elem: Box::new(Type::Unknown),
                    lo: None,
                    hi: None,
                })
            }
            // Logical-clock and audit affordances for governed memory (v0.x).
            "advance" => Some(Type::Unit),
            "audit_log" => Some(Type::List {
                elem: Box::new(Type::Glyph),
                lo: None,
                hi: None,
            }),
            "listen" => {
                if args.len() != 1 {
                    self.diags.push(Diagnostic::type_error(
                        "`listen` takes one glyph prompt argument".to_string(),
                        span,
                    ));
                }
                Some(Type::Glyph)
            }
            _ => None,
        }
    }

    /// A governed-memory access (`mem.write`/`mem.recent`) requires the memory's
    /// scope capability in context, or it is an out-of-scope compile error.
    fn check_memory_access(
        &mut self,
        mem: &str,
        scope: &str,
        method: &str,
        span: crate::span::Span,
    ) -> Type {
        let cap = Capability {
            kind: "scope".to_string(),
            param: Some(scope.to_string()),
            span,
        };
        if !self.cap_granted(&cap) {
            self.diags.push(Diagnostic::type_error(
                format!(
                    "out-of-scope access to memory `{}`: `scope({})` not granted (enter `within {}`)",
                    mem, scope, scope
                ),
                span,
            ));
        }
        match method {
            "recent" | "nearest" => Type::List {
                elem: Box::new(Type::Unknown),
                lo: None,
                hi: None,
            },
            _ => Type::Unit,
        }
    }

    /// Emit a cross-space error if both types are embeddings of differing spaces.
    fn require_same_space(&mut self, a: &Type, b: &Type, span: crate::span::Span) {
        if let (Type::Embedding(sa), Type::Embedding(sb)) = (a, b) {
            if sa != sb {
                self.diags.push(Diagnostic::type_error(
                    format!(
                        "cannot compare embeddings of different spaces `{}` and `{}` (no implicit cross-space bridge)",
                        sa, sb
                    ),
                    span,
                ));
            }
        }
    }
}
