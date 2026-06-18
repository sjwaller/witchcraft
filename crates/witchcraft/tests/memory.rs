//! Governed-memory acceptance tests (§5.2): scope is a capability (cross-tenant
//! access will not compile), and retention/audit are runtime-enforced.

use witchcraft::{check_source, run_source, RunConfig};

fn run(src: &str) -> String {
    run_source(src, RunConfig::default()).unwrap_or_else(|ds| {
        panic!(
            "expected program to run, got: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    })
}

fn check_err(src: &str) -> String {
    match check_source(src) {
        Ok(()) => panic!("expected a compile error, but the program checked clean"),
        Err(ds) => ds
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

#[test]
fn declare_a_governed_memory() {
    let src = "\
memory tickets { scope tenant, retention 24 months, retrieval recency, audit required }
within tenant { tickets.write(\"hello\") }
";
    assert!(check_source(src).is_ok());
}

#[test]
fn memory_without_scope_is_rejected() {
    let src = "memory tickets { retention 24 months }";
    assert!(check_err(src).contains("must declare a `scope`"));
}

#[test]
fn in_scope_access_is_granted() {
    let src = "\
memory tickets { scope tenant }
within tenant {
    tickets.write(\"a\")
    print tickets.recent(1)
}
";
    assert_eq!(run(src), "[a]\n");
}

#[test]
fn out_of_scope_access_is_a_compile_error() {
    let src = "\
memory tickets { scope tenant }
tickets.write(\"leak\")
";
    let err = check_err(src);
    assert!(err.contains("tickets"), "names the memory: {err}");
    assert!(err.contains("tenant"), "names the scope: {err}");
}

#[test]
fn scope_grant_does_not_leak_past_within() {
    let src = "\
memory tickets { scope tenant }
within tenant { tickets.write(\"a\") }
print tickets.recent(1)
";
    assert!(check_err(src).contains("tickets"));
}

#[test]
fn recency_retrieval_within_scope() {
    let src = "\
memory tickets { scope tenant, retention 100 }
within tenant {
    tickets.write(\"first\")
    tickets.write(\"second\")
    print tickets.recent(2)
}
";
    assert_eq!(run(src), "[second, first]\n");
}

#[test]
fn expired_entries_are_not_retrieved() {
    let src = "\
memory tickets { scope tenant, retention 5 }
within tenant {
    tickets.write(\"old\")
    advance(10)
    tickets.write(\"new\")
    print tickets.recent(5)
}
";
    assert_eq!(run(src), "[new]\n");
}

#[test]
fn audited_access_produces_a_record() {
    let src = "\
memory tickets { scope tenant, audit required }
within tenant { tickets.write(\"x\") }
print audit_log()
";
    assert_eq!(run(src), "[memory=tickets op=write scope=tenant]\n");
}

#[test]
fn unaudited_memory_records_nothing() {
    let src = "\
memory tickets { scope tenant }
within tenant { tickets.write(\"x\") }
print audit_log()
";
    assert_eq!(run(src), "[]\n");
}
