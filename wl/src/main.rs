use rusttype::{self, point, Scale};
use std::cmp::max;
use wayland_client::protocol::{
    wl_buffer::{self, WlBuffer},
    wl_compositor::{self, WlCompositor},
    wl_pointer,
    wl_seat::{self, WlSeat},
    wl_shm::{self, WlShm},
    wl_surface::{self, WlSurface},
};
use wayland_client::EventQueue;
use wayland_client::{global_filter, Display, Filter, GlobalManager, Main};
use wayland_protocols::xdg_shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

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
                if let Some(rb) = ddata.get::<RegistryBuilder>() {
                    $cb(x, rb)
                } else {
                    eprintln!("building registry is over")
                }
            }]),*
        )
    };
}

#[derive(Debug, Default)]
struct Config {
    font: Font,
    options: Vec<String>,
    button_dim: (usize, usize),
    border: usize,
    window_dim: (usize, usize),
    should_close: bool,
}

impl Config {
    fn new(options: Vec<String>, button_dim: (usize, usize), border: usize) -> Config {
        let window_dim = (
            border + options.len() * (button_dim.0 + border),
            border + button_dim.1 + border,
        );
        Config {
            font: Font::new(),
            options,
            button_dim,
            border,
            window_dim,
            should_close: false,
        }
    }
}

#[derive(Debug, Default)]
struct RegistryBuilder {
    compositor: Option<Main<WlCompositor>>,
    seat: Option<Main<WlSeat>>,
    shm: Option<Main<WlShm>>,
    wmbase: Option<Main<XdgWmBase>>,
}

#[derive(Debug)]
struct Registry {
    compositor: Main<WlCompositor>,
    seat: Main<WlSeat>,
    shm: Main<WlShm>,
    wmbase: Main<XdgWmBase>,
}

impl RegistryBuilder {
    fn ready(&self) -> bool {
        self.compositor.is_some()
            && self.seat.is_some()
            && self.shm.is_some()
            && self.wmbase.is_some()
    }

    fn build(self) -> Option<Registry> {
        Some(Registry {
            compositor: self.compositor?,
            seat: self.seat?,
            shm: self.shm?,
            wmbase: self.wmbase?,
        })
    }
}

#[derive(Debug, Default)]
struct Pointer {
    pos: Option<(f64, f64)>,
    pos_prev: Option<(f64, f64)>,
    btn: Option<wl_pointer::ButtonState>,
    btn_prev: Option<wl_pointer::ButtonState>,
    frame: bool,
}

#[derive(Debug, PartialEq)]
struct Surface {
    wl: Main<WlSurface>,
    xdg: Main<XdgSurface>,
    toplevel: Main<XdgToplevel>,
    buffer_committed: bool,
    configured: bool,
}

#[derive(Debug)]
struct Data {
    cfg: Config,
    registry: Registry,
    ptr: Pointer,
    seat_cap: Option<wl_seat::Capability>,
    shm_formats: Vec<wl_shm::Format>,
    buffer: Option<ShmPixelBuffer>,
    surface: Option<Surface>,
}

