use std::sync::mpsc;

use bevy_dmabuf::dmabuf::DmabufBuffer;
pub struct TestInterface {
    pub dmabuf_channel: mpsc::Sender<DmabufBuffer>,
}

fn test(proxy: zbus::Proxy) {


}
#[zbus::interface(
    name = "dev.schmarni.bevy_dmabuf.example",
    proxy(
        default_service = "dev.schmarni.bevy_dmabuf.example",
        default_path = "/dev/schmarni/bevy_dmabuf"
    )
)]
impl TestInterface {
    fn dmabuf(&self, dmabuf: DmabufBuffer) {
        _ = self.dmabuf_channel.send(dmabuf);
    }
    // fn semaphore(&self) -> Fd {
    //     Fd::Borrowed(self.semaphore.as_fd())
    // }
}
