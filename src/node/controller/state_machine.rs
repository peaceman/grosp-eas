use async_trait::async_trait;

#[derive(Debug)]
pub enum NodeMachine {
    Initializing(Data<Initializing>),
    Provisioning(Data<Provisioning>),
    Exploring(Data<Exploring>),
    Ready(Data<Ready>),
    Active(Data<Active>),
    Draining(Data<Draining>),
    Deprovisioning(Data<Deprovisioning>),
    Deprovisioned(Data<Deprovisioned>),
}

pub enum NodeMachineEvent {}

#[async_trait]
trait Handler {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine;
}

pub trait MachineState {}

#[derive(Debug)]
pub struct Data<S: MachineState> {
    state: S,
}

#[derive(Debug)]
pub struct Initializing {}

impl MachineState for Initializing {}

#[async_trait]
impl Handler for Data<Initializing> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Provisioning {}

impl MachineState for Provisioning {}

#[async_trait]
impl Handler for Data<Provisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Exploring {}

impl MachineState for Exploring {}

#[async_trait]
impl Handler for Data<Exploring> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Ready {}

impl MachineState for Ready {}

#[async_trait]
impl Handler for Data<Ready> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Active {}

impl MachineState for Active {}

#[async_trait]
impl Handler for Data<Active> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Draining {}

impl MachineState for Draining {}

#[async_trait]
impl Handler for Data<Draining> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Deprovisioning {}

impl MachineState for Deprovisioning {}

#[async_trait]
impl Handler for Data<Deprovisioning> {
    async fn handle(self, event: Option<NodeMachineEvent>) -> NodeMachine {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct Deprovisioned {}

impl MachineState for Deprovisioned {}

impl NodeMachine {
    pub fn new() -> Self {
        Self::Initializing(Data {
            state: Initializing {},
        })
    }

    pub async fn handle(self, event: Option<NodeMachineEvent>) -> Self {
        match self {
            Self::Initializing(m) => m.handle(event).await,
            Self::Provisioning(m) => m.handle(event).await,
            Self::Exploring(m) => m.handle(event).await,
            Self::Ready(m) => m.handle(event).await,
            Self::Active(m) => m.handle(event).await,
            Self::Draining(m) => m.handle(event).await,
            Self::Deprovisioning(m) => m.handle(event).await,
            Self::Deprovisioned(_) => self,
        }
    }
}
