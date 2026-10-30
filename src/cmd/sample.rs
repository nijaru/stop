//! Finite, scoped sampling. Scheduling skips missed slots instead of catching up.

use chrono::Utc;
use std::time::{Duration, Instant};

use super::Outcome;
use crate::cli::{DEFAULT_CPU_WINDOW, OutputArgs, SampleArgs};
use crate::collector::{self, Request, Sampler, TargetObservation};
use crate::error::StopError;
use crate::model::{ProcessIdentity, ProcessInfo, SamplePoint, SampleReport, Snapshot, SortKey};
use crate::output;
use crate::runtime::Cancellation;

struct Schedule {
    next: Instant,
    period: Duration,
}
impl Schedule {
    fn advance(&mut self, now: Instant) -> usize {
        self.next += self.period;
        if self.next >= now {
            return 0;
        }
        let missed = now.duration_since(self.next).as_nanos() / self.period.as_nanos() + 1;
        // A run is bounded to 1000 points and a period of at least 200ms.
        let missed =
            u32::try_from(missed).expect("overrun fits in 27 years of minimum-period slots");
        self.next += self.period * missed;
        missed as usize
    }
}

#[derive(Default)]
struct PinnedTarget {
    identity: Option<ProcessIdentity>,
}
impl PinnedTarget {
    fn observe(&mut self, identity: ProcessIdentity) -> bool {
        if let Some(bound) = &self.identity {
            *bound == identity
        } else {
            self.identity = Some(identity);
            true
        }
    }

    fn restrict(
        &mut self,
        target: &TargetObservation,
        rows: &mut Vec<ProcessInfo>,
    ) -> Result<String, StopError> {
        let observed = match target {
            TargetObservation::Observed(identity) => self.observe(identity.clone()),
            TargetObservation::Unavailable if self.identity.is_none() => {
                return Err(StopError::Operational(
                    "cannot sample this PID: process birth ID unavailable".into(),
                ));
            }
            _ => false,
        };
        if let Some(identity) = &self.identity {
            rows.retain(|p| {
                p.pid == identity.pid && p.birth_id.as_deref() == Some(identity.birth_id.as_str())
            });
            Ok(if observed {
                "observed"
            } else {
                "no_longer_observed"
            }
            .into())
        } else {
            Ok("not_observed_yet".into())
        }
    }
}

pub fn run(
    args: &SampleArgs,
    output_args: &OutputArgs,
    cancel: &Cancellation,
) -> Result<Outcome, StopError> {
    let period = args.period();
    let interval_ms = collector::millis(period);
    let started_at = Utc::now().to_rfc3339();
    let mut sampler = Sampler::new(Request::query(&args.query, true));
    let mut pin = PinnedTarget::default();
    let mut baseline = sampler.sample()?;
    let mut failure = if args.query.pid.is_some() {
        pin.restrict(&baseline.target, &mut baseline.processes)
            .err()
    } else {
        None
    };
    let mut samples = Vec::new();
    let mut emitted = 0;
    let mut any_match = false;
    if failure.is_none()
        && let Err(err) = cancel.wait_until(Instant::now() + DEFAULT_CPU_WINDOW)
    {
        return Err(err.into());
    }
    let mut schedule = Schedule {
        next: Instant::now(),
        period,
    };
    let mut skipped_intervals = 0;
    while failure.is_none() && emitted < args.count {
        let deadline = sampler
            .next_cpu_deadline()
            .map(|ready| ready.max(schedule.next))
            .unwrap_or(schedule.next);
        if !cancel.wait_until(deadline)? {
            break;
        }
        let mut data = match sampler.sample() {
            Ok(data) => data,
            Err(err) => {
                failure = Some(err);
                break;
            }
        };
        let target_status = if args.query.pid.is_some() {
            match pin.restrict(&data.target, &mut data.processes) {
                Ok(status) => Some(status),
                Err(err) => {
                    failure = Some(err);
                    break;
                }
            }
        } else {
            None
        };
        args.query
            .sort
            .unwrap_or(SortKey::Cpu)
            .sort_processes(&mut data.processes);
        let snapshot = Snapshot::finish(
            data.metadata,
            data.total_processes,
            data.processes,
            Some(args.query.limit.unwrap_or(10)),
        );
        any_match |= snapshot.returned > 0;
        let point = SamplePoint {
            index: emitted + 1,
            requested_count: args.count,
            interval_ms,
            skipped_intervals,
            target_status,
            snapshot,
            system: data.system,
        };
        if output_args.jsonl {
            output::print_json(&point, false, cancel)?;
        } else if !output_args.document() {
            output::print_sample_point(&point, cancel)?;
        }
        if output_args.document() {
            samples.push(point);
        }
        emitted += 1;
        // Include rendering and pipe backpressure in overrun handling.
        if emitted < args.count {
            skipped_intervals = schedule.advance(Instant::now());
        }
    }
    // These flags describe collection, before final delivery. A later signal
    // cannot retroactively change metadata already sent to a consumer.
    let interrupted = cancel.interrupted();
    if output_args.document() {
        output::print_json(
            &SampleReport {
                schema_version: crate::model::SCHEMA_VERSION,
                started_at,
                interval_ms,
                requested_count: args.count,
                count: emitted,
                completed: emitted == args.count && failure.is_none() && !interrupted,
                interrupted,
                samples,
            },
            output_args.pretty,
            cancel,
        )?;
    }
    if let Some(err) = failure {
        return Err(err);
    }
    if interrupted {
        return Err(StopError::Interrupted);
    }
    Ok(if any_match {
        Outcome::Success
    } else {
        Outcome::NoMatch
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_pid_does_not_rebind_after_reuse() {
        let mut pin = PinnedTarget::default();
        let original = ProcessIdentity {
            pid: 42,
            birth_id: "original".into(),
        };
        // Even when additional selectors leave no returned rows, the PID is bound.
        let mut rows = vec![];
        pin.restrict(&TargetObservation::Observed(original.clone()), &mut rows)
            .unwrap();
        pin.restrict(&TargetObservation::NotObserved, &mut rows)
            .unwrap();
        pin.restrict(
            &TargetObservation::Observed(ProcessIdentity {
                pid: 42,
                birth_id: "replacement".into(),
            }),
            &mut rows,
        )
        .unwrap();
        assert_eq!(pin.identity, Some(original));
    }

    #[test]
    fn overrun_skips_slots_and_never_catches_up() {
        let origin = Instant::now();
        let mut schedule = Schedule {
            next: origin,
            period: Duration::from_secs(1),
        };
        assert_eq!(schedule.advance(origin + Duration::from_millis(2500)), 2);
        assert_eq!(schedule.next, origin + Duration::from_secs(3));
        assert_eq!(schedule.advance(origin + Duration::from_millis(3100)), 0);
        assert_eq!(schedule.next, origin + Duration::from_secs(4));
    }
}
