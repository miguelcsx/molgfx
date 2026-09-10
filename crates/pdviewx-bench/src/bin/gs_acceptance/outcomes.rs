fn blocked(spec: &GoldenSceneSpec, fixture: Option<&Path>, reason: &str) -> SceneEvidence {
    SceneEvidence {
        id: spec.id,
        title: spec.title,
        recipe: spec.recipe,
        outcome: SceneOutcome::Blocked,
        fixture: fixture.map(|path| path.display().to_string()),
        target_atoms: spec.target_atoms,
        observed_atoms: None,
        blockers: vec![reason.into()],
        candidate: None,
        metrics: None,
    }
}

fn failed(
    spec: &GoldenSceneSpec,
    fixture: &Path,
    observed_atoms: Option<u64>,
    reason: &str,
) -> SceneEvidence {
    SceneEvidence {
        id: spec.id,
        title: spec.title,
        recipe: spec.recipe,
        outcome: SceneOutcome::Failed,
        fixture: Some(fixture.display().to_string()),
        target_atoms: spec.target_atoms,
        observed_atoms,
        blockers: vec![reason.into()],
        candidate: None,
        metrics: None,
    }
}

fn unavailable_adapter() -> AdapterEvidence {
    AdapterEvidence {
        scope: "unavailable",
        capability_fingerprint: "unavailable".into(),
        runtime_name: Availability::Unavailable {
            reason: "no scene opened an adapter".into(),
        },
    }
}

fn panic_reason(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(reason) = payload.downcast_ref::<String>() {
        reason
    } else if let Some(reason) = payload.downcast_ref::<&str>() {
        reason
    } else {
        "non-string panic payload"
    }
}
