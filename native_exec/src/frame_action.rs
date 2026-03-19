/// Replaces the bool return from MockInvocationFrame::execute().
/// Continue = keep frame open (was: true)
/// Close = close frame (was: false)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    Continue,
    Close,
}

impl FrameAction {
    /// Convert to the bool expected by MockInvocationFrame::execute()
    pub(crate) fn as_bool(self) -> bool {
        match self {
            FrameAction::Continue => true,
            FrameAction::Close => false,
        }
    }
}
