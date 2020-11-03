use rusttype::{self, point, Scale};
use wayland_client::protocol::{
    wl_buffer::{self, WlBuffer},
    wl_compositor::{self, WlCompositor},
    wl_seat::{self, WlSeat},
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
    buffer_committed: bool,
    state: SurfaceState,
}

#[derive(Debug, Default)]
struct Data {
    compositor: Option<Main<WlCompositor>>,
    seat: Option<Main<WlSeat>>,
    seat_cap: Option<wl_seat::Capability>,
    shm: Option<Main<WlShm>>,
    shm_formats: Vec<wl_shm::Format>,
    buffer: Option<Main<WlBuffer>>,
    wmbase: Option<Main<XdgWmBase>>,
    surface: Option<Surface>,
    xdg_toplevel: Option<Main<XdgToplevel>>,
    should_close: bool,
}

macro_rules! filter2 {
    ($self:ident, $data:ident, $($p:pat => $body:expr),*) => {
        $self.assign(
            Filter::new(move |(_, ev), _filter, mut ddata| {
                let $data = ddata.get::<Data>().expect("failed to get data");
                match ev {
                    $($p => $body),+,
                    _ => {},
                }
            }
        ))
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

struct Font {
    font: rusttype::Font<'static>,
    scale: Scale,
    offset: rusttype::Point<f32>,
}

struct Glyphs<'f> {
    glyphs: Vec<rusttype::PositionedGlyph<'f>>,
    width: f32,
    height: f32,
}

impl Font {
    fn new() -> Font {
        let font =
            rusttype::Font::try_from_bytes(include_bytes!("../SourceCodePro-Regular.otf") as &[u8])
                .expect("error constructing a Font from bytes");
        let scale = Scale::uniform(100.0);
        let v_metrics = font.v_metrics(scale);
        let offset = point(0.0, v_metrics.ascent);

        Font {
            font,
            scale,
            offset,
        }
    }

    fn glyphs(&self, s: &str) -> Glyphs {
        let glyphs: Vec<_> = self.font.layout(s, self.scale, self.offset).collect();
        let width = glyphs
            .last()
            .map(|g| g.position().x as f32 + g.unpositioned().h_metrics().advance_width)
            .unwrap_or(0.0);

        Glyphs {
            glyphs,
            width,
            height: self.scale.y,
        }
    }
}

impl<'f> Glyphs<'f> {
    fn render(self, d: impl Fn(usize, usize, u8)) {
        let width = self.width.ceil();
        let height = self.height.ceil();
        for g in self.glyphs {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|x, y, v| {
                    let v = (v * 255.0).ceil() as u8;
                    let x = x as i32 + bb.min.x;
                    let y = y as i32 + bb.min.y;
                    if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                        d(x as usize, y as usize, v);
                    }
                })
            }
        }
    }
}

fn main() {
    let display = Display::connect_to_env().expect("failed to connect to display");
    let mut event_queue = display.create_event_queue();
    let disp_proxy = display.attach(event_queue.token());

    let _globals = GlobalManager::new_with_cb(
        &disp_proxy,
        global_filter!(
            [WlCompositor, 4, |x, data: &mut Data| {
                data.compositor.replace(x);
            }],
            [WlSeat, 4, |seat: Main<WlSeat>, data: &mut Data| {
                filter2!(seat, data,
                    wl_seat::Event::Capabilities{capabilities} => {
                        println!("seat caps {:?}", capabilities);
                        data.seat_cap.replace(capabilities);
                    }
                );
                data.seat.replace(seat);
            }],
            [XdgWmBase, 2, |wmbase: Main<XdgWmBase>, data: &mut Data| {
                println!("setting wmbase");
                filter2!(wmbase, data,
                    xdg_wm_base::Event::Ping { serial } => {
                        let wmbase = data
                            .wmbase
                            .as_ref()
                            .expect("got a ping without wmbase object?");
                        wmbase.detach().pong(serial);
                    }
                );
                data.wmbase.replace(wmbase);
            }],
            [WlShm, 1, |shm: Main<WlShm>, data: &mut Data| {
                filter2!(shm, data,
                    wl_shm::Event::Format { format } => {
                        data.shm_formats.push(format);
                    }
                );
                data.shm.replace(shm);
            }]
        ),
    );

    let font = Font::new();
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
            let pixel_size = 4;
            let stride = width * pixel_size;
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
            font.glyphs("привет").render(|x, y, v| {
                if v > 0x22 {
                    unsafe {
                        *shmdata.offset((x + y * (width as usize)) as isize) =
                            u32::from_be_bytes([0xdd, v, v, v]);
                    }
                }
            });

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
            filter2!(toplevel, data,
                xdg_toplevel::Event::Close => data.should_close = true
            );
            xdg.set_window_geometry(0, 0, 100, 100);
            filter2!(xdg, data,
                xdg_surface::Event::Configure { serial } => {
                    println!("xdg_surface Configure");
                    let surface = data.surface.as_mut().expect("surface configured without surface?");
                    surface.xdg.detach().ack_configure(serial);
                    surface.state = SurfaceState::Configured;
                }
            );
            wl.commit();
            data.surface.replace(Surface {
                wl,
                xdg,
                toplevel,
                buffer_committed: false,
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
