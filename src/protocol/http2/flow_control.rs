use webparse::http2::WindowSize;

#[derive(Debug)]
pub struct FlowControl {
    window_size: i32,
    available: i32,
}

impl FlowControl {
    pub fn new(default: WindowSize) -> Self {
        Self {
            window_size: default as i32,
            available: default as i32,
        }
    }

    pub fn is_available(&self) -> bool {
        self.available > 0
    }
}
