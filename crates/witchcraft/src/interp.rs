//! Tree-walking interpreter. Assumes the program already type-checked, but still
//! reports runtime faults (undefined names, bad operands) as diagnostics rather
//! than panicking. Inference goes through the `Decoder` seam; nothing here
//! touches the network.

use std::collections::HashMap;

use crate::ast::*;
use crate::engine::mock::MockEngine;
use crate::engine::{Engine, InferRequest, Policy};
use crate::env::{AssignError, DefineError, Env};
use crate::error::Diagnostic;
use crate::grammar;
use crate::manifest::Manifest;
use crate::typeck::{build_type_table, resolve_type};
use crate::types::Type;
use crate::value::{Provenance, Value};

#[derive(Clone, Debug, Default)]
pub struct RunConfig {
    pub seed: u64,
    /// Litmus knob: when true, every `divine` output type is weakened to free
    /// text (as if the type were deleted), changing what is generated.
    pub weaken_divine: bool,
    /// Fault-injection knob: when set, every discharge sees this confidence.
    pub force_confidence: Option<f64>,
    /// Deployment binding (change `add-inference-runtime`). When present, each
    /// oracle's intent is resolved to a concrete engine at load; when absent the
    /// deterministic Mock engine serves every need (the offline default).
    pub manifest: Option<Manifest>,
}

enum Exec {
    /// Statement produced a value (an expression statement) or unit.
    Value(Value),
    /// Control flow is unwinding to the enclosing function/program (explicit
    /// `return`, or a `divine` fallback firing).
    Return(Value),
}

pub fn run(prog: &Program, config: RunConfig) -> Result<String, Diagnostic> {
    let mut interp = Interp::new(prog, config);
    interp.resolve_engines(prog)?;
    interp.run_program(prog)?;
    Ok(interp.out)
}

struct Interp {
    env: Env,
    fns: HashMap<String, DefineDecl>,
    familiars: HashMap<String, FamiliarDecl>,
    types: HashMap<String, Type>,
    /// The Mock engine that serves every need when no manifest is present (the
    /// offline default). One shared instance preserves the single deterministic
    /// decode sequence the runtime mirrors.
    default_engine: Box<dyn Engine>,
    /// Engines resolved from the manifest at load, keyed by oracle intent.
    engines: HashMap<String, Box<dyn Engine>>,
    config: RunConfig,
    out: String,
    /// Governed-memory stores, a logical clock (for deterministic retention), and
    /// an audit log. Memory is an in-memory v0.x runtime resource (D2).
    memories: HashMap<String, MemoryStore>,
    clock: u64,
    audit: Vec<String>,
}

struct MemoryStore {
    scope: String,
    retention_ticks: Option<f64>,
    audit_required: bool,
    entries: Vec<(u64, Value)>,
}

impl Interp {
    fn new(prog: &Program, config: RunConfig) -> Self {
        let mut fns = HashMap::new();
        let mut familiars = HashMap::new();
        for item in &prog.items {
            match item {
                Item::Define(f) => {
                    fns.insert(f.name.clone(), f.clone());
                }
                Item::Familiar(fam) => {
                    familiars.insert(fam.name.clone(), fam.clone());
                }
                _ => {}
            }
        }
        let default_engine: Box<dyn Engine> = Box::new(MockEngine::new(config.seed, ""));
        Interp {
            env: Env::new(),
            fns,
            familiars,
            types: build_type_table(prog),
            default_engine,
            engines: HashMap::new(),
            config,
            out: String::new(),
            memories: HashMap::new(),
            clock: 0,
            audit: Vec::new(),
        }
    }

    /// Load-time engine resolution (change `add-inference-runtime`). For each
    /// oracle intent the program summons, resolve a concrete engine from the
    /// manifest under the intent's policy. A need that cannot be satisfied makes
    /// the program refuse to start. With no manifest, the default Mock serves
    /// every need and nothing is resolved here.
    fn resolve_engines(&mut self, prog: &Program) -> Result<(), Diagnostic> {
        let manifest = match &self.config.manifest {
            Some(m) => m.clone(),
            None => return Ok(()),
        };
        let policies = oracle_policies(prog);
        for (intent, (policy, span)) in policies {
            match manifest.resolve(&intent, &policy, self.config.seed) {
                Ok(engine) => {
                    self.engines.insert(intent, engine);
                }
                Err(e) => return Err(Diagnostic::runtime(e.message(), span)),
            }
        }
        Ok(())
    }

