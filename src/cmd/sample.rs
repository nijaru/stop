//! `stop sample` — collect a bounded process and system time series.

use std::time::Instant;

use chrono::Utc;

use crate::cli::SampleArgs;
use crate::cmd::Outcome;
use crate::collector;
use crate::error::StopError;
use crate::model::{SamplePoint, SampleReport};
use crate::output;

pub fn run(args: &SampleArgs) -> Result<Outcome, StopError> {
    let period = args.period();
    let started_at = Utc::now().to_rfc3339();
    let mut sampler = collector::Sampler::new();
    let mut samples = Vec::with_capacity(args.count);
    let mut next_deadline = Instant::now();

    for index in 0..args.count {
        if let Some(delay) = next_deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(delay);
        }

        // The first accurate point needs the CPU baseline warm-up. Later
        // points use the interval since the previous refresh as their delta.
        let warm_up_cpu = !args.collection.fast && index == 0;
        let include_cpu = !args.collection.fast;
        let (system, mut processes) = sampler.sample(warm_up_cpu, include_cpu)?;
        processes.sort_by_key(|process| process.pid);
        samples.push(SamplePoint {
            collected_at: Utc::now().to_rfc3339(),
            total_processes: processes.len(),
            system,
            processes,
        });

        next_deadline = next_deadline
            .checked_add(period)
            .unwrap_or_else(Instant::now);
    }

    let report = SampleReport {
        started_at,
        interval_ms: period.as_millis() as u64,
        count: args.count,
        samples,
    };

    if args.output.json {
        output::print_json(&report, args.output.pretty)?;
    } else {
        for (index, sample) in report.samples.iter().enumerate() {
            output::print_sample_point(sample, index + 1, report.count)?;
        }
    }

    Ok(Outcome::Success)
}
