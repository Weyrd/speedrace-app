pub trait StreamHandle: Send + Sync {
    fn is_live(&self) -> bool;
    fn stop(&self);
}
