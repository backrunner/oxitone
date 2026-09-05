//! `bench-archive` — collect criterion estimates plus environment info into
//! the JSON baseline archive required by
//! `.agents/docs/05-performance-and-benchmarks.md` (回归策略/验收):
//! `benchmarks/results/<date>-<machine>.json`.
//!
//! Usage: run `cargo bench -p oxitone-bench` first, then
//! `cargo run -p oxitone-bench --bin bench-archive`. The command rewrites
//! today's file for this machine in place.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Serialize)]
struct Machine {
    hostname: String,
    cpu: String,
    os: String,
    arch: String,
    rustc: String,
}

#[derive(Serialize)]
struct Config {
    sample_rate: u32,
    block_size: u32,
    channels: u32,
    harness: String,
    note: String,
}

#[derive(Serialize)]
struct Estimate {
    point_estimate_ns: f64,
    standard_error_ns: f64,
    confidence_interval_ns: [f64; 2],
}

#[derive(Serialize)]
struct BenchResult {
    id: String,
    throughput: Option<String>,
    mean: Estimate,
    median: Estimate,
    std_dev: Estimate,
}

#[derive(Serialize)]
struct Archive {
    schema: String,
    date: String,
    machine: Machine,
    config: Config,
    benchmarks: Vec<BenchResult>,
}

fn run_capture(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn machine_info() -> Machine {
    let cpu = run_capture("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let os = if cfg!(target_os = "macos") {
        let name = run_capture("sw_vers", &["-productName"]);
        let version = run_capture("sw_vers", &["-productVersion"]);
        format!("{name} {version}")
    } else {
        run_capture("uname", &["-srm"])
    };
    Machine {
        hostname: run_capture("hostname", &[]),
        cpu,
        os,
        arch: std::env::consts::ARCH.to_string(),
        rustc: run_capture("rustc", &["--version"]),
    }
}

fn machine_slug(machine: &Machine) -> String {
    let slug: String = machine
        .cpu
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn criterion_root() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir).join("criterion");
    }
    workspace_root().join("target/criterion")
}

#[derive(serde::Deserialize)]
struct Stat {
    point_estimate: f64,
    standard_error: f64,
    confidence_interval: ConfidenceInterval,
}

#[derive(serde::Deserialize)]
struct ConfidenceInterval {
    lower_bound: f64,
    upper_bound: f64,
}

#[derive(serde::Deserialize)]
struct Estimates {
    mean: Stat,
    median: Stat,
    std_dev: Stat,
}

#[derive(serde::Deserialize)]
struct BenchmarkMeta {
    full_id: Option<String>,
    throughput: Option<ThroughputMeta>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
enum ThroughputMeta {
    Bytes(u64),
    Elements(u64),
}

fn stat(s: Stat) -> Estimate {
    Estimate {
        point_estimate_ns: s.point_estimate,
        standard_error_ns: s.standard_error,
        confidence_interval_ns: [
            s.confidence_interval.lower_bound,
            s.confidence_interval.upper_bound,
        ],
    }
}

fn collect(criterion_dir: &Path) -> Vec<BenchResult> {
    let mut results = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![criterion_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("new/estimates.json").is_file() {
                    dirs.push(path);
                } else if entry.file_name() != "report" {
                    stack.push(path);
                }
            }
        }
    }
    dirs.sort();
    for dir in dirs {
        // Directory names sanitize `/` to `_`; the canonical id lives in
        // `new/benchmark.json` as `full_id`.
        let fallback_id = dir
            .strip_prefix(criterion_dir)
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        let Ok(estimates) = std::fs::read_to_string(dir.join("new/estimates.json")) else {
            continue;
        };
        let Ok(estimates) = serde_json::from_str::<Estimates>(&estimates) else {
            continue;
        };
        let meta = std::fs::read_to_string(dir.join("new/benchmark.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<BenchmarkMeta>(&s).ok());
        let id = meta
            .as_ref()
            .and_then(|m| m.full_id.clone())
            .unwrap_or(fallback_id);
        let throughput = meta.and_then(|m| m.throughput).map(|t| match t {
            ThroughputMeta::Bytes(n) => format!("bytes:{n}"),
            ThroughputMeta::Elements(n) => format!("elements:{n}"),
        });
        results.push(BenchResult {
            id,
            throughput,
            mean: stat(estimates.mean),
            median: stat(estimates.median),
            std_dev: stat(estimates.std_dev),
        });
    }
    results.sort_by(|a, b| a.id.cmp(&b.id));
    results
}

fn main() {
    let root = workspace_root();
    let criterion_dir = criterion_root();
    if !criterion_dir.is_dir() {
        eprintln!(
            "no criterion output at {}; run `cargo bench -p oxitone-bench` first",
            criterion_dir.display()
        );
        std::process::exit(1);
    }
    let benchmarks = collect(&criterion_dir);
    if benchmarks.is_empty() {
        eprintln!(
            "no benchmark estimates found under {}",
            criterion_dir.display()
        );
        std::process::exit(1);
    }

    let machine = machine_info();
    let date = run_capture("date", &["+%F"]);
    let archive = Archive {
        schema: "oxitone-bench-results/v1".to_string(),
        date: date.clone(),
        config: Config {
            sample_rate: 48_000,
            block_size: 128,
            channels: 2,
            harness: "criterion 0.5".to_string(),
            note: "microbenchmarks are fixed-seed with preallocated buffers; warmup and \
                   measurement durations follow the criterion defaults"
                .to_string(),
        },
        machine,
        benchmarks,
    };

    let out_dir = root.join("benchmarks/results");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join(format!("{date}-{}.json", machine_slug(&archive.machine)));
    let json = serde_json::to_string_pretty(&archive).unwrap();
    std::fs::write(&out, json).unwrap();

    let mut by_group: BTreeMap<String, usize> = BTreeMap::new();
    for b in &archive.benchmarks {
        let group = b.id.split('/').take(2).collect::<Vec<_>>().join("/");
        *by_group.entry(group).or_insert(0) += 1;
    }
    println!(
        "wrote {} ({} benchmarks)",
        out.display(),
        archive.benchmarks.len()
    );
    for (group, count) in by_group {
        println!("  {group}: {count}");
    }
}
