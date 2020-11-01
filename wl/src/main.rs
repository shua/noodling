use wayland_client::protocol::{
    wl_buffer::{self, WlBuffer},
    wl_compositor::{self, WlCompositor},
    wl_shm::{self, WlShm},
    wl_surface::{self, WlSurface},
};
use wayland_client::{global_filter, Display, Filter, GlobalManager, Main};
use wayland_protocols::xdg_shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

#[derive(Debug, PartialEq, Eq)]
enum SurfaceState {
    Created,
    Configured,
    Committed,
}

#[derive(Debug, PartialEq)]
struct Surface {
    wl: Main<WlSurface>,
    xdg: Main<XdgSurface>,
    toplevel: Main<XdgToplevel>,
    state: SurfaceState,
}

#[derive(Debug, Default)]
struct Data {
    compositor: Option<Main<WlCompositor>>,
    shm: Option<Main<WlShm>>,
    shm_formats: Vec<wl_shm::Format>,
    buffer: Option<Main<WlBuffer>>,
    wmbase: Option<Main<XdgWmBase>>,
    surface: Option<Surface>,
    xdg_surface: Option<(SurfaceState, Main<WlSurface>, Main<XdgSurface>)>,
    xdg_toplevel: Option<Main<XdgToplevel>>,
    should_close: bool,
}

impl Data {}

macro_rules! filter {
    ($data:pat, $($p:pat => $body:expr),+) => {
        Filter::new(move |(_, ev), _filter, mut ddata| {
            let $data = ddata.get::<Data>().expect("failed to get data");
            match ev {
                $($p => $body),+,
                _ => ()
            }
        })
    };
}

macro_rules! global_filter {
    ($([$interface:ty, $version:expr, $cb:expr]),*) => {
        wayland_client::global_filter!(
            $([$interface, $version, |x, mut ddata: DispatchData| {
                $cb(x, ddata.get::<Data>().expect("failed to get data"))
            }]),*
        )
    };
}

fn main() {
    let display = Display::connect_to_env().expect("failed to connect to display");
    let mut event_queue = display.create_event_queue();
    let disp_proxy = display.attach(event_queue.token());

    let _globals = GlobalManager::new_with_cb(
        &disp_proxy,
        global_filter!(
            // Bind all wl_seat with version 4
            [WlCompositor, 4, |x, data: &mut Data| {
                data.compositor.replace(x);
            }],
            [XdgWmBase, 2, |wmbase: Main<XdgWmBase>, data: &mut Data| {
                println!("setting wmbase");
                wmbase.assign(filter!(data,
                    xdg_wm_base::Event::Ping { serial } => {
                        let wmbase = data
                            .wmbase
                            .as_ref()
                            .expect("got a ping without wmbase object?");
                        wmbase.detach().pong(serial);
                    }
                ));
                data.wmbase.replace(wmbase);
            }],
            [WlShm, 1, |shm: Main<WlShm>, data: &mut Data| {
                shm.assign(filter!(data,
                    wl_shm::Event::Format { format } => {
                        data.shm_formats.push(format);
                    }
                ));
                data.shm.replace(shm);
            }]
        ),
    );

    let mut data = Data {
        should_close: false,
        ..Data::default()
    };
    while !data.should_close {
        event_queue
            .dispatch(&mut data, |ev, _proxy, _ddata| {
                if true || ev.interface != "wl_registry" {
                    eprintln!("{:?}", ev);
                }
            })
            .expect("An error occurred during event dispatch");

        if let (None, Some(shm)) = (&data.buffer, &data.shm) {
            let fd = nix::unistd::mkstemp("/dev/shm/shmbuf_XXXXXX")
                .and_then(|(fd, path)| nix::unistd::unlink(path.as_path()).and(Ok(fd)))
                .expect("failed to create temp file fd for shm");
            let width = 480;
            let height = 320;
            let stride = width * 4;
            let size: i32 = stride * height;

            nix::unistd::ftruncate(fd, size as i64).expect("failed calling ftruncate");

            let shmdata: *mut u32 = unsafe {
                let data = libc::mmap(
                    std::ptr::null_mut(),
                    size as usize,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                );
                if data == libc::MAP_FAILED || data.is_null() {
                    libc::close(fd);
                    panic!("map failed");
                }
                data as *mut u32
            };
            println!("fill shm data ({:?})", shmdata);
            // 4 bytes in a 32bit word, so size / 4 == size >> 2
            for i in 0..((size >> 2) as isize) {
                unsafe { *shmdata.offset(i) = 0xdd222222 }
            }

            let pool = shm.create_pool(fd, size);
            let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888);
            pool.destroy();

            data.buffer.replace(buffer);
        }

        if let (None, Some(compositor), Some(wmbase)) =
            (&data.surface, &data.compositor, &data.wmbase)
        {
            println!("setting xdg_surface");
            let wl = compositor.create_surface();
            let xdg = wmbase.get_xdg_surface(&wl.detach());
            let toplevel = xdg.get_toplevel();
            let appid = String::from("wlr-shlayer");
            toplevel.set_title(appid.clone());
            toplevel.set_app_id(appid);
            toplevel.assign(filter!(data,
                xdg_toplevel::Event::Close => data.should_close = true
            ));
            xdg.set_window_geometry(0, 0, 100, 100);
            xdg.assign(filter!(data,
                xdg_surface::Event::Configure { serial } => {
                    println!("xdg_surface Configure");
                    let surface = data.surface.as_mut().expect("surface configured without surface?");
                    surface.xdg.detach().ack_configure(serial);
                    surface.state = SurfaceState::Configured;
                }
            ));
            wl.commit();
            data.surface.replace(Surface {
                wl,
                xdg,
                toplevel,
                state: SurfaceState::Created,
            });
        }

        if let (
            Some(
                surface
                @
                Surface {
                    state: SurfaceState::Configured,
                    ..
                },
            ),
            Some(buffer),
        ) = (&mut data.surface, &data.buffer)
        {
            println!("buffer commit");
            surface.wl.attach(Some(buffer), 0, 0);
            surface.wl.commit();
            surface.state = SurfaceState::Committed;
        }
    }
}
