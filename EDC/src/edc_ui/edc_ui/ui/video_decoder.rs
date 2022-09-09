pub trait VideoDecoder {
    fn new(
        window: &Window,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<MPVCtx>;
    fn paint(&mut self, _window: &Window);
    fn handle_window_event(&self, _window_id: WindowId, event: WindowEvent);
    fn handle_user_event(&self, window: &Window, _ctrl_flow: &ControlFlow, event: &MPVEvent):
    fn needs_evloop_proxy(&mut self) -> bool;
}
