//! Turns targets on the command line into an ordered set of tasks.

use crate::task::Task;
use crate::tasks::{GROUPS, TASKS};

pub struct Plan {
    /// In registry order.
    pub tasks: Vec<Planned>,
}

pub struct Planned {
    pub task: &'static Task,
    /// Indices into `Plan::tasks` that must succeed first.
    pub needs: Vec<usize>,
    /// Indices into `Plan::tasks` that must finish first, in any state.
    pub after: Vec<usize>,
}

pub struct Selection<'a> {
    pub targets: &'a [String],
    pub skip: &'a [String],
    pub no_deps: bool,
}

pub fn resolve(selection: &Selection) -> Result<Plan, String> {
    let mut chosen = vec![false; TASKS.len()];
    for target in selection.targets {
        for i in expand(target)? {
            chosen[i] = true;
        }
    }
    if !selection.no_deps {
        // Registry order does not follow prerequisites, so repeat to a fixpoint.
        let mut changed = true;
        while changed {
            changed = false;
            for i in 0..TASKS.len() {
                if chosen[i] {
                    for need in TASKS[i].needs {
                        let j = index(need);
                        changed |= !chosen[j];
                        chosen[j] = true;
                    }
                }
            }
        }
    }
    let mut skipped = vec![false; TASKS.len()];
    for target in selection.skip {
        for i in expand(target)? {
            skipped[i] = true;
        }
    }
    for i in 0..TASKS.len() {
        if !chosen[i] || skipped[i] || selection.no_deps {
            continue;
        }
        if let Some(need) =
            TASKS[i].needs.iter().find(|need| skipped[index(need)])
        {
            return Err(format!(
                "{} needs {need}, which --skip removes; add --no-deps to assume its outputs exist",
                TASKS[i].id
            ));
        }
    }

    let planned: Vec<usize> = (0..TASKS.len())
        .filter(|&i| chosen[i] && !skipped[i])
        .collect();
    let position = |id: &str| planned.iter().position(|&i| TASKS[i].id == id);
    let mut tasks: Vec<Planned> = planned
        .iter()
        .map(|&i| {
            let task = &TASKS[i];
            Planned {
                task,
                needs: task
                    .needs
                    .iter()
                    .filter_map(|id| position(id))
                    .collect(),
                after: task
                    .after
                    .iter()
                    .filter_map(|id| position(id))
                    .collect(),
            }
        })
        .collect();
    for i in 0..tasks.len() {
        if tasks[i].task.last {
            tasks[i].after =
                (0..tasks.len()).filter(|&j| !tasks[j].task.last).collect();
        }
    }
    check_acyclic(&tasks)?;
    Ok(Plan { tasks })
}

/// The registry indices a target names: a task id, a group, or a glob over
/// task ids such as `fixtures-negative:*`.
fn expand(target: &str) -> Result<Vec<usize>, String> {
    if let Some(i) = TASKS.iter().position(|t| t.id == target) {
        return Ok(vec![i]);
    }
    if let Some(group) = GROUPS.iter().find(|g| g.name == target) {
        let mut found = Vec::new();
        for member in group.members {
            found.extend(expand(member)?);
        }
        return Ok(found);
    }
    if target.contains('*') {
        let found: Vec<usize> = (0..TASKS.len())
            .filter(|&i| glob(target, TASKS[i].id))
            .collect();
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Err(format!(
        "unknown task or group `{target}` (see `gnaw list`)"
    ))
}

fn index(id: &str) -> usize {
    TASKS
        .iter()
        .position(|t| t.id == id)
        .unwrap_or_else(|| panic!("no task `{id}`"))
}

/// Matches `*` against any run of characters.
fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((prefix, rest)) => {
            let Some(text) = text.strip_prefix(prefix) else {
                return false;
            };
            (0..=text.len())
                .any(|i| text.is_char_boundary(i) && glob(rest, &text[i..]))
        }
    }
}

fn check_acyclic(tasks: &[Planned]) -> Result<(), String> {
    let mut done = vec![false; tasks.len()];
    for _ in 0..tasks.len() {
        let Some(next) = (0..tasks.len()).find(|&i| {
            !done[i]
                && tasks[i]
                    .needs
                    .iter()
                    .chain(&tasks[i].after)
                    .all(|&j| done[j])
        }) else {
            let stuck: Vec<&str> = (0..tasks.len())
                .filter(|&i| !done[i])
                .map(|i| tasks[i].task.id)
                .collect();
            return Err(format!(
                "task ordering has a cycle among: {}",
                stuck.join(", ")
            ));
        };
        done[next] = true;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(
        targets: &[&str],
        skip: &[&str],
        no_deps: bool,
    ) -> Result<Vec<&'static str>, String> {
        let targets: Vec<String> =
            targets.iter().map(|s| s.to_string()).collect();
        let skip: Vec<String> = skip.iter().map(|s| s.to_string()).collect();
        let plan = resolve(&Selection {
            targets: &targets,
            skip: &skip,
            no_deps,
        })?;
        Ok(plan.tasks.iter().map(|p| p.task.id).collect())
    }

    #[test]
    fn registry_is_consistent() {
        for task in TASKS {
            for id in task.needs.iter().chain(task.after) {
                assert!(
                    TASKS.iter().any(|t| t.id == *id),
                    "{} names unknown task {id}",
                    task.id
                );
            }
            assert_eq!(
                TASKS.iter().filter(|t| t.id == task.id).count(),
                1,
                "duplicate {}",
                task.id
            );
        }
        for group in GROUPS {
            assert!(
                TASKS.iter().all(|t| t.id != group.name),
                "group {} shadows a task",
                group.name
            );
            for member in group.members {
                expand(member).unwrap();
            }
        }
        let all: Vec<String> = vec!["*".to_string()];
        resolve(&Selection {
            targets: &all,
            skip: &[],
            no_deps: false,
        })
        .unwrap();
    }

    #[test]
    fn adds_prerequisites() {
        assert_eq!(
            ids(&["fixtures:test"], &[], false).unwrap(),
            ["cli", "fixtures:rv32", "fixtures:test"]
        );
        assert_eq!(
            ids(&["fixtures:test"], &[], true).unwrap(),
            ["fixtures:test"]
        );
    }

    #[test]
    fn expands_globs_and_groups() {
        assert_eq!(
            ids(&["fixtures-negative:*"], &[], false).unwrap(),
            [
                "fixtures-negative:domain-collision",
                "fixtures-negative:dup-entrypoint",
                "fixtures-negative:registry-stripped"
            ]
        );
        assert!(
            ids(&["quick"], &[], false)
                .unwrap()
                .contains(&"version-check")
        );
        assert!(ids(&["nope"], &[], false).is_err());
    }

    #[test]
    fn skipping_a_prerequisite_needs_no_deps() {
        assert!(ids(&["fixtures:test"], &["fixtures:rv32"], false).is_err());
        assert_eq!(ids(&["fixtures"], &["lint:*"], false).unwrap().len(), 7);
    }

    #[test]
    fn last_task_follows_everything() {
        let targets = vec!["quick".to_string()];
        let plan = resolve(&Selection {
            targets: &targets,
            skip: &[],
            no_deps: false,
        })
        .unwrap();
        let last = plan
            .tasks
            .iter()
            .find(|p| p.task.id == "version-check")
            .unwrap();
        assert_eq!(last.after.len(), plan.tasks.len() - 1);
    }

    #[test]
    fn globs() {
        assert!(glob("lint:*", "lint:sdk"));
        assert!(glob("*:rv32", "itc:rv32"));
        assert!(!glob("lint:*", "fmt"));
    }
}