impl Data {
    fn new(cfg: Config, mut registry: Registry) -> Data {
        let seat = &mut registry.seat;
        filter2!(seat, data,
            wl_seat::Event::Capabilities{capabilities} => {
                eprintln!("seat caps {:?}", capabilities);
                data.seat_cap.replace(capabilities);
            }
        );
        let pointer = seat.get_pointer();
        filter2!(pointer, data,
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                data.ptr.pos.replace((surface_x, surface_y));
            },
            wl_pointer::Event::Leave { .. } => {
                data.ptr.pos.take();
            },
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                data.ptr.pos.replace((surface_x, surface_y));
            },
            wl_pointer::Event::Button { button: 0x110, state, .. } => {
                // 0x110 is BUTTON1
                data.ptr.btn.replace(state);
            },
            wl_pointer::Event::Frame => {
                data.ptr.frame = true;
            }
        );

        let wmbase = &mut registry.wmbase;
        filter2!(wmbase, data,
            xdg_wm_base::Event::Ping { serial } => {
                data.registry.wmbase.detach().pong(serial);
            }
        );

        let shm = &mut registry.shm;
        filter2!(shm, data,
            wl_shm::Event::Format { format } => {
                data.shm_formats.push(format);
            }
        );

        Data {
            cfg,
            registry,
            ptr: Pointer::default(),
            buffer: None,
            surface: None,
            seat_cap: None,
            shm_formats: vec![],
        }
    }

    fn render(&mut self) {
        if self.buffer.is_none() {
            return;
        }
        let shm = &mut self.buffer.as_mut().unwrap();
        let Config {
            border,
            button_dim: (bw, bh),
            ..
        } = self.cfg;
        shm.clear(0x22222222);
        for i in 0..shm.width {
            for j in 0..shm.height {
                if i > border
                    && ((i - border) % (bw + border)) < bw
                    && j > border
                    && j < border + bh
                {
                    // inside button
                    shm[(i, j)] = 0xdd222222;
                }
            }
        }

        for i in 0..self.cfg.options.len() {
            let g = self.cfg.font.glyphs(self.cfg.options.get(i).unwrap());

            let left = border + i * (bw + border);
            let right = left + bw;
            let top = border;
            let bottom = top + bh;

            let offset_x : i32 = left as i32 - g.width.ceil() as i32 / 2 + bw as i32 / 2;
            let offset_y : i32 = top as i32 - g.height.ceil() as i32 / 2 + bh as i32 / 2;

            let (mut btn_boundaries, mut buf_boundaries) = (false, false);
            g.render(|x, y, v| {
                let (x, y) = (x as i32 + offset_x, y as i32 + offset_y);
                if x < 0 || x as usize >= shm.width || y < 0 || y as usize >= shm.height {
                    if !buf_boundaries {
                        eprintln!(
                            "glyph exceeds buffer boundaries: {:?} {:?}",
                            (x, y),
                            (shm.width, shm.height)
                        );
                        buf_boundaries = true;
                    }
                    return;
                }
                let (x, y) = (x as usize, y as usize);
                if x < left || x >= right || y < top || y >= bottom {
                    if !btn_boundaries {
                        eprintln!(
                            "glyph exceeds button boundaries: {:?} {:?}",
                            (x, y),
                            (left, right, top, bottom)
                        );
                        btn_boundaries = true;
                    }
                    return;
                }

                if 0x22 < v {
                    let [a, r, g, b] = shm[(x, y)].to_be_bytes();
                    shm[(x, y)] = u32::from_be_bytes([a, max(r, v), max(g, v), max(b, v)]);
                }
            });
        }
    }
}

#[derive(Debug)]
struct Font {
    font: rusttype::Font<'static>,
    scale: Scale,
    offset: rusttype::Point<f32>,
}

#[derive(Debug)]
struct Glyphs<'f> {
    glyphs: Vec<rusttype::PositionedGlyph<'f>>,
    width: f32,
    height: f32,
}

impl Default for Font {
    fn default() -> Self {
        Font::new()
    }
}

