use std::fmt::Debug;

pub trait HostnameGenerator: Send + Sync + Debug {
    fn generate_hostname(&self, node_group_name: &str) -> String;
}

impl HostnameGenerator for String {
    fn generate_hostname(&self, node_group_name: &str) -> String {
        format!("{}-{}.{}", node_group_name, generate_random_part(), self)
    }
}

fn generate_random_part() -> String {
    let charset: Vec<char> = ('a'..='z').collect();

    use rand::prelude::*;
    let mut rng = rand::thread_rng();

    let mut hostname = String::with_capacity(8);
    for i in 0..8 {
        hostname.push(charset[rng.gen_range(0, charset.len())]);
    }

    hostname
}

impl HostnameGenerator for Vec<&str> {
    fn generate_hostname(&self, node_group_name: &str) -> String {
        use rand::prelude::*;

        let mut rng = rand::thread_rng();
        let mut hostname = self[rng.gen_range(0, self.len())];

        format!("{}-{}", node_group_name, hostname)
    }
}

impl HostnameGenerator for Vec<char> {
    fn generate_hostname(&self, node_group_name: &str) -> String {
        use rand::prelude::*;

        let mut rng = rand::thread_rng();
        let mut hostname = self[rng.gen_range(0, self.len())];

        format!("{}-{}", node_group_name, hostname)
    }
}
