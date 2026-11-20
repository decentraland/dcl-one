use super::Handler;

pub fn handler() -> Handler {
    Handler::None
}

pub fn register_handler() -> Result<(), anyhow::Error> {
    anyhow::bail!("deep-link sign-in is not supported on this platform")
}
