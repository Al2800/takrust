use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepAxis {
    pub name: String,
    pub values: Vec<i64>,
}

impl SweepAxis {
    pub fn new(name: impl Into<String>, values: Vec<i64>) -> Result<Self, SweepError> {
        let name = name.into();
        if name.is_empty() {
            return Err(SweepError::EmptyAxisName);
        }
        if values.is_empty() {
            return Err(SweepError::EmptyAxisValues { axis: name });
        }
        Ok(Self { name, values })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepCase {
    pub index: usize,
    pub case_id: u64,
    pub parameters: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepOutcome<T> {
    pub case: SweepCase,
    pub result: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepReport<T> {
    pub total_cases: usize,
    pub executed_cases: usize,
    pub next_start_index: Option<usize>,
    pub outcomes: Vec<SweepOutcome<T>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SweepRunOptions {
    pub start_index: usize,
    pub max_cases: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepError {
    EmptyAxisName,
    EmptyAxisValues { axis: String },
    DuplicateAxisName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepRunner {
    axes: Vec<SweepAxis>,
    seed: u64,
}

impl SweepRunner {
    pub fn new(axes: Vec<SweepAxis>, seed: u64) -> Result<Self, SweepError> {
        let mut seen = std::collections::BTreeSet::new();
        for axis in &axes {
            if !seen.insert(axis.name.clone()) {
                return Err(SweepError::DuplicateAxisName);
            }
        }
        Ok(Self { axes, seed })
    }

    #[must_use]
    pub fn enumerate_cases(&self) -> Vec<SweepCase> {
        let mut current = BTreeMap::new();
        let mut out = Vec::new();
        self.enumerate_recursive(0, &mut current, &mut out);
        out
    }

    pub fn execute<T, F>(&self, options: SweepRunOptions, mut evaluator: F) -> SweepReport<T>
    where
        F: FnMut(&SweepCase) -> T,
    {
        let cases = self.enumerate_cases();
        let total = cases.len();
        let start = options.start_index.min(total);
        let end = options
            .max_cases
            .map_or(total, |max| start.saturating_add(max).min(total));

        let mut outcomes = Vec::with_capacity(end.saturating_sub(start));
        for case in cases[start..end].iter().cloned() {
            let result = evaluator(&case);
            outcomes.push(SweepOutcome { case, result });
        }

        SweepReport {
            total_cases: total,
            executed_cases: outcomes.len(),
            next_start_index: (end < total).then_some(end),
            outcomes,
        }
    }

    fn enumerate_recursive(
        &self,
        axis_index: usize,
        current: &mut BTreeMap<String, i64>,
        out: &mut Vec<SweepCase>,
    ) {
        if axis_index == self.axes.len() {
            let case_id = stable_case_id(self.seed, current);
            out.push(SweepCase {
                index: out.len(),
                case_id,
                parameters: current.clone(),
            });
            return;
        }

        let axis = &self.axes[axis_index];
        for value in &axis.values {
            current.insert(axis.name.clone(), *value);
            self.enumerate_recursive(axis_index + 1, current, out);
        }
    }
}

fn stable_case_id(seed: u64, parameters: &BTreeMap<String, i64>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    hash = fnv1a(hash, &seed.to_le_bytes());
    for (name, value) in parameters {
        hash = fnv1a(hash, name.as_bytes());
        hash = fnv1a(hash, &value.to_le_bytes());
    }
    hash
}

fn fnv1a(mut state: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x100_0000_01b3);
    }
    state
}

#[cfg(test)]
mod tests {
    use crate::sweep::{SweepAxis, SweepRunOptions, SweepRunner};

    #[test]
    fn grid_enumeration_is_deterministic() {
        let axes = vec![
            SweepAxis::new("dropout_ppm", vec![0, 25]).expect("axis"),
            SweepAxis::new("sensor_noise_mm", vec![10, 20, 30]).expect("axis"),
        ];
        let runner = SweepRunner::new(axes, 7).expect("runner");

        let first = runner.enumerate_cases();
        let second = runner.enumerate_cases();

        assert_eq!(first, second);
        assert_eq!(first.len(), 6);
        assert_eq!(first[0].index, 0);
        assert_eq!(first[5].index, 5);
    }

    #[test]
    fn execute_supports_resumable_windows() {
        let axes = vec![
            SweepAxis::new("a", vec![1, 2]).expect("axis"),
            SweepAxis::new("b", vec![3, 4]).expect("axis"),
        ];
        let runner = SweepRunner::new(axes, 42).expect("runner");

        let first = runner.execute(
            SweepRunOptions {
                start_index: 0,
                max_cases: Some(2),
            },
            |case| case.case_id,
        );
        assert_eq!(first.executed_cases, 2);
        assert_eq!(first.next_start_index, Some(2));

        let second = runner.execute(
            SweepRunOptions {
                start_index: first.next_start_index.expect("next"),
                max_cases: Some(2),
            },
            |case| case.case_id,
        );
        assert_eq!(second.executed_cases, 2);
        assert_eq!(second.next_start_index, None);
    }
}
