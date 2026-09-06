use std::time::Duration;

use anyhow::Result;

use crate::challenge::TestCase;

#[derive(Debug, Clone, serde::Deserialize)]
struct BinaryOutput {
    passed: bool,
    error: String,
    time_ns: u64,
    memory_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct HarnessResult {
    pub passed: bool,
    pub error: String,
    pub time_ns: u64,
    pub memory_bytes: u64,
    pub binary_size: u64,
    pub compile_time_ns: u64,
}

impl HarnessResult {
    fn compile_error(msg: String) -> Self {
        Self {
            passed: false,
            error: msg,
            time_ns: 0,
            memory_bytes: 0,
            binary_size: 0,
            compile_time_ns: 0,
        }
    }

    fn runtime_error(msg: String, compile_time_ns: u64, binary_size: u64) -> Self {
        Self {
            passed: false,
            error: msg,
            time_ns: 0,
            memory_bytes: 0,
            binary_size,
            compile_time_ns,
        }
    }
}

/// Full evaluation pipeline: generate source → compile → run → parse metrics.
pub async fn evaluate_candidate(
    func_source: &str,
    tests: &[TestCase],
    timeout_compile: Duration,
    timeout_run: Duration,
) -> Result<HarnessResult> {
    let tmpdir = tempfile::tempdir()?;
    let src_path = tmpdir.path().join("candidate.rs");
    let bin_path = tmpdir.path().join("candidate");

    let full_source = generate_source(func_source, tests);
    tokio::fs::write(&src_path, &full_source).await?;

    // ── Compile ─────────────────────────────────
    let compile_start = std::time::Instant::now();
    let compile_result = tokio::time::timeout(
        timeout_compile,
        tokio::process::Command::new("rustc")
            .args(["-O", "-o"])
            .arg(&bin_path)
            .arg(&src_path)
            .output(),
    )
    .await;
    let compile_time_ns = compile_start.elapsed().as_nanos() as u64;

    let compile_output = match compile_result {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Ok(HarnessResult::compile_error(format!("rustc error: {e}"))),
        Err(_) => return Ok(HarnessResult::compile_error("compilation timed out".into())),
    };

    if !compile_output.status.success() {
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        return Ok(HarnessResult::compile_error(format!(
            "compilation failed:\n{stderr}"
        )));
    }

    // ── Binary size ─────────────────────────────
    let binary_size = tokio::fs::metadata(&bin_path).await?.len();

    // ── Run ─────────────────────────────────────
    let run_result = tokio::time::timeout(
        timeout_run,
        tokio::process::Command::new(&bin_path).output(),
    )
    .await;

    let run_output = match run_result {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Ok(HarnessResult::runtime_error(
                format!("execution error: {e}"),
                compile_time_ns,
                binary_size,
            ))
        }
        Err(_) => {
            return Ok(HarnessResult::runtime_error(
                "execution timed out".into(),
                compile_time_ns,
                binary_size,
            ))
        }
    };

    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        return Ok(HarnessResult::runtime_error(
            format!("runtime error:\n{stderr}"),
            compile_time_ns,
            binary_size,
        ));
    }

    // ── Parse JSON output ───────────────────────
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let binary_out: BinaryOutput = serde_json::from_str(stdout.trim())
        .map_err(|e| anyhow::anyhow!("failed to parse binary output: {e}\nstdout: {stdout}"))?;

    Ok(HarnessResult {
        passed: binary_out.passed,
        error: binary_out.error,
        time_ns: binary_out.time_ns,
        memory_bytes: binary_out.memory_bytes,
        binary_size,
        compile_time_ns,
    })
}

/// Build a complete Rust source file with tracking allocator, candidate function,
/// test assertions, and JSON-output benchmark.
fn generate_source(func_source: &str, tests: &[TestCase]) -> String {
    let mut src = String::with_capacity(4096);

    // ── Tracking allocator ──────────────────────
    src.push_str(ALLOCATOR_PREAMBLE);

    // ── Candidate function ──────────────────────
    src.push_str("\n// ── candidate ───────────────────────────────\n");
    src.push_str(func_source);
    src.push('\n');

    // ── main() ──────────────────────────────────
    src.push_str("\nfn main() {\n");
    src.push_str("    let mut passed = true;\n");
    src.push_str("    let mut error = String::new();\n\n");

    // Test cases
    for (i, test) in tests.iter().enumerate() {
        src.push_str(&generate_test_block(i, test));
    }

    // Failure path — output JSON and exit
    src.push_str(
        r##"
    if !passed {
        let escaped = error.replace('"', "\\\"").replace('\n', "\\n");
        println!("{{\"passed\":false,\"error\":\"{}\",\"time_ns\":0,\"memory_bytes\":0}}", escaped);
        return;
    }

"##,
    );

    // Benchmark — reset allocator, run iterations, measure
    src.push_str("    // ── benchmark ───────────────────────────\n");
    src.push_str("    PEAK.store(0, std::sync::atomic::Ordering::SeqCst);\n");
    src.push_str("    ALLOCATED.store(0, std::sync::atomic::Ordering::SeqCst);\n\n");

    let bench_input = &tests[0].input;
    src.push_str("    let iterations = 10_000u64;\n");
    src.push_str("    let start = std::time::Instant::now();\n");
    src.push_str("    for _ in 0..iterations {\n");
    src.push_str(&format!(
        "        std::hint::black_box(solve(std::hint::black_box({})));\n",
        bench_input
    ));
    src.push_str("    }\n");
    src.push_str("    let elapsed = start.elapsed();\n");
    src.push_str("    let time_ns = elapsed.as_nanos() as u64 / iterations;\n");
    src.push_str("    let memory_bytes = PEAK.load(std::sync::atomic::Ordering::SeqCst);\n\n");

    // Success JSON output
    src.push_str(r##"    println!("{{\"passed\":true,\"error\":\"\",\"time_ns\":{},\"memory_bytes\":{}}}", time_ns, memory_bytes);
"##);

    src.push_str("}\n");
    src
}

fn generate_test_block(i: usize, test: &TestCase) -> String {
    let mut block = String::new();
    block.push_str(&format!("    // test {i}\n"));
    block.push_str("    {\n");
    block.push_str(&format!("        let result = solve({});\n", test.input));
    block.push_str(&format!("        let expected = {};\n", test.expected));
    block.push_str("        if result != expected {\n");
    block.push_str("            passed = false;\n");
    block.push_str(&format!(
        "            error = format!(\"test {i}: got {{:?}}, want {{:?}}\", result, expected);\n"
    ));
    block.push_str("        }\n");
    block.push_str("    }\n");
    block
}

const ALLOCATOR_PREAMBLE: &str = r#"use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let current = ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(current, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;
"#;
