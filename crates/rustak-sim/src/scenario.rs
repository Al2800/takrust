use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    pub duration_ticks: u64,
    pub parameters: BTreeMap<String, i64>,
    pub metadata: BTreeMap<String, String>,
}

impl Scenario {
    #[must_use]
    pub fn new(name: impl Into<String>, seed: u64, duration_ticks: u64) -> Self {
        Self {
            name: name.into(),
            seed,
            duration_ticks,
            parameters: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<(), ScenarioError> {
        if self.name.is_empty() {
            return Err(ScenarioError::EmptyName);
        }
        if self.duration_ticks == 0 {
            return Err(ScenarioError::ZeroDurationTicks);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScenarioOverlay {
    pub name: Option<String>,
    pub seed: Option<u64>,
    pub duration_ticks: Option<u64>,
    pub parameters: BTreeMap<String, i64>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScenarioComposition {
    pub includes: Vec<Scenario>,
    pub overlay: ScenarioOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    NoIncludes,
    EmptyName,
    ZeroDurationTicks,
}

impl ScenarioComposition {
    pub fn compose(&self) -> Result<Scenario, ScenarioError> {
        let mut scenarios = self.includes.iter();
        let mut composed = scenarios.next().cloned().ok_or(ScenarioError::NoIncludes)?;

        for include in scenarios {
            composed = merge_include(composed, include.clone());
        }

        apply_overlay(&mut composed, &self.overlay);
        composed.validate()?;
        Ok(composed)
    }
}

fn merge_include(mut base: Scenario, include: Scenario) -> Scenario {
    base.name = include.name;
    base.seed = include.seed;
    base.duration_ticks = include.duration_ticks;

    for (key, value) in include.parameters {
        base.parameters.insert(key, value);
    }
    for (key, value) in include.metadata {
        base.metadata.insert(key, value);
    }

    base
}

fn apply_overlay(base: &mut Scenario, overlay: &ScenarioOverlay) {
    if let Some(name) = &overlay.name {
        base.name = name.clone();
    }
    if let Some(seed) = overlay.seed {
        base.seed = seed;
    }
    if let Some(duration_ticks) = overlay.duration_ticks {
        base.duration_ticks = duration_ticks;
    }
    for (key, value) in &overlay.parameters {
        base.parameters.insert(key.clone(), *value);
    }
    for (key, value) in &overlay.metadata {
        base.metadata.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use crate::scenario::{Scenario, ScenarioComposition, ScenarioError, ScenarioOverlay};

    #[test]
    fn include_chain_and_overlay_are_deterministic() {
        let mut base = Scenario::new("base", 1, 10);
        base.parameters.insert("noise_mm".to_string(), 10);
        base.metadata
            .insert("source".to_string(), "baseline".to_string());

        let mut include = Scenario::new("include", 2, 20);
        include.parameters.insert("dropout_ppm".to_string(), 25);
        include.metadata.insert("site".to_string(), "A".to_string());

        let mut overlay = ScenarioOverlay {
            name: Some("composed".to_string()),
            seed: Some(7),
            duration_ticks: Some(30),
            ..ScenarioOverlay::default()
        };
        overlay.parameters.insert("noise_mm".to_string(), 12);
        overlay.metadata.insert("site".to_string(), "B".to_string());

        let composition = ScenarioComposition {
            includes: vec![base.clone(), include.clone()],
            overlay: overlay.clone(),
        };
        let first = composition.compose().expect("compose");
        let second = composition.compose().expect("compose");

        assert_eq!(first, second);
        assert_eq!(first.name, "composed");
        assert_eq!(first.seed, 7);
        assert_eq!(first.duration_ticks, 30);
        assert_eq!(first.parameters["noise_mm"], 12);
        assert_eq!(first.parameters["dropout_ppm"], 25);
        assert_eq!(first.metadata["site"], "B");
        assert_eq!(first.metadata["source"], "baseline");
    }

    #[test]
    fn composition_rejects_zero_duration() {
        let base = Scenario::new("base", 1, 10);
        let overlay = ScenarioOverlay {
            duration_ticks: Some(0),
            ..ScenarioOverlay::default()
        };

        let composition = ScenarioComposition {
            includes: vec![base],
            overlay,
        };
        let error = composition.compose().expect_err("must fail");
        assert_eq!(error, ScenarioError::ZeroDurationTicks);
    }
}
