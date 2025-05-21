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
    let mut wlr_dmabuf = wlr_dmabuf::WlrDmabufCapture::<_, _>::new(wlx_client, output_id);
    let conn = zbus::connection::Connection::session().await.unwrap();
    let proxy = TestInterfaceProxy::builder(&conn).build().await.unwrap();
    // println!("init");
    wlr_dmabuf.init(&[], (), |_, frame| {
        // println!("event!");

        if let WlxFrame::Dmabuf(dmabuf) = frame {
            // println!("format: {:?}", dmabuf.format.fourcc.value);
            // println!(
            //     "modifier: {:?}",
            //     drm_fourcc::DrmModifier::from(dmabuf.format.modifier)
            // );
            return Some(DmabufBuffer {
                planes: dmabuf
                    .planes
                    .iter()
                    .filter_map(|plane| {
                        Some(DmabufPlane {
                            dmabuf_fd: plane.fd.map(|fd| {
                                unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) }.into()
                            })?,
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
            // for plane in dmabuf.planes {
            //     if let Some(v) = plane.fd {
            //         // println!("fd: {}", v);
            //         // println!("offset: {}, stride: {}", plane.offset, plane.stride);
            //     }
            // }
            // data.dmabuf(zbus::zvariant::Fd::Borrowed(dmabuf))
        }
        None
    });
    println!("resume");
    loop {
        wlr_dmabuf.request_new_frame();
        if let Some(event) = wlr_dmabuf.receive() {
            proxy
                .inner()
                .call::<_, _, ()>("Dmabuf", &::zbus::zvariant::DynamicTuple((&event,)))
                .await
                .unwrap();

            event
                .planes
                .into_iter()
                .for_each(|p| std::mem::forget(p.dmabuf_fd));
            // _ = proxy.dmabuf(event).await;
        }
    }
    // _ = tokio::signal::ctrl_c().await;
}
