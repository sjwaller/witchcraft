//! Runtime values. Inference produces `Value::Inferred`, which carries confidence
//! and provenance; provenance also rides on records/variants so it can thread
//! through field access into `enact`.

#[derive(Clone, Debug, PartialEq)]
pub struct Provenance {
    pub oracle: String,
    pub model: String,
    pub seed: u64,
}

impl Provenance {
    pub fn render(&self) -> String {
        format!(
            "oracle={} model={} seed={}",
            self.oracle, self.model, self.seed
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Spark(f64),
    Bool(bool),
    Glyph(String),
    Record {
        fields: Vec<(String, Value)>,
        provenance: Option<Provenance>,
    },
    Variant {
        name: String,
        fields: Vec<(String, Value)>,
        provenance: Option<Provenance>,
    },
    Oracle {
        name: String,
        model: String,
    },
    /// An embedding vector tagged with its space (the producing model id).
    Embedding {
        space: String,
        vector: Vec<f64>,
        provenance: Option<Provenance>,
    },
    List(Vec<Value>),
    Inferred {
        inner: Box<Value>,
        confidence: f64,
        provenance: Provenance,
    },
    Unit,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Spark(_) => "spark",
            Value::Bool(_) => "bool",
            Value::Glyph(_) => "glyph",
            Value::Record { .. } => "record",
            Value::Variant { .. } => "variant",
            Value::Oracle { .. } => "oracle",
            Value::Embedding { .. } => "embedding",
            Value::List(_) => "list",
            Value::Inferred { .. } => "inferred",
            Value::Unit => "essence",
        }
    }

    pub fn provenance(&self) -> Option<&Provenance> {
        match self {
            Value::Record { provenance, .. } | Value::Variant { provenance, .. } => {
                provenance.as_ref()
            }
            Value::Embedding { provenance, .. } => provenance.as_ref(),
            Value::Inferred { provenance, .. } => Some(provenance),
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Value::Spark(n) => fmt_num(*n),
            Value::Bool(b) => b.to_string(),
            Value::Glyph(s) => s.clone(),
            Value::Record { fields, .. } => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(n, v)| format!("{}: {}", n, v.display()))
                    .collect();
                format!("{{ {} }}", inner.join(", "))
            }
            Value::Variant { name, fields, .. } => {
                if fields.is_empty() {
                    name.clone()
                } else {
                    let inner: Vec<String> = fields
                        .iter()
                        .map(|(n, v)| format!("{}: {}", n, v.display()))
                        .collect();
                    format!("{}({})", name, inner.join(", "))
                }
            }
            Value::Oracle { model, .. } => format!("<oracle {}>", model),
            Value::Embedding { space, .. } => format!("<embedding@{}>", space),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Inferred {
                inner, confidence, ..
            } => format!(
                "Inferred({}, confidence={})",
                inner.display(),
                fmt_num(*confidence)
            ),
            Value::Unit => "()".to_string(),
        }
    }
}

pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}
