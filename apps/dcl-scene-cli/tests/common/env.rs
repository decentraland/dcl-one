use super::testgate::unavailable;

fn env_value(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

pub fn require_env(var: &str) -> Option<String> {
    match env_value(var) {
        Some(v) => Some(v),
        None => unavailable(var, "the variable is unset"),
    }
}
