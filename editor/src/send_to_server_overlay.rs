pub struct SendToServerOverlay {
    pub status: SendToServerOverlayStatus,
}

impl SendToServerOverlay {
    pub fn new() -> Self {
        Self { status: SendToServerOverlayStatus::Ready }
    }

    pub fn mark_as_submitting(&mut self) {
        self.status = SendToServerOverlayStatus::Submitting;
    }

    pub fn mark_error(&mut self, desc: String) {
        self.status = SendToServerOverlayStatus::Error(desc);
    }

    pub fn mark_as_success(&mut self) {
        self.status = SendToServerOverlayStatus::Success;
    }
}

pub enum SendToServerOverlayStatus {
    Ready,
    Submitting,
    Error(String),
    Success,
}
