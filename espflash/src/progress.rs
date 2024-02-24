/// Progress update callbacks
pub trait ProgressCallbacks {
    /// Initialize some progress report
    fn init(&mut self, addr: u32, total: usize);
    /// Update some progress report
    fn update(&mut self, current: usize);
    /// Finish some progress report
    fn finish(&mut self);
}