impl Font {
    fn new() -> Font {
        let font =
            rusttype::Font::try_from_bytes(include_bytes!("../SourceCodePro-Regular.otf") as &[u8])
                .expect("error constructing a Font from bytes");
        let scale = Scale::uniform(40.0);
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
    fn render(self, mut d: impl FnMut(usize, usize, u8)) {
        let width = self.width.ceil();
        let height = self.height.ceil();

        for g in self.glyphs {
            let bb = g.pixel_bounding_box();
            if bb.is_none() {
                continue;
            }
            let bb = bb.unwrap();

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

#[derive(Debug)]
struct ShmPixelBuffer {
    wl: Main<WlBuffer>,
    locked: bool,
    addr: *mut u32,
    width: usize,
    height: usize,
}

impl std::ops::Index<(usize, usize)> for ShmPixelBuffer {
    type Output = u32;
    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        if x >= self.width || y >= self.height {
            panic!(
                "index ({}, {}) out of bounds (0..{}, 0..{})",
                x, y, self.width, self.height
            );
        }
        unsafe {
            self.addr
                .offset((x + y * self.width) as isize)
                .as_ref()
                .unwrap()
        }
    }
}

impl std::ops::IndexMut<(usize, usize)> for ShmPixelBuffer {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        if x >= self.width || y >= self.height {
            panic!(
                "index ({}, {}) out of bounds (0..{}, 0..{})",
                x, y, self.width, self.height
            );
        }
        unsafe {
            self.addr
                .offset((x + y * self.width) as isize)
                .as_mut()
                .unwrap()
        }
    }
}

impl ShmPixelBuffer {
    fn clear(&mut self, c: u32) {
        for y in 0..self.height {
            for x in 0..self.width {
                self[(x, y)] = c;
            }
        }
    }
}

fn init_registry(display: &Display, event_queue: &mut EventQueue) -> Option<Registry> {
    let disp_proxy = display.attach(event_queue.token());

    let _globals = GlobalManager::new_with_cb(
        &disp_proxy,
        global_filter!(
            [WlCompositor, 4, |compositor, rb: &mut RegistryBuilder| {
                rb.compositor.replace(compositor);
            }],
            [WlSeat, 4, |seat, rb: &mut RegistryBuilder| {
                rb.seat.replace(seat);
            }],
            [XdgWmBase, 2, |wmbase, rb: &mut RegistryBuilder| {
                rb.wmbase.replace(wmbase);
            }],
            [WlShm, 1, |shm, rb: &mut RegistryBuilder| {
                rb.shm.replace(shm);
            }]
        ),
    );

    let mut registry = RegistryBuilder::default();
    while !registry.ready() {
        event_queue
            .dispatch(&mut registry, |_ev, _proxy, _ddata| {})
            .expect("An error occured during event dispatch");
    }

    registry.build()
}

fn main() {
    // todo: read options in from stdin
    // todo: read config args in from process args
    let cfg = Config::new(
        vec!["shutdown".into(), "restart".into(), "hibernate".into()],
        (300, 300),
        30,
    );

    let display = Display::connect_to_env().expect("failed to connect to display");
    let mut event_queue = display.create_event_queue();

    let registry = init_registry(&display, &mut event_queue).expect("wtf registry");
    let mut data = Data::new(cfg, registry);

    while !data.cfg.should_close {
        event_queue
            .dispatch(&mut data, |ev, _proxy, _ddata| {
                if true || ev.interface != "wl_registry" {
                    eprintln!("{:?}", ev);
                }
            })
            .expect("An error occurred during event dispatch");

        if data.buffer.is_none() {
            let fd = nix::unistd::mkstemp("/dev/shm/shmbuf_XXXXXX")
                .and_then(|(fd, path)| nix::unistd::unlink(path.as_path()).and(Ok(fd)))
                .expect("failed to create temp file fd for shm");
            let pixel_size = 4;
            let (width, height) = data.cfg.window_dim;
            let stride = data.cfg.window_dim.0 * pixel_size;
            let size: usize = stride * height;

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

            let pool = data.registry.shm.create_pool(fd, size as i32);
            let buffer = pool.create_buffer(
                0,
                width as i32,
                height as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
            );
            pool.destroy();

            filter2!(buffer, data,
                wl_buffer::Event::Release => {
                    if let Some(buffer) = &mut data.buffer {
                        buffer.locked = false;
                    }
                }
            );

            eprintln!("created mmap'd shm buffer ({:?})", shmdata);
            let shmbuffer = ShmPixelBuffer {
                wl: buffer,
                locked: false,
                addr: shmdata,
                width: width as usize,
                height: height as usize,
            };
            data.buffer.replace(shmbuffer);
            data.render();
        }

        if data.surface.is_none() {
            eprintln!("setting xdg_surface");
            let wl = data.registry.compositor.create_surface();
            let xdg = data.registry.wmbase.get_xdg_surface(&wl.detach());
            let toplevel = xdg.get_toplevel();
            let appid = String::from("wlr-shlayer");
            toplevel.set_title(appid.clone());
            toplevel.set_app_id(appid);
            filter2!(toplevel, data,
                xdg_toplevel::Event::Close => data.cfg.should_close = true
            );
            xdg.set_window_geometry(
                0,
                0,
                data.cfg.window_dim.0 as i32,
                data.cfg.window_dim.1 as i32,
            );
            filter2!(xdg, data,
                xdg_surface::Event::Configure { serial } => {
                    eprintln!("xdg_surface Configure");
                    let surface = data.surface.as_mut().expect("surface configured without surface?");
                    surface.xdg.detach().ack_configure(serial);
                    surface.configured = true;
                }
            );
            wl.commit();
            data.surface.replace(Surface {
                wl,
                xdg,
                toplevel,
                buffer_committed: false,
                configured: false,
            });
        }

        if let (true, Some(_), Some(ShmPixelBuffer { locked: false, .. })) =
            (data.ptr.frame, &data.surface, &data.buffer)
        {
            if data.ptr.btn != data.ptr.btn_prev {
                data.ptr.btn_prev = data.ptr.btn;
                data.render();

                if let (Some(wl_pointer::ButtonState::Pressed), Some((x, y))) = (data.ptr.btn, data.ptr.pos) {
                    let (x, y) = (x.ceil() as usize, y.ceil() as usize);
                    let (border, bw, bh) = (data.cfg.border, data.cfg.button_dim.0, data.cfg.button_dim.1);
                    if y >= border && y < border + bh && x >= border && (x - border) % (border + bw) < bw {
                        let i = (x - border) / (border + bw);
                        if let Some(opt) = data.cfg.options.get(i) {
                            println!("{}", opt);
                            data.cfg.should_close = true;
                        }
                    }
                }

                if let Some(surface) = &mut data.surface {
                    let (width, height) = data.cfg.window_dim;
                    eprintln!("Damage 0..{} 0..{}", width, height);
                    surface.wl.damage(0, 0, width as i32, height as i32);
                    surface.buffer_committed = false;
                }
            }
        }

        if let (
            Some(
                surface
                @
                Surface {
                    configured: true,
                    buffer_committed: false,
                    ..
                },
            ),
            Some(buffer),
        ) = (&mut data.surface, &mut data.buffer)
        {
            eprintln!("buffer commit");
            surface.wl.attach(Some(&buffer.wl), 0, 0);
            buffer.locked = true;
            surface.wl.commit();
            surface.buffer_committed = true;
        }
    }
}
