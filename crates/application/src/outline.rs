//! Read-only navigation data. No PDF action is executed by this port.
#[derive(Clone, Debug)]
pub struct OutlineNode {
    pub title: String,
    pub parent: Option<usize>,
    pub page: Option<u16>,
    pub point: Option<[f32; 2]>,
    pub blocked: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Outline {
    pub nodes: Vec<OutlineNode>,
    pub limited: bool,
}
