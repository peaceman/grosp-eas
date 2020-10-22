use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    pub provisioning_timeout: Duration,
    pub draining_time: Duration,
}