    fn run_program(&mut self, prog: &Program) -> Result<(), Diagnostic> {
        for item in &prog.items {
            if let Item::Stmt(s) = item {
                if let Exec::Return(_) = self.exec_stmt(s)? {
                    break;
                }
            }
        }
        Ok(())
    }

    fn emit(&mut self, line: &str) {
        self.out.push_str(line);
        self.out.push('\n');
    }

    fn exec_block(&mut self, stmts: &[Stmt]) -> Result<Exec, Diagnostic> {
        self.env.push();
        let mut last = Value::Unit;
        for s in stmts {
            match self.exec_stmt(s)? {
                Exec::Return(v) => {
                    self.env.pop();
                    return Ok(Exec::Return(v));
                }
                Exec::Value(v) => last = v,
            }
        }
        self.env.pop();
        Ok(Exec::Value(last))
    }

    fn exec_stmt(&mut self, s: &Stmt) -> Result<Exec, Diagnostic> {
        match s {
            Stmt::Let {
                name, value, span, ..
            } => {
                let v = self.eval(value)?;
                self.bind(name, v, false, *span)?;
                Ok(Exec::Value(Value::Unit))
            }
            Stmt::Var {
                name, value, span, ..
            } => {
                let v = self.eval(value)?;
                self.bind(name, v, true, *span)?;
                Ok(Exec::Value(Value::Unit))
            }
            Stmt::Assign { name, value, span } => {
                let v = self.eval(value)?;
                match self.env.assign(name, v) {
                    Ok(()) => Ok(Exec::Value(Value::Unit)),
                    Err(AssignError::Undefined) => Err(Diagnostic::runtime(
                        format!("cannot assign to undefined variable `{}`", name),
                        *span,
                    )),
                    Err(AssignError::Immutable) => Err(Diagnostic::runtime(
                        format!("cannot reassign `let` binding `{}` (use `var`)", name),
                        *span,
                    )),
                }
            }
            Stmt::Speak { value, .. } => {
                let v = self.eval(value)?;
                self.emit(&v.display());
                Ok(Exec::Value(Value::Unit))
            }
            Stmt::While { cond, body, span } => {
                loop {
                    let c = self.eval(cond)?;
                    match c {
                        Value::Bool(true) => {}
                        Value::Bool(false) => break,
                        other => {
                            return Err(Diagnostic::runtime(
                                format!(
                                    "`while` condition must be bool, found {}",
                                    other.type_name()
                                ),
                                *span,
                            ))
                        }
                    }
                    if let Exec::Return(v) = self.exec_block(body)? {
                        return Ok(Exec::Return(v));
                    }
                }
                Ok(Exec::Value(Value::Unit))
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => {
                let c = self.eval(cond)?;
                match c {
                    Value::Bool(true) => self.exec_block(then_branch),
                    Value::Bool(false) => match else_branch {
                        Some(eb) => self.exec_block(eb),
                        None => Ok(Exec::Value(Value::Unit)),
                    },
                    other => Err(Diagnostic::runtime(
                        format!("`if` condition must be bool, found {}", other.type_name()),
                        *span,
                    )),
                }
            }
            Stmt::Summon { name, model, span } => {
                let v = Value::Oracle {
                    name: name.clone(),
                    model: model.clone(),
                };
                self.bind(name, v, false, *span)?;
                Ok(Exec::Value(Value::Unit))
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e)?,
                    None => Value::Unit,
                };
                Ok(Exec::Return(v))
            }
            Stmt::Divine(d) => self.exec_divine(d),
            Stmt::Enact {
                subject,
                arms,
                span,
            } => self.exec_enact(subject, arms, *span),
            // Capabilities are compile-time only; at run time a grant region is
            // an ordinary lexical block.
            Stmt::Grant { body, .. } => self.exec_block(body),
            Stmt::MemoryDecl(m) => {
                self.memories.insert(
                    m.name.clone(),
                    MemoryStore {
                        scope: m.scope.clone().unwrap_or_default(),
                        retention_ticks: m.retention.as_ref().map(|(n, _)| *n),
                        audit_required: m.audit_required,
                        entries: Vec::new(),
                    },
                );
                Ok(Exec::Value(Value::Unit))
            }
            // `within` is a capability grant at compile time; at run time it is an
            // ordinary lexical block.
            Stmt::Within { body, .. } => self.exec_block(body),
            Stmt::Expr(e) => {
                let v = self.eval(e)?;
                Ok(Exec::Value(v))
            }
        }
    }

    fn bind(
        &mut self,
        name: &str,
        value: Value,
        mutable: bool,
        span: crate::span::Span,
    ) -> Result<(), Diagnostic> {
        match self.env.define(name, value, mutable) {
            Ok(()) => Ok(()),
            Err(DefineError::Duplicate) => Err(Diagnostic::runtime(
                format!("`{}` is already defined in this scope", name),
                span,
            )),
        }
    }

    fn exec_divine(&mut self, d: &DivineStmt) -> Result<Exec, Diagnostic> {
        // Evaluate the inputs into a prompt the engine receives. The v0.1 ABI
        // dropped these; the contract threads them through (Break 3).
        let mut parts = Vec::with_capacity(d.inputs.len());
        for input in &d.inputs {
            let v = self.eval(input)?;
            parts.push(v.display());
        }
        let prompt = parts.join("\n");

        let out_ty = resolve_type(&d.out_ty, &self.types)
            .map_err(|e| Diagnostic::runtime(e.message, d.span))?;
        let g = grammar::compile(&out_ty, self.config.weaken_divine, d.span)
            .map_err(|e| Diagnostic::runtime(e.message, d.span))?;

        // The oracle names a semantic intent; the manifest binds it to an engine.
        let intent = match self.env.get(&d.oracle) {
            Some(Value::Oracle { model, .. }) => model.clone(),
            _ => d.oracle.clone(),
        };
        let seed = self.config.seed;
        let policy = Policy::default();
        let req = InferRequest {
            intent_id: &intent,
            input: &prompt,
            grammar: &g,
            policy: &policy,
            seed,
        };

        let engine: &mut dyn Engine = match self.engines.get_mut(&intent) {
            Some(e) => e.as_mut(),
            None => self.default_engine.as_mut(),
        };
        let result = engine.infer(&req);
        let confidence = self.config.force_confidence.unwrap_or(result.confidence);
        let provenance = result.provenance;
        let value = attach_provenance(result.value, &provenance);

        match d.threshold {
            Some(threshold) => {
                if confidence >= threshold {
                    self.bind(&d.name, value, false, d.span)?;
                    Ok(Exec::Value(Value::Unit))
                } else {
                    // Discharge failed: run the fallback and unwind. The inferred
                    // value never flows downstream (the fault-injection guarantee).
                    let fb = match &d.fallback {
                        Some(e) => self.eval(e)?,
                        None => Value::Unit,
                    };
                    Ok(Exec::Return(fb))
                }
            }
            None => {
                // No discharge clause: bind as an inferred value. The type checker
                // forbids authoritative use, so this is observable only via tools.
                let inferred = Value::Inferred {
                    inner: Box::new(value),
                    confidence,
                    provenance,
                };
                self.bind(&d.name, inferred, false, d.span)?;
                Ok(Exec::Value(Value::Unit))
            }
        }
    }

    fn exec_enact(
        &mut self,
        subject: &Expr,
        arms: &[EnactArm],
        span: crate::span::Span,
    ) -> Result<Exec, Diagnostic> {
        let value = self.eval(subject)?;
        let prov = value.provenance().cloned();
        let (name, fields) = match value {
            Value::Variant { name, fields, .. } => (name, fields),
            other => {
                return Err(Diagnostic::runtime(
                    format!(
                        "`enact` expects a variant value, found {}",
                        other.type_name()
                    ),
                    span,
                ))
            }
        };
        let arm = match arms.iter().find(|a| a.variant == name) {
            Some(a) => a,
            None => {
                return Err(Diagnostic::runtime(
                    format!("no `enact` arm for variant `{}`", name),
                    span,
                ))
            }
        };
        self.env.push();
        for (i, b) in arm.bindings.iter().enumerate() {
            let v = fields.get(i).map(|(_, v)| v.clone()).unwrap_or(Value::Unit);
            let _ = self.env.define(b, v, false);
        }
        // Thread provenance into the action so it remains attached at the moment
        // of execution.
        if let Some(p) = &prov {
            let _ = self
                .env
                .define("provenance", Value::Glyph(p.render()), false);
        }
        let mut result = Exec::Value(Value::Unit);
        for s in &arm.body {
            match self.exec_stmt(s)? {
                Exec::Return(v) => {
                    result = Exec::Return(v);
                    break;
                }
                Exec::Value(_) => {}
            }
        }
        self.env.pop();
        Ok(result)
    }

    fn eval(&mut self, e: &Expr) -> Result<Value, Diagnostic> {
        match e {
            Expr::Number(n, _) => Ok(Value::Spark(*n)),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Str(segs, _) => {
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        StrSeg::Lit(t) => s.push_str(t),
                        StrSeg::Interp(inner) => {
                            let v = self.eval(inner)?;
                            s.push_str(&v.display());
                        }
                    }
                }
                Ok(Value::Glyph(s))
            }
            Expr::Ident(name, span) => self
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| Diagnostic::runtime(format!("undefined name `{}`", name), *span)),
            Expr::Unary { op, rhs, span } => {
                let v = self.eval(rhs)?;
                match op {
                    UnOp::Neg => match v {
                        Value::Spark(n) => Ok(Value::Spark(-n)),
                        other => Err(Diagnostic::runtime(
                            format!("cannot negate {}", other.type_name()),
                            *span,
                        )),
                    },
                    UnOp::Not => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        other => Err(Diagnostic::runtime(
                            format!("cannot apply `not` to {}", other.type_name()),
                            *span,
                        )),
                    },
                }
            }
            Expr::Binary { op, lhs, rhs, span } => self.eval_binary(*op, lhs, rhs, *span),
            Expr::Call { callee, args, span } => {
                let mut argv = Vec::new();
                for a in args {
                    argv.push(self.eval(a)?);
                }
                self.call_fn(callee, argv, *span)
            }
            Expr::Method {
                recv,
                method,
                args,
                span,
            } => {
                // Memory accesses resolve by name before evaluating the receiver
                // (a memory is a runtime resource, not an ordinary value binding).
                if let Expr::Ident(name, _) = recv.as_ref() {
                    if self.memories.contains_key(name) {
                        let argv = args
                            .iter()
                            .map(|a| self.eval(a))
                            .collect::<Result<Vec<_>, _>>()?;
                        return self.memory_op(name, method, argv, *span);
                    }
                }
                let r = self.eval(recv)?;
                match (method.as_str(), &r) {
                    ("embed", Value::Oracle { name, model }) => {
                        if args.len() != 1 {
                            return Err(Diagnostic::runtime(
                                "`embed` takes exactly one glyph argument".to_string(),
                                *span,
                            ));
                        }
                        let text = match self.eval(&args[0])? {
                            Value::Glyph(s) => s,
                            other => {
                                return Err(Diagnostic::runtime(
                                    format!("`embed` expects a glyph, found {}", other.type_name()),
                                    *span,
                                ))
                            }
                        };
                        Ok(Value::Embedding {
                            space: model.clone(),
                            vector: embed_vector(&text, model),
                            provenance: Some(Provenance {
                                oracle: name.clone(),
                                model: model.clone(),
                                model_version_or_sha: "mock".to_string(),
                                backend_id: "mock".to_string(),
                                seed: self.config.seed,
                                sampling: "deterministic".to_string(),
                            }),
                        })
                    }
                    _ => Err(Diagnostic::runtime(
                        format!("unknown method `{}` on {}", method, r.type_name()),
                        *span,
                    )),
                }
            }
            Expr::Field { recv, field, span } => {
                let r = self.eval(recv)?;
                let prov = r.provenance().cloned();
                let fields = match &r {
                    Value::Record { fields, .. } | Value::Variant { fields, .. } => fields,
                    other => {
                        return Err(Diagnostic::runtime(
                            format!("cannot read field `{}` of {}", field, other.type_name()),
                            *span,
                        ))
                    }
                };
                match fields.iter().find(|(n, _)| n == field) {
                    Some((_, v)) => Ok(propagate_provenance(v.clone(), prov.as_ref())),
                    None => Err(Diagnostic::runtime(format!("no field `{}`", field), *span)),
                }
            }
            Expr::Variant { name, fields, .. } => {
                let mut fv = Vec::new();
                for (n, fe) in fields {
                    fv.push((n.clone(), self.eval(fe)?));
                }
                Ok(Value::Variant {
                    name: name.clone(),
                    fields: fv,
                    provenance: None,
                })
            }
            Expr::List { items, .. } => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    vs.push(self.eval(it)?);
                }
                Ok(Value::List(vs))
            }
        }
    }

    fn eval_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: crate::span::Span,
    ) -> Result<Value, Diagnostic> {
        use BinOp::*;
        // Short-circuiting logical operators.
        if matches!(op, And | Or) {
            let l = self.eval(lhs)?;
            let lb = as_bool(&l, span)?;
            if op == And && !lb {
                return Ok(Value::Bool(false));
            }
            if op == Or && lb {
                return Ok(Value::Bool(true));
            }
            let r = self.eval(rhs)?;
            return Ok(Value::Bool(as_bool(&r, span)?));
        }

        let l = self.eval(lhs)?;
        let r = self.eval(rhs)?;
        match op {
            Add | Sub | Mul | Div => {
                let a = as_spark(&l, span)?;
                let b = as_spark(&r, span)?;
                let v = match op {
                    Add => a + b,
                    Sub => a - b,
                    Mul => a * b,
                    Div => {
                        if b == 0.0 {
                            return Err(Diagnostic::runtime("division by zero", span));
                        }
                        a / b
                    }
                    _ => unreachable!(),
                };
                Ok(Value::Spark(v))
            }
            Lt | Le | Gt | Ge => {
                let a = as_spark(&l, span)?;
                let b = as_spark(&r, span)?;
                let v = match op {
                    Lt => a < b,
                    Le => a <= b,
                    Gt => a > b,
                    Ge => a >= b,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(v))
            }
            Eq => Ok(Value::Bool(l == r)),
            Ne => Ok(Value::Bool(l != r)),
            And | Or => unreachable!(),
        }
    }

    fn call_fn(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: crate::span::Span,
    ) -> Result<Value, Diagnostic> {
        if let Some(v) = self.call_builtin(name, &args, span)? {
            return Ok(v);
        }
        // Familiars are callable like functions; permits are erased at run time
        // (they are a compile-time boundary). The body runs single-pass.
        if let Some(fam) = self.familiars.get(name).cloned() {
            return self.call_familiar(&fam, args, span);
        }
        let f = match self.fns.get(name) {
            Some(f) => f.clone(),
            None => {
                return Err(Diagnostic::runtime(
                    format!("unknown function `{}`", name),
                    span,
                ))
            }
        };
        if args.len() != f.params.len() {
            return Err(Diagnostic::runtime(
                format!(
                    "function `{}` expects {} argument(s), got {}",
                    name,
                    f.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        let call_env = self.env.global_only();
        let saved = std::mem::replace(&mut self.env, call_env);
        self.env.push();
        for (p, v) in f.params.iter().zip(args) {
            let _ = self.env.define(&p.name, v, false);
        }
        let outcome = self.exec_block(&f.body);
        self.env = saved;
        match outcome? {
            Exec::Return(v) => Ok(v),
            Exec::Value(v) => Ok(v),
        }
    }

    fn call_familiar(
        &mut self,
        fam: &FamiliarDecl,
        args: Vec<Value>,
        span: crate::span::Span,
    ) -> Result<Value, Diagnostic> {
        if args.len() != fam.params.len() {
            return Err(Diagnostic::runtime(
                format!(
                    "familiar `{}` expects {} argument(s), got {}",
                    fam.name,
                    fam.params.len(),
                    args.len()
                ),
                span,
            ));
        }
        let call_env = self.env.global_only();
        let saved = std::mem::replace(&mut self.env, call_env);
        self.env.push();
        for (p, v) in fam.params.iter().zip(args) {
            let _ = self.env.define(&p.name, v, false);
        }
        let outcome = self.exec_block(&fam.body);
        self.env = saved;
        match outcome? {
            Exec::Return(v) | Exec::Value(v) => Ok(v),
        }
    }

    /// Built-in operations that are not user functions. Returns `Ok(None)` if the
    /// name is not a builtin, so the normal function path runs.
    fn call_builtin(
        &mut self,
        name: &str,
        args: &[Value],
        span: crate::span::Span,
    ) -> Result<Option<Value>, Diagnostic> {
        match name {
            "similarity" => {
                if args.len() != 2 {
                    return Err(Diagnostic::runtime(
                        "`similarity` takes two embeddings".to_string(),
                        span,
                    ));
                }
                let (sa, va) = as_embedding(&args[0], span)?;
                let (sb, vb) = as_embedding(&args[1], span)?;
                if sa != sb {
                    return Err(Diagnostic::runtime(
                        format!("cannot compare embeddings of spaces `{}` and `{}`", sa, sb),
                        span,
                    ));
                }
                Ok(Some(Value::Spark(cosine(va, vb))))
            }
            "nearest" => {
                if args.len() != 3 {
                    return Err(Diagnostic::runtime(
                        "`nearest` takes (query, candidates, k)".to_string(),
                        span,
                    ));
                }
                let (qspace, qvec) = as_embedding(&args[0], span)?;
                let candidates = match &args[1] {
                    Value::List(items) => items,
                    other => {
                        return Err(Diagnostic::runtime(
                            format!(
                                "`nearest` expects a list of candidates, found {}",
                                other.type_name()
                            ),
                            span,
                        ))
                    }
                };
                let k = as_spark(&args[2], span)?.max(0.0) as usize;
                let mut scored: Vec<(usize, f64)> = Vec::with_capacity(candidates.len());
                for (i, c) in candidates.iter().enumerate() {
                    let (cspace, cvec) = as_embedding(c, span)?;
                    if cspace != qspace {
                        return Err(Diagnostic::runtime(
                            format!(
                                "cannot compare embeddings of spaces `{}` and `{}`",
                                qspace, cspace
                            ),
                            span,
                        ));
                    }
                    scored.push((i, cosine(qvec, cvec)));
                }
                // Descending by score; ties broken by original index for determinism.
                scored.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.0.cmp(&b.0))
                });
                let chosen = scored
                    .into_iter()
                    .take(k)
                    .map(|(i, _)| candidates[i].clone())
                    .collect();
                Ok(Some(Value::List(chosen)))
            }
            // Advance the logical clock (deterministic retention testing).
            "advance" => {
                let n = as_spark(args.first().unwrap_or(&Value::Spark(0.0)), span)?.max(0.0);
                self.clock = self.clock.saturating_add(n as u64);
                Ok(Some(Value::Unit))
            }
            // The accumulated audit records as a list of glyphs.
            "audit_log" => Ok(Some(Value::List(
                self.audit.iter().cloned().map(Value::Glyph).collect(),
            ))),
            "listen" => {
                if args.len() != 1 {
                    return Err(Diagnostic::runtime(
                        "`listen` takes one glyph prompt argument".to_string(),
                        span,
                    ));
                }
                let _prompt = &args[0];
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).map_err(|e| {
                    Diagnostic::runtime(format!("stdin read failed: {}", e), span)
                })?;
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                Ok(Some(Value::Glyph(line)))
            }
            _ => Ok(None),
        }
    }

    /// Run a governed-memory operation (`write` / `recent`). Scope is enforced
    /// statically; here we enforce retention (expired entries are not returned)
    /// and audit (each governed access appends a record).
    fn memory_op(
        &mut self,
        name: &str,
        method: &str,
        args: Vec<Value>,
        span: crate::span::Span,
    ) -> Result<Value, Diagnostic> {
        let now = self.clock;
        let store = self
            .memories
            .get_mut(name)
            .ok_or_else(|| Diagnostic::runtime(format!("unknown memory `{}`", name), span))?;
        if store.audit_required {
            self.audit.push(format!(
                "memory={} op={} scope={}",
                name, method, store.scope
            ));
        }
        match method {
            "write" => {
                let entry = args.into_iter().next().unwrap_or(Value::Unit);
                store.entries.push((now, entry));
                self.clock = self.clock.saturating_add(1);
                Ok(Value::Unit)
            }
            "recent" | "nearest" => {
                // recency retrieval: newest-first, excluding entries older than the
                // declared retention (semantic retrieval is composed in the flagship).
                let k = args
                    .last()
                    .map(|v| as_spark(v, span))
                    .transpose()?
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let retention = store.retention_ticks;
                let mut live: Vec<(u64, Value)> = store
                    .entries
                    .iter()
                    .filter(|(tick, _)| match retention {
                        Some(r) => (now.saturating_sub(*tick)) as f64 <= r,
                        None => true,
                    })
                    .cloned()
                    .collect();
                live.sort_by_key(|(tick, _)| std::cmp::Reverse(*tick));
                let chosen = live.into_iter().take(k).map(|(_, v)| v).collect();
                Ok(Value::List(chosen))
            }
            other => Err(Diagnostic::runtime(
                format!("unknown memory operation `{}`", other),
                span,
            )),
        }
    }
}

