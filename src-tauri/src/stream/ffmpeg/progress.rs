#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct ProgressBlock {
    pub(super) frame: Option<u64>,
}

#[derive(Default)]
pub(super) struct ProgressParser {
    pending: ProgressBlock,
}

impl ProgressParser {
    pub(super) fn feed(&mut self, line: &str) -> Option<ProgressBlock> {
        let (key, value) = line.split_once('=')?;
        let value = value.trim();
        match key {
            "frame" => self.pending.frame = value.parse().ok(),
            "progress" => return Some(std::mem::take(&mut self.pending)),
            _ => {}
        }
        None
    }
}
