use std::{
    os::fd::{BorrowedFd, FromRawFd, IntoRawFd, OwnedFd},
    sync::Arc,
    time::Duration,
};

use bevy_dmabuf::dmatex::{Dmatex, DmatexPlane, Resolution};
use example_usages::TestInterfaceProxy;
use tokio::{sync::Notify, time::timeout};
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
    let notify = Arc::new(Notify::new());
    wlx_capture.init(&[], notify.clone(), |notify, frame| {
        notify.notify_one();
        match &frame {
            WlxFrame::Dmabuf(_) => println!("dmabuf"),
            WlxFrame::MemFd(_) => println!("mem_fd"),
            WlxFrame::MemPtr(_) => println!("mem_ptr"),
        }

        if let WlxFrame::Dmabuf(dmabuf) = frame {
            // i *think* wlx-capture automatically closes that dmabuf? and sometimes its already
            // invalid
            let fd = unsafe { BorrowedFd::borrow_raw(dmabuf.planes.first()?.fd?) };
            let cloned_fd = match fd.try_clone_to_owned() {
                Ok(fd) => fd,
                Err(err) => {
                    println!("unable to clone fd: {err}");
                    return None;
                }
            };

            return Some(Dmatex {
                dmabuf_fd: cloned_fd.into(),
                planes: dmabuf
                    .planes
                    .iter()
                    .filter(|p| p.fd.is_some())
                    .map(|plane| DmatexPlane {
                        offset: plane.offset,
                        stride: plane.stride,
                    })
                    .collect(),
                res: Resolution {
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
    loop {
        wlx_capture.request_new_frame();
        _ = timeout(Duration::from_millis(250), notify.notified()).await;
        if let Some(event) = wlx_capture.receive() {
            _ = proxy.dmatex(event).await;
        }
    }
}