/// A deterministic, offline embedding: a fixed-dimension vector derived from the
/// text and its space. This is a stand-in for a real model (like the mock
/// decoder) — same text + space always yields the same vector, so similarity and
/// `nearest` are reproducible.
pub(crate) fn embed_vector(text: &str, space: &str) -> Vec<f64> {
    const DIMS: usize = 16;
    let mut v = vec![0.0f64; DIMS];
    // Hash each (token, space) into a dimension and accumulate, so semantically
    // different texts diverge but the same text is stable.
    for token in text.split_whitespace() {
        for (d, slot) in v.iter_mut().enumerate() {
            let h = fnv1a(&format!("{space}\u{0}{token}\u{0}{d}"));
            // map to [-1, 1]
            *slot += ((h % 2000) as f64) / 1000.0 - 1.0;
        }
    }
    if text.split_whitespace().next().is_none() {
        // Empty text still gets a stable, space-dependent vector.
        for (d, slot) in v.iter_mut().enumerate() {
            let h = fnv1a(&format!("{space}\u{0}<empty>\u{0}{d}"));
            *slot = ((h % 2000) as f64) / 1000.0 - 1.0;
        }
    }
    v
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn as_embedding(v: &Value, span: crate::span::Span) -> Result<(&str, &[f64]), Diagnostic> {
    match v {
        Value::Embedding { space, vector, .. } => Ok((space, vector)),
        other => Err(Diagnostic::runtime(
            format!("expected an embedding, found {}", other.type_name()),
            span,
        )),
    }
}

fn as_spark(v: &Value, span: crate::span::Span) -> Result<f64, Diagnostic> {
    match v {
        Value::Spark(n) => Ok(*n),
        other => Err(Diagnostic::runtime(
            format!("expected spark, found {}", other.type_name()),
            span,
        )),
    }
}

fn as_bool(v: &Value, span: crate::span::Span) -> Result<bool, Diagnostic> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(Diagnostic::runtime(
            format!("expected bool, found {}", other.type_name()),
            span,
        )),
    }
}

