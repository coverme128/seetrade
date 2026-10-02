//! Optional local language-model configuration. It grants no engine write capability.
pub fn configured_model(mode: Option<&str>, model: Option<&str>) -> Result<Option<String>, String> {
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    // Preserve the existing opt-in by model name when the explicit mode is absent.
    match mode.unwrap_or(if model.is_some() { "ASSIST" } else { "OFF" }) {
        "OFF" => Ok(None),
        "ASSIST" => model
            .map(|m| Some(m.to_owned()))
            .ok_or_else(|| "AI_MODE=ASSIST 需要 MICRO_ALPHA_ASSIST_MODEL".into()),
        _ => Err("MICRO_ALPHA_AI_MODE 只能为 OFF 或 ASSIST".into()),
    }
}
pub fn from_env() -> Result<Option<String>, String> {
    configured_model(
        std::env::var("MICRO_ALPHA_AI_MODE").ok().as_deref(),
        std::env::var("MICRO_ALPHA_ASSIST_MODEL").ok().as_deref(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_off_disables_model_and_assist_requires_local_model() {
        assert_eq!(configured_model(None, None).unwrap(), None);
        assert_eq!(
            configured_model(Some("OFF"), Some("local-model")).unwrap(),
            None
        );
        assert_eq!(
            configured_model(Some("ASSIST"), Some("local-model")).unwrap(),
            Some("local-model".into())
        );
        assert!(configured_model(Some("ASSIST"), Some(" ")).is_err());
        assert!(configured_model(Some("AUTO"), Some("model")).is_err());
    }
}
