pub trait GetDebugInfo {
    type Output;

    fn dbg_info(&self) -> Self::Output;
}
