#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Btn {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Start,
    Select,
    Menu,
    VolUp,
    VolDown,
    Power,
    Lid,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RawEvent {
    Down(Btn),
    Up(Btn),
}

pub type Millis = u64;

/// An observed device event, before button or axis translation. Never synthesized from bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalEvent {
    pub node: String,
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

pub trait InputSource {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent>;

    fn analog_sticks(&self) -> [[i16; 2]; 2] {
        [[0; 2]; 2]
    }

    fn analog_available(&self) -> bool {
        false
    }

    /// Enable only for the user-controlled input screen; implementations must bound storage.
    fn set_monitoring(&mut self, _enabled: bool) {}

    fn physical_available(&self) -> bool {
        false
    }

    fn take_physical_events(&mut self) -> Vec<PhysicalEvent> {
        Vec::new()
    }
}
