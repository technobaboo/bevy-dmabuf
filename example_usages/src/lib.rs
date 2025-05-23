use std::sync::mpsc;

use bevy_dmabuf::dmatex::Dmatex;
pub struct TestInterface {
    pub dmatex_channel: mpsc::Sender<Dmatex>,
}

#[zbus::interface(
    name = "dev.schmarni.bevy_dmabuf.dmatex",
    proxy(
        default_service = "dev.schmarni.bevy_dmabuf.dmatex",
        default_path = "/dev/schmarni/bevy_dmabuf/dmatex"
    )
)]
impl TestInterface {
    fn dmatex(&self, dmabuf: Dmatex) {
        _ = self.dmatex_channel.send(dmabuf);
    }
}