// ---------- load-time policy derivation (change add-inference-runtime) ----------

use crate::span::Span;

/// Derive, per oracle intent, the POLICY in force at the `divine` sites that use
/// it (locality via `permit(network)`, downgrade via `permit(unsafe_inference)`),
/// plus a representative span for diagnostics. Capabilities granted around a site
/// (`with grant`, a `fn`'s `requires`, a `familiar`'s `permits`) are the
/// source-visible constraints the resolver checks against the manifest.
pub(crate) fn oracle_policies(prog: &Program) -> HashMap<String, (Policy, Span)> {
    let mut var_to_intent: HashMap<String, String> = HashMap::new();
    for item in &prog.items {
        match item {
            Item::Define(f) => collect_summons(&f.body, &mut var_to_intent),
            Item::Familiar(fam) => collect_summons(&fam.body, &mut var_to_intent),
            Item::Stmt(s) => collect_summons(std::slice::from_ref(s), &mut var_to_intent),
            Item::Type(_) => {}
        }
    }

    let mut out: HashMap<String, (Policy, Span)> = HashMap::new();
    for item in &prog.items {
        match item {
            Item::Define(f) => walk_policy(&f.body, &cap_names(&f.requires), &var_to_intent, &mut out),
            Item::Familiar(fam) => walk_policy(
                &fam.body,
                &cap_names(&fam.permits),
                &var_to_intent,
                &mut out,
            ),
            Item::Stmt(s) => walk_policy(std::slice::from_ref(s), &[], &var_to_intent, &mut out),
            Item::Type(_) => {}
        }
    }
    out
}

