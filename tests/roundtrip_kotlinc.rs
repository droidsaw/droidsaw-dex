//! kotlinc-1.9.22 roundtrip gate (PR-9a scaffolding).
//!
//! Per Brief Acceptance gates §"Roundtrip-via-kotlinc gate":
//!
//!   each clean corpus entry decompiles → recompiles via kotlinc-1.9.22
//!   → re-decompiles → compares against first decompile under AST
//!   normalization. `coroutine_suspend/*` is roundtrip-gate-EXEMPT.
//!
//! ## What PR-9a delivers
//!
//! Just the harness machinery:
//!
//! 1. Compile each clean kotlinc-1.9 corpus fixture (`*.kt` → `.class` →
//!    `.dex`) using kotlinc-1.9.22 + d8 — same as `corpus_check.rs`'s
//!    compile path.
//! 2. Decompile the resulting `.dex` via [`droidsaw_dex::classes::decompile_class`]
//!    to produce the **first decompile** string `D1`.
//! 3. Attempt kotlinc-recompile of `D1` (write to a temp `.kt` file,
//!    invoke kotlinc-1.9.22, observe success/failure).
//! 4. If recompile succeeds: convert recompiled `.class` → `.dex` →
//!    decompile again to produce `D2`; compare `D1` vs `D2` byte-equal.
//! 5. If recompile fails: record the failure stage (recompile / d8 /
//!    re-decompile / diff).
//! 6. Per-fixture expected outcome is hardcoded in [`expected_outcome`]:
//!    fixtures whose roundtrip works pass with `Pass`; fixtures whose
//!    roundtrip is gated on un-landed Kotlin emit pieces are
//!    `RecompileFails` (the EXPECTED state today — current decompile
//!    produces Java-syntax output that kotlinc rejects); the
//!    coroutine_suspend fixture is `Exempt` per Brief.
//!
//! As Kotlin emit lands incrementally, individual fixtures flip from
//! `RecompileFails` to `Pass`. The harness is the gate that makes each
//! flip a deliberate ratchet move rather than silent drift.
//!
//! ## What this PR does NOT deliver
//!
//! - **Class-level Kotlin emit** (top-level fn extraction from `<XxxKt>`
//!   synthetic classes; `class`/`sealed class`/`object`/`data class`
//!   keyword emission). Lands in PR-9b/9d/9e.
//! - **Stmt-level Kotlin emit** (parallel to `emit_stmt_depth` for the
//!   Kotlin dialect: `val/var name: Type = expr`, `fun foo() = expr`
//!   single-expression bodies, etc.). Lands in PR-9c.
//! - **Diff normalization** (whitespace / variable-name renaming /
//!   comment elision). Today's diff is byte-equal; PR-9c may relax to
//!   AST-equiv if byte-identity proves too strict on round-tripped
//!   shapes.
//!
//! ## Skip behaviour
//!
//! When kotlinc / d8 / javac are unavailable (developer machine without
//! the toolchain), the test prints a SKIP message and exits clean.
//! Mirrors `corpus_check.rs` and `fixture_ratchet.rs` toolchain-gate
//! discipline.

use std::path::{Path, PathBuf};
use std::process::Command;

use droidsaw_dex::DexFile;
use droidsaw_dex::classes::{classes_to_decompile, decompile_class};
use droidsaw_fixture_harness::{check_warnings_strict, skipped_outcome};

// ── tools ────────────────────────────────────────────────────────────────

struct Tools {
    kotlinc: PathBuf,
    d8: PathBuf,
}

impl Tools {
    fn resolve() -> Option<Self> {
        Some(Tools {
            kotlinc: resolve_kotlinc()?,
            d8: resolve_d8()?,
        })
    }
}

fn resolve_kotlinc() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("KOTLIN_HOME") {
        let p = Path::new(&home).join("bin").join("kotlinc");
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg("kotlinc").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn resolve_d8() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("ANDROID_HOME") {
        let bt = Path::new(&home).join("build-tools");
        if let Ok(read) = std::fs::read_dir(&bt) {
            let mut versions: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
            versions.sort();
            for v in versions.into_iter().rev() {
                let d8 = v.join("d8");
                if d8.exists() {
                    return Some(d8);
                }
            }
        }
    }
    let out = Command::new("which").arg("d8").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

// ── compile / d8 ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct StageFailure {
    stage: &'static str,
    detail: String,
}

