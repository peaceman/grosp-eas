use crate::node_groups::NodeGroup;
use act_zero::{act_zero, Actor, Addr, Local, Sender};
use log::info;
use std::fmt;
use std::time::Duration;

pub struct NodeGroupScaler {
    node_group: NodeGroup,
    should_terminate: bool,
}

impl NodeGroupScaler {
    pub fn new(node_group: NodeGroup) -> Self {
        NodeGroupScaler {
            node_group,
            should_terminate: false,
        }
    }
}

impl fmt::Display for NodeGroupScaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeGroupScaler {}", self.node_group.name)
    }
}

impl Actor for NodeGroupScaler {
    type Error = ();

    fn started(&mut self, addr: Addr<Local<NodeGroupScaler>>) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        info!("Started {}", self);

        addr.timer_loop(Duration::from_secs(1));

        Ok(())
    }

    fn should_terminate(&self) -> bool {
        self.should_terminate
    }
}

impl Drop for NodeGroupScaler {
    fn drop(&mut self) {
        info!("Drop {}", self);
    }
}

#[act_zero]
pub trait NodeGroupScalerTrait {
    fn timer_loop(&self, period: Duration);
    fn terminate(&self);
    fn is_alive(&self, res: Sender<bool>);
    fn scale(&self);
}

#[act_zero]
impl NodeGroupScalerTrait for NodeGroupScaler {
    async fn timer_loop(self: Addr<Local<NodeGroupScaler>>, period: Duration) {
        info!("Start timer with a period of {:?}", period);

        let mut interval = tokio::time::interval(period);

        loop {
            interval.tick().await;
            self.scale();
        }
    }

    async fn terminate(&mut self) {
        self.should_terminate = true;
    }

    async fn is_alive(self: Addr<Local<NodeGroupScaler>>, res: Sender<bool>) {
        res.send(true).ok();
    }

    async fn scale(&mut self) {
        info!("Scale {}", self.node_group.name);
    }
}
