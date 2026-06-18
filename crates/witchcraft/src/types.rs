//! Resolved types. This is where nativeness lives: `Inferred<T>` is a distinct
//! type that is deliberately NOT assignable to a plain `T` (it must be
//! discharged first), and refinements/variants are structural.

#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    /// Numeric, optionally refined to a `[lo, hi]` range (inclusive).
    Spark {
        lo: Option<f64>,
        hi: Option<f64>,
    },
    Bool,
    Glyph,
    Oracle,
    Record(Vec<(String, Type)>),
    Sum(Vec<Variant>),
    /// The result of inference: carries an underlying type, plus (at runtime)
    /// confidence and provenance. Not assignable to the underlying type.
    Inferred(Box<Type>),
    Unit,
    /// Unannotated host values; assignable to/from anything (kept permissive so
    /// the v0.1 host language stays ergonomic without full inference).
    Unknown,
}

impl Type {
    pub fn spark() -> Type {
        Type::Spark { lo: None, hi: None }
    }

    /// Themed, human-readable name for diagnostics.
    pub fn display(&self) -> String {
        match self {
            Type::Spark { lo: None, hi: None } => "spark".to_string(),
            Type::Spark { lo, hi } => format!(
                "spark in {}..{}",
                lo.map(fmt_num).unwrap_or_else(|| "_".into()),
                hi.map(fmt_num).unwrap_or_else(|| "_".into())
            ),
            Type::Bool => "bool".to_string(),
            Type::Glyph => "glyph".to_string(),
            Type::Oracle => "oracle".to_string(),
            Type::Record(fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(n, t)| format!("{}: {}", n, t.display()))
                    .collect();
                format!("{{ {} }}", inner.join(", "))
            }
            Type::Sum(variants) => {
                let names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
                format!("one_of {{ {} }}", names.join(", "))
            }
            Type::Inferred(inner) => format!("Inferred<{}>", inner.display()),
            Type::Unit => "essence".to_string(),
            Type::Unknown => "essence".to_string(),
        }
    }

    /// Is a value of type `self` usable where `target` is required?
    pub fn assignable_to(&self, target: &Type) -> bool {
        use Type::*;
        match (self, target) {
            (Unknown, _) | (_, Unknown) => true,
            (Spark { lo: a, hi: b }, Spark { lo: c, hi: d }) => range_within(*a, *b, *c, *d),
            (Bool, Bool) => true,
            (Glyph, Glyph) => true,
            (Oracle, Oracle) => true,
            (Unit, Unit) => true,
            (Record(fa), Record(fb)) => {
                fa.len() == fb.len()
                    && fb.iter().all(|(n, tb)| {
                        fa.iter()
                            .find(|(m, _)| m == n)
                            .is_some_and(|(_, ta)| ta.assignable_to(tb))
                    })
            }
            (Sum(va), Sum(vb)) => {
                // structural: every variant of `self` must exist (by name + payload) in target
                va.iter().all(|v| {
                    vb.iter().any(|w| {
                        v.name == w.name
                            && v.fields.len() == w.fields.len()
                            && v.fields
                                .iter()
                                .zip(&w.fields)
                                .all(|((_, t1), (_, t2))| t1.assignable_to(t2))
                    })
                })
            }
            (Inferred(a), Inferred(b)) => a.assignable_to(b),
            // The headline: Inferred<T> is NOT assignable to plain T.
            _ => false,
        }
    }
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn range_within(a: Option<f64>, b: Option<f64>, c: Option<f64>, d: Option<f64>) -> bool {
    // [a,b] within [c,d], treating None bounds as ±infinity.
    let lo_ok = match (c, a) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(c), Some(a)) => a >= c,
    };
    let hi_ok = match (d, b) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(d), Some(b)) => b <= d,
    };
    lo_ok && hi_ok
}