fn kotlinc_compile(
    tools: &Tools,
    src: &Path,
    class_dir: &Path,
) -> Result<(), StageFailure> {
    // Bundled kotlinx-coroutines jar lives next to kotlinc under the
    // distribution's lib/ directory. coroutine_suspend needs it; other
    // constructs do not, but adding the classpath unconditionally is
    // cheap (kotlinc accepts an unused -cp entry).
    let coroutines_jar = tools
        .kotlinc
        .parent()
        .and_then(|bin| bin.parent())
        .map(|home| home.join("lib").join("kotlinx-coroutines-core-jvm.jar"))
        .filter(|p| p.exists());
    let mut cmd = Command::new(&tools.kotlinc);
    cmd.arg("-jvm-target").arg("21");
    if let Some(jar) = coroutines_jar.as_ref() {
        cmd.arg("-cp").arg(jar);
    }
    cmd.arg("-d").arg(class_dir).arg(src);
    let out = cmd.output().map_err(|e| StageFailure {
        stage: "kotlinc spawn",
        detail: e.to_string(),
    })?;
    if !out.status.success() {
        return Err(StageFailure {
            stage: "kotlinc",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

fn d8_convert(tools: &Tools, class_dir: &Path, dex_dir: &Path) -> Result<(), StageFailure> {
    let entries = std::fs::read_dir(class_dir).map_err(|e| StageFailure {
        stage: "read class_dir",
        detail: e.to_string(),
    })?;
    let class_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("class"))
        .collect();
    if class_files.is_empty() {
        return Err(StageFailure {
            stage: "d8",
            detail: "no .class files produced by kotlinc".to_string(),
        });
    }
    let out = Command::new(&tools.d8)
        .arg("--no-desugaring")
        .arg("--output")
        .arg(dex_dir)
        .args(&class_files)
        .output()
        .map_err(|e| StageFailure {
            stage: "d8 spawn",
            detail: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(StageFailure {
            stage: "d8",
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

// ── decompile ────────────────────────────────────────────────────────────

/// Decompile every class in `dex_path/classes.dex` as a single
/// concatenated string. Order matches `class_defs` iteration; classes
/// are separated by a `// ── class X ──` divider so the same harness
/// can split per-class on need without forcing a single-class corpus.
fn decompile_dex(dex_path: &Path) -> Result<String, StageFailure> {
    let data = std::fs::read(dex_path).map_err(|e| StageFailure {
        stage: "read dex",
        detail: e.to_string(),
    })?;
    let dex = DexFile::parse(&data, None).map_err(|e| StageFailure {
        stage: "parse dex",
        detail: format!("{e:?}"),
    })?;
    let mut out = String::new();
    // Iterate via `classes_to_decompile` (not `dex.class_defs.iter()`) so
    // the iterator-level suppression filters fire — `is_suppressible_enum_artifact`
    // for enum subclass artifacts, and
    // `is_suppressible_kotlin_sealed_subclass` for Kotlin sealed subclasses
    // whose content is inlined into the parent's `sealed class { ... }`
    // body. The top binary uses this same entry point at
    // `droidsaw/src/commands/mod.rs::dex_classes`; mirroring it here keeps
    // the harness's roundtrip semantics in sync with production.
    for (_, cd) in classes_to_decompile(&dex) {
        let desc = dex.get_type_descriptor(cd.class_idx).unwrap_or("?");
        out.push_str("// ── class ");
        out.push_str(desc);
        out.push_str(" ──\n");
        out.push_str(&decompile_class(&dex, &data, cd));
        out.push('\n');
    }
    Ok(out)
}

// ── per-fixture expected outcome ─────────────────────────────────────────

/// What we expect the kotlinc-roundtrip to produce for a given
/// fixture, given the **current** state of Kotlin emit support. As
/// Kotlin emit lands across PR-9b/9c/9d/9e/9f, fixtures flip from
/// `RecompileFails` → `Pass` and the assertion in [`roundtrip_kotlinc`]
/// enforces the ratchet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Pass is unconstructed today; flipped per fixture as PR-9b–9f land.
enum Expected {
    /// Decompile output is compilable Kotlin, kotlinc accepts it,
    /// re-decompile equals first decompile.
    Pass,
    /// Decompile output isn't valid Kotlin yet — kotlinc rejects with
    /// a parse / type error. Expected state for fixtures gated on
    /// PR-9b through PR-9f.
    RecompileFails,
    /// Brief Acceptance gates §exemption — coroutine_suspend's
    /// decompile renders a banner + raw bytecode, which is intentionally
    /// not Kotlin-syntactic. Roundtrip-gate-EXEMPT by design.
    Exempt,
}

/// Hardcoded per-fixture expected outcome. Keyed by
/// `<construct>/<fixture-name>` (matches the corpus directory
/// structure under `tests/corpus/clean/kotlinc-1.9/`). Future
/// PR-9b/9c/9d/9e/9f sessions edit this table to flip fixtures to
/// `Pass` as the relevant Kotlin emit pieces land.
fn expected_outcome(construct: &str, fixture_name: &str) -> Expected {
    match (construct, fixture_name) {
        // coroutine_suspend renders as a banner + raw smali. Brief
        // exemption.
        ("coroutine_suspend", _) => Expected::Exempt,

        // PR-9c flips: int-discriminant `when` (dense + sparse) maps
        // directly to the simple `<when-with-arm-assigns> + return v`
        // shape that the Kotlinify post-pass handles.
        // `kotlinify_facade_when_return` rewrites these to
        // `return when (...) { ... }` form that kotlinc accepts +
        // round-trips byte-equal through decompile-recompile-decompile.
        ("when_int", _) => Expected::Pass,
        ("when_int_sparse", _) => Expected::Pass,

        // PR-9e flips: sealed-OBJECT fixtures with ≥3 arms. Chain-merge
        // fix in `signatures/kotlinc19/when_sealed_object` absorbs
        // inner-lifted MultiArms into the outer chain; the
        // `is_kotlin_sealed_root` predicate + `render_kotlin_sealed_class_header`
        // emit the parent class as `sealed class { object Sub : Parent() ... }`
        // with subclass content inlined; the
        // `is_suppressible_kotlin_sealed_subclass` iterator filter
        // drops the subclasses from top-level emission; the
        // `kotlinify_facade_when_inline_return` post-pass rewrites the
        // facade body's `when (...) { ... return v; }`-in-last-arm
        // shape as `return when (...) { ... }`.
        // PR-9e.1 lowered MIN_ARMS to 2; sealed-OBJECT/02arms now flips.
        ("when_sealed_object", "02arms") => Expected::Pass,
        ("when_sealed_object", "05arms") => Expected::Pass,
        ("when_sealed_object", "50arms") => Expected::Pass,
        ("when_sealed_object", "130arms") => Expected::Pass,

        // Remaining fixtures are deferred:
        // - `when_string/*` 02arms: recognizer doesn't fire on
        //   the 2-arm linear `Intrinsics.areEqual` chain (different
        //   layer fix).
        // - `when_string/*` 05arms / 50arms: hashCode + per-bucket
        //   `equals` chain produces multi-stmt arm bodies; needs
        //   a more sophisticated post-pass.
        // - `when_sealed_object/02arms`: recognizer's `MIN_ARMS=3` gate
        //   rejects 2-arm shapes.
        // - `when_sealed_class/*`: multi-stmt arm bodies (cast + getter
        //   + arithmetic) need a separate post-pass.
        // - `data_class_destructure/pair`: invoke-dynamic / D8 synth
        //   throw stub blockers.
        _ => Expected::RecompileFails,
    }
}

// ── corpus walker ────────────────────────────────────────────────────────

#[derive(Debug)]
struct Fixture {
    construct: String,
    name: String,
    src: PathBuf,
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join("clean")
        .join("kotlinc-1.9")
}

fn discover_fixtures() -> std::io::Result<Vec<Fixture>> {
    let mut out = Vec::new();
    let root = corpus_root();
    if !root.is_dir() {
        return Ok(out);
    }
    for ct_entry in std::fs::read_dir(&root)? {
        let ct_entry = ct_entry?;
        if !ct_entry.file_type()?.is_dir() {
            continue;
        }
        let construct = ct_entry.file_name().to_string_lossy().into_owned();
        for src_entry in std::fs::read_dir(ct_entry.path())? {
            let src_entry = src_entry?;
            let path = src_entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("kt") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                out.push(Fixture {
                    construct: construct.clone(),
                    name,
                    src: path,
                });
            }
        }
    }
    out.sort_by(|a, b| a.construct.cmp(&b.construct).then(a.name.cmp(&b.name)));
    Ok(out)
}

// ── per-fixture roundtrip ────────────────────────────────────────────────

// Field-read warnings are silenced because `Debug`-format reads
// (`{actual:?}` in panic strings) don't satisfy the dead-code linter.
// Each variant's payload IS load-bearing for the diagnostic message.
#[derive(Debug)]
#[allow(dead_code)]
enum Outcome {
    /// Full roundtrip: D1 == D2 byte-equal.
    Pass,
    /// Recompile of decompile output failed.
    RecompileFailed { detail: String },
    /// Recompile succeeded but D1 != D2.
    DiffMismatch { d1_len: usize, d2_len: usize },
    /// d8 / parse-dex / read-dex failure (rare; indicates infrastructure
    /// gap rather than emit issue).
    Infrastructure { stage: &'static str, detail: String },
    /// Brief exemption.
    Exempt,
}

fn run_roundtrip(tools: &Tools, fx: &Fixture, work_root: &Path) -> Outcome {
    if expected_outcome(&fx.construct, &fx.name) == Expected::Exempt {
        return Outcome::Exempt;
    }

    let work = work_root.join(&fx.construct).join(&fx.name);
    let class_dir1 = work.join("classes-iter1");
    let dex_dir1 = work.join("dex-iter1");
    let class_dir2 = work.join("classes-iter2");
    let dex_dir2 = work.join("dex-iter2");
    // Clear + recreate per-fixture work dirs each run. `work_root`
    // persists across `cargo test` invocations (it's under
    // `target/roundtrip-kotlinc/`), so stale `.class` files from a
    // prior iteration would leak into the next d8 input. PR-9c of
    // #41b — surfaced when the harness's d1_path was switched from
    // a fixed `decompile-iter1.kt` to a fixture-named path: stale
    // `decompile_iter1Kt.class` was getting d8'd alongside the new
    // `<NameKt>.class` and producing a doubled DEX, doubling D2's
    // length and failing the byte-equality check.
    for d in [&class_dir1, &dex_dir1, &class_dir2, &dex_dir2] {
        // remove_dir_all returns NotFound on first run; ignore.
        let _ = std::fs::remove_dir_all(d);
        if let Err(e) = std::fs::create_dir_all(d) {
            return Outcome::Infrastructure {
                stage: "mkdir",
                detail: e.to_string(),
            };
        }
    }

    // Iter 1: original .kt → .class → .dex → D1.
    if let Err(e) = kotlinc_compile(tools, &fx.src, &class_dir1) {
        return Outcome::Infrastructure {
            stage: e.stage,
            detail: e.detail,
        };
    }
    if let Err(e) = d8_convert(tools, &class_dir1, &dex_dir1) {
        return Outcome::Infrastructure {
            stage: e.stage,
            detail: e.detail,
        };
    }
    let d1 = match decompile_dex(&dex_dir1.join("classes.dex")) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Infrastructure {
                stage: e.stage,
                detail: e.detail,
            };
        }
    };

    // Iter 2: D1 → kotlinc-recompile attempt. The decompile output
    // is currently Java-style (PR-9b through PR-9f progressively
    // make it Kotlin); recompile is the gate that fires the ratchet
    // bit.
    //
    // D1 is written to a file named after the fixture (`<name>.kt`)
    // rather than a generic path so that kotlinc's filename →
    // synthetic-facade-class-name rule (`<NameKt>`) regenerates the
    // SAME class descriptor as the original. The harness's
    // `decompile_dex` divider line embeds the class descriptor; if
    // the recompile produced a different facade name (e.g.
    // `decompile_iter1Kt` from a generic temp path), the dividers
    // would diverge between D1 and D2 even though the actual fn
    // bodies were byte-identical. PR-9c of #41b.
    let d1_path = work.join(format!("{}.kt", fx.name));
    if let Err(e) = std::fs::write(&d1_path, &d1) {
        return Outcome::Infrastructure {
            stage: "write d1",
            detail: e.to_string(),
        };
    }
    if let Err(e) = kotlinc_compile(tools, &d1_path, &class_dir2) {
        return Outcome::RecompileFailed { detail: e.detail };
    }
    if let Err(e) = d8_convert(tools, &class_dir2, &dex_dir2) {
        return Outcome::RecompileFailed {
            detail: format!("d8 (post-recompile) failed: {}", e.detail),
        };
    }
    let d2 = match decompile_dex(&dex_dir2.join("classes.dex")) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::RecompileFailed {
                detail: format!("re-decompile failed: stage={} detail={}", e.stage, e.detail),
            };
        }
    };

    if d1 == d2 {
        Outcome::Pass
    } else {
        Outcome::DiffMismatch {
            d1_len: d1.len(),
            d2_len: d2.len(),
        }
    }
}

// ── test ─────────────────────────────────────────────────────────────────

#[test]
fn roundtrip_kotlinc() {
    let fixtures = discover_fixtures().expect("discover_fixtures");
    if fixtures.is_empty() {
        eprintln!("SKIP: no kotlinc-1.9 corpus fixtures found");
        return;
    }

    let tools = match Tools::resolve() {
        Some(t) => t,
        None => {
            let outcome = skipped_outcome(
                "roundtrip_kotlinc",
                "kotlinc/d8",
                "kotlinc-1.9.22 / d8 not found. \
                 Set KOTLIN_HOME=/path/to/kotlinc-1.9.22 + ANDROID_HOME=/path/to/android-sdk \
                 (with build-tools/<v>/d8 present) — or add `kotlinc` and `d8` to PATH.",
            );
            for w in &outcome.warnings {
                eprintln!("SKIP: {w:?}");
            }
            check_warnings_strict(&[outcome]).expect("strict-warnings gate");
            return;
        }
    };

    let work_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("roundtrip-kotlinc");
    std::fs::create_dir_all(&work_root).expect("mkdir work_root");

    let mut mismatches = Vec::<String>::new();
    let mut counts = Counts::default();
    for fx in &fixtures {
        let expected = expected_outcome(&fx.construct, &fx.name);
        eprintln!("roundtrip_kotlinc: starting {}/{}", fx.construct, fx.name);
        // Spawn each fixture's roundtrip in a thread with a 16 MB
        // stack. The 130-arm sealed-object fixture's structurer +
        // emit overflows the 2 MB default test thread on the
        // pre-recognizer if-chain shape (recognizer fires above
        // emit's depth cap is fine; structurer-side processing of
        // the input chain is what blows the stack).
        let actual = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn({
                let tools_kotlinc = tools.kotlinc.clone();
                let tools_d8 = tools.d8.clone();
                let fx_construct = fx.construct.clone();
                let fx_name = fx.name.clone();
                let fx_src = fx.src.clone();
                let work_root = work_root.clone();
                move || {
                    let tools = Tools {
                        kotlinc: tools_kotlinc,
                        d8: tools_d8,
                    };
                    let fx = Fixture {
                        construct: fx_construct,
                        name: fx_name,
                        src: fx_src,
                    };
                    run_roundtrip(&tools, &fx, &work_root)
                }
            })
            .expect("spawn thread")
            .join()
            .unwrap_or_else(|_| Outcome::Infrastructure {
                stage: "thread join",
                detail: "panic in roundtrip thread (likely stack overflow past 16 MB cap)"
                    .to_string(),
            });
        let label = format!("{}/{}", fx.construct, fx.name);
        match (expected, &actual) {
            (Expected::Pass, Outcome::Pass) => counts.pass += 1,
            (Expected::Exempt, Outcome::Exempt) => counts.exempt += 1,
            (Expected::RecompileFails, Outcome::RecompileFailed { .. }) => {
                counts.expected_fail += 1;
            }
            // Any other (expected, actual) pair is a ratchet drift.
            // Either:
            //   (a) Pass expected but recompile / diff failed — regression
            //       in Kotlin emit;
            //   (b) RecompileFails expected but recompile succeeded —
            //       improvement; flip the fixture to `Pass` in
            //       expected_outcome.
            (exp, act) => {
                mismatches.push(format!("{label}: expected {exp:?}, got {act:?}"));
            }
        }
    }

    eprintln!(
        "roundtrip_kotlinc: {pass} pass / {expected_fail} expected-fail \
         / {exempt} exempt / {mism} drift, total {total}",
        pass = counts.pass,
        expected_fail = counts.expected_fail,
        exempt = counts.exempt,
        mism = mismatches.len(),
        total = fixtures.len(),
    );

    if !mismatches.is_empty() {
        panic!(
            "{} ratchet drift(s) — update `expected_outcome` if this is \
             intentional improvement, or fix the regression:\n  - {}",
            mismatches.len(),
            mismatches.join("\n  - "),
        );
    }
}

#[derive(Default)]
struct Counts {
    pass: usize,
    expected_fail: usize,
    exempt: usize,
}
