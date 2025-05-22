use std::os::fd::FromRawFd;

use bevy_dmabuf::dmabuf::{DmabufBuffer, DmabufPlane};
use example_usages::TestInterfaceProxy;
use wlx_capture::{
    WlxCapture,
    frame::{Transform, WlxFrame},
    wlr_dmabuf,
};

#[tokio::main]
async fn main() {
    let wlx_client = wlx_capture::wayland::WlxClient::new().unwrap();
    let output_id = *wlx_client.outputs.iter().next().unwrap().0;
    println!("output_id: {output_id}");

    let mut wlx_capture = wlr_dmabuf::WlrDmabufCapture::<_, _>::new(wlx_client, output_id);
    let conn = zbus::connection::Connection::session().await.unwrap();
    let proxy = TestInterfaceProxy::builder(&conn).build().await.unwrap();
    wlx_capture.init(&[], (), |_, frame| {
        match &frame {
            WlxFrame::Dmabuf(dmabuf_frame) => println!("dmabuf"),
            WlxFrame::MemFd(mem_fd_frame) => println!("mem_fd"),
            WlxFrame::MemPtr(mem_ptr_frame) => println!("mem_ptr"),
        }

        if let WlxFrame::Dmabuf(dmabuf) = frame {
            return Some(DmabufBuffer {
                planes: dmabuf
                    .planes
                    .iter()
                    .filter_map(|plane| {
                        Some(DmabufPlane {
                            dmabuf_fd: plane.fd.map(|fd| fd.into())?,
                            offset: plane.offset,
                            stride: plane.stride,
                        })
                    })
                    .collect(),
                res: bevy_dmabuf::dmabuf::Resolution {
                    x: dmabuf.format.width,
                    y: dmabuf.format.height,
                },
                modifier: dmabuf.format.modifier,
                format: dmabuf.format.fourcc.value,
                flip_y: matches!(dmabuf.format.transform, Transform::Flipped),
            });
        }
        None
    });
    println!("resume");
    loop {
        wlx_capture.request_new_frame();
        if let Some(event) = wlx_capture.receive() {
            println!("frame!");
            _ = proxy.dmabuf(event).await;
        }
    }
}