fn cap_names(caps: &[Capability]) -> Vec<String> {
    caps.iter().map(|c| c.display()).collect()
}

fn collect_summons(stmts: &[Stmt], out: &mut HashMap<String, String>) {
    for s in stmts {
        match s {
            Stmt::Summon { name, model, .. } => {
                out.insert(name.clone(), model.clone());
            }
            Stmt::While { body, .. } | Stmt::Grant { body, .. } | Stmt::Within { body, .. } => {
                collect_summons(body, out)
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_summons(then_branch, out);
                if let Some(e) = else_branch {
                    collect_summons(e, out);
                }
            }
            Stmt::Enact { arms, .. } => {
                for arm in arms {
                    collect_summons(&arm.body, out);
                }
            }
            _ => {}
        }
    }
}

fn walk_policy(
    stmts: &[Stmt],
    active: &[String],
    var_to_intent: &HashMap<String, String>,
    out: &mut HashMap<String, (Policy, Span)>,
) {
    for s in stmts {
        match s {
            Stmt::Divine(d) => {
                let intent = var_to_intent
                    .get(&d.oracle)
                    .cloned()
                    .unwrap_or_else(|| d.oracle.clone());
                let allow_network = active.iter().any(|c| c == "permit(network)");
                let allow_downgrade = active.iter().any(|c| c == "permit(unsafe_inference)");
                out.entry(intent)
                    .and_modify(|(p, _)| {
                        p.allow_network |= allow_network;
                        p.allow_downgrade |= allow_downgrade;
                    })
                    .or_insert((
                        Policy {
                            allow_network,
                            allow_downgrade,
                            ..Policy::default()
                        },
                        d.span,
                    ));
            }
            Stmt::Grant { caps, body, .. } => {
                let mut next = active.to_vec();
                next.extend(cap_names(caps));
                walk_policy(body, &next, var_to_intent, out);
            }
            Stmt::Within { body, .. } => walk_policy(body, active, var_to_intent, out),
            Stmt::While { body, .. } => walk_policy(body, active, var_to_intent, out),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                walk_policy(then_branch, active, var_to_intent, out);
                if let Some(e) = else_branch {
                    walk_policy(e, active, var_to_intent, out);
                }
            }
            Stmt::Enact { arms, .. } => {
                for arm in arms {
                    walk_policy(&arm.body, active, var_to_intent, out);
                }
            }
            _ => {}
        }
    }
}

fn attach_provenance(v: Value, prov: &Provenance) -> Value {
    match v {
        Value::Record { fields, .. } => Value::Record {
            fields,
            provenance: Some(prov.clone()),
        },
        Value::Variant { name, fields, .. } => Value::Variant {
            name,
            fields,
            provenance: Some(prov.clone()),
        },
        other => other,
    }
}

fn propagate_provenance(v: Value, prov: Option<&Provenance>) -> Value {
    let prov = match prov {
        Some(p) => p,
        None => return v,
    };
    match v {
        Value::Record {
            fields,
            provenance: None,
        } => Value::Record {
            fields,
            provenance: Some(prov.clone()),
        },
        Value::Variant {
            name,
            fields,
            provenance: None,
        } => Value::Variant {
            name,
            fields,
            provenance: Some(prov.clone()),
        },
        other => other,
    }
}
