//! Runs a plan: starts tasks whose prerequisites are done, up to the job
//! limit, one per lane.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::plan::Plan;
use crate::repo::Repo;
use crate::report::{Outcome, Renderer, Running, TaskResult};
use crate::task::{Ctx, Failure, Lane, Log, RunOptions, Stop};

pub struct ExecOptions {
    pub jobs: usize,
    pub keep_going: bool,
    /// A skipped task counts as failed.
    pub strict: bool,
}

enum State {
    Pending,
    Running(Instant, Arc<Log>),
    Done(Outcome),
}

pub fn execute(
    plan: &Plan,
    repo: &Repo,
    run: &RunOptions,
    exec: &ExecOptions,
    renderer: &mut dyn Renderer,
) -> Vec<TaskResult> {
    let mut states: Vec<State> =
        plan.tasks.iter().map(|_| State::Pending).collect();
    let mut results: Vec<Option<TaskResult>> =
        plan.tasks.iter().map(|_| None).collect();
    let mut stopping = false;
    let (send, receive) = mpsc::channel::<(usize, Outcome, Vec<String>)>();

    std::thread::scope(|scope| {
        loop {
            // Tasks whose prerequisites cannot succeed, or that a stop leaves
            // unstarted, finish without running.
            for i in 0..plan.tasks.len() {
                if !matches!(states[i], State::Pending) {
                    continue;
                }
                let blocked =
                    plan.tasks[i].needs.iter().find(|&&j| match &states[j] {
                        State::Done(outcome) => {
                            !outcome.satisfies_dependents()
                        }
                        _ => false,
                    });
                let reason = match blocked {
                    Some(&j) => Some(format!(
                        "needs {}, which did not succeed",
                        plan.tasks[j].task.id
                    )),
                    None if stopping => {
                        Some("stopped after an earlier failure".to_string())
                    }
                    None => None,
                };
                if let Some(reason) = reason {
                    let result =
                        TaskResult::not_run(plan.tasks[i].task, reason);
                    renderer.finished(&result);
                    states[i] = State::Done(result.outcome.clone());
                    results[i] = Some(result);
                }
            }

            let running = states
                .iter()
                .filter(|s| matches!(s, State::Running(..)))
                .count();
            if running == 0
                && states.iter().all(|s| matches!(s, State::Done(_)))
            {
                break;
            }

            // Start whatever is ready, in plan order.
            let mut running = running;
            for i in 0..plan.tasks.len() {
                if running >= exec.jobs {
                    break;
                }
                let planned = &plan.tasks[i];
                let ready = matches!(states[i], State::Pending)
                    && planned
                        .needs
                        .iter()
                        .chain(&planned.after)
                        .all(|&j| matches!(states[j], State::Done(_)))
                    && (planned.task.lane == Lane::Free
                        || !(0..plan.tasks.len()).any(|j| {
                            matches!(states[j], State::Running(..))
                                && plan.tasks[j].task.lane == planned.task.lane
                        }));
                if !ready {
                    continue;
                }
                let task = planned.task;
                let log = match Log::create(
                    log_path(repo, task.id),
                    renderer.streams(),
                ) {
                    Ok(log) => Arc::new(log),
                    Err(e) => {
                        let result = TaskResult::failed(
                            task,
                            Failure::new(format!("creating its log: {e}")),
                        );
                        renderer.finished(&result);
                        states[i] = State::Done(result.outcome.clone());
                        results[i] = Some(result);
                        stopping |= !exec.keep_going;
                        continue;
                    }
                };
                renderer.started(task.id);
                states[i] = State::Running(Instant::now(), log.clone());
                running += 1;
                let send = send.clone();
                scope.spawn(move || {
                    let mut ctx = Ctx::new(repo, run, log);
                    let outcome = match catch_unwind(AssertUnwindSafe(|| {
                        (task.run)(&mut ctx)
                    })) {
                        Ok(Ok(())) => Outcome::Ok,
                        Ok(Err(Stop::Skip(reason))) => {
                            Outcome::Skipped(reason)
                        }
                        Ok(Err(Stop::Fail(failure))) => {
                            Outcome::Failed(failure)
                        }
                        Err(panic) => Outcome::Failed(Failure::new(format!(
                            "gnaw panicked: {}",
                            panic_message(&panic)
                        ))),
                    };
                    let _ = send.send((i, outcome, ctx.into_warnings()));
                });
            }

            match receive.recv_timeout(Duration::from_millis(200)) {
                Ok((i, outcome, warnings)) => {
                    let State::Running(started, log) = &states[i] else {
                        unreachable!()
                    };
                    let outcome = match outcome {
                        Outcome::Skipped(reason) if exec.strict => {
                            Outcome::Failed(Failure::new(format!(
                                "skipped under --strict: {reason}"
                            )))
                        }
                        outcome => outcome,
                    };
                    stopping |= matches!(outcome, Outcome::Failed(_))
                        && !exec.keep_going;
                    let result = TaskResult {
                        id: plan.tasks[i].task.id,
                        outcome,
                        elapsed: started.elapsed(),
                        warnings,
                        log: Some(log.path.clone()),
                    };
                    renderer.finished(&result);
                    states[i] = State::Done(result.outcome.clone());
                    results[i] = Some(result);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let running: Vec<Running> = plan
                        .tasks
                        .iter()
                        .zip(&states)
                        .filter_map(|(p, s)| match s {
                            State::Running(started, log) => Some(Running {
                                id: p.task.id,
                                elapsed: started.elapsed(),
                                log: log.clone(),
                            }),
                            _ => None,
                        })
                        .collect();
                    renderer.tick(&running);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    unreachable!("the scheduler holds a sender")
                }
            }
        }
    });
    results.into_iter().map(Option::unwrap).collect()
}

pub fn log_path(repo: &Repo, id: &str) -> std::path::PathBuf {
    repo.logs().join(format!("{}.log", id.replace(':', "-")))
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}
