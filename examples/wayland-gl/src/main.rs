// NOTE: the flow is
//   - connect to wayland display
//   - connect to egl display
//   - choose egl config and create egl context
//   - load libgl
//   - register wayland globals (roundtrip)
//   - setup wayland surface (roundtrip)
//   - hook up wayland surface to egl
//   - ... happy triangle
//
// NOTE: i want to keep this simple (well.. wayland simple)
//   to be able to test and debug combo of wayland+egl+gl things.
//
//   be aware/remember that amd and nvidia gpus/drivders don't behave identicaly.
//
// NOTE: my laptop has integrated amd gpu and intel gpu, by default it selects amd gpu,
// to force it selecting nvidia i specified following env vars:
//   `__NV_PRIME_RENDER_OFFLOAD=1 __EGL_VENDOR_LIBRARY_NAME=nvidia`
//
// TODO: figure out how to list available gpus and specify preferences for which gpu to select
//   low power, most powerful ..
//   this should be possible with opengl right? in a simlar way it is possible with vulkan.

use std::{
    f32,
    ffi::{CStr, c_char, c_void},
    mem::offset_of,
    ptr::{null, null_mut},
    str,
    time::Instant,
};

use anyhow::{Context as _, anyhow};

const DEFAULT_WINDOW_SIZE: (u32, u32) = (800, 600);

const VSHADER_SOURCE: &CStr = c"
#version 440 core

layout (location = 0) in vec2 a_position;  
layout (location = 1) in vec3 a_color;

layout (location = 0) uniform mat2 u_projection;
layout (location = 1) uniform mat2 u_rotation;

out gl_PerVertex {
    vec4 gl_Position;
};
out vec3 color;

void main() {
    gl_Position = vec4(u_projection * (u_rotation * a_position), 0.0, 1.0);
    color = a_color / 255.0;
}
";

const FSHADER_SOURCE: &CStr = c"
#version 440 core

in vec3 color;

layout (location = 0) out vec4 o_color;

void main() {
    o_color = vec4(color, 1.0);
}
";

// TODO: make this into wayland context, don't pack in egl, gl stuff.
struct WaylandConnection {
    libwayland_client: wayland::ClientApi,
    libwayland_egl: wayland::EglApi,

    wl_display: *mut wayland::wl_display,

    // globals
    wl_compositor: *mut wayland::wl_compositor,
    xdg_wm_base: *mut wayland::xdg_wm_base,

    // window
    wl_surface: *mut wayland::wl_surface,
    xdg_surface: *mut wayland::xdg_surface,
    xdg_toplevel: *mut wayland::xdg_toplevel,
    did_ack_first_xdg_surface_configure: bool,
    close_requested: bool,
    width: u32,
    height: u32,
    wl_egl_window: *mut wayland::wl_egl_window,
}

struct EglConnection {
    libegl: egl::Api,

    egl_display: egl::EGLDisplay,
    egl_config: egl::EGLConfig,
    egl_context: egl::EGLContext,

    // connected to wl_egl_window.
    egl_window_surface: egl::EGLSurface,
}

unsafe extern "C" fn noop_listener() {}
const NOOP_LISTENER: unsafe extern "C" fn() = noop_listener;
macro_rules! noop_listener {
    () => {
        unsafe { std::mem::transmute(NOOP_LISTENER) }
    };
}

const WL_REGISTRY_LISTENER: wayland::wl_registry_listener = {
    unsafe extern "C" fn handle_global(
        data: *mut c_void,
        wl_registry: *mut wayland::wl_registry,
        name: u32,
        interface: *const c_char,
        version: u32,
    ) {
        let ctx = unsafe { &mut *(data as *mut WaylandConnection) };

        let interface = unsafe { CStr::from_ptr(interface) }
            .to_str()
            .expect("invalid interface string");

        println!("recv wl_registry_listener.global ({interface})");

        match interface {
            "wl_compositor" => unsafe {
                ctx.wl_compositor = wayland::wl_registry_bind(
                    &ctx.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wl_compositor_interface,
                    version,
                ) as _;
            },
            "xdg_wm_base" => unsafe {
                ctx.xdg_wm_base = wayland::wl_registry_bind(
                    &ctx.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::xdg_wm_base_interface,
                    version,
                ) as _;
            },
            _ => {}
        }
    }

    wayland::wl_registry_listener {
        global: handle_global,
        global_remove: noop_listener!(),
    }
};

const XDG_WM_BASE_LISTENER: wayland::xdg_wm_base_listener = {
    unsafe extern "C" fn handle_ping(
        data: *mut c_void,
        xdg_wm_base: *mut wayland::xdg_wm_base,
        serial: u32,
    ) {
        println!("recv xdg_wm_base_listener.ping");

        let ctx = unsafe { &mut *(data as *mut WaylandConnection) };
        unsafe { wayland::xdg_wm_base_pong(&ctx.libwayland_client, xdg_wm_base, serial) };
    }

    wayland::xdg_wm_base_listener { ping: handle_ping }
};

const XDG_SURFACE_LISTENER: wayland::xdg_surface_listener = {
    unsafe extern "C" fn handle_configure(
        data: *mut c_void,
        xdg_surface: *mut wayland::xdg_surface,
        serial: u32,
    ) {
        println!("recv xdg_surface_listener.configure");

        let ctx = unsafe { &mut *(data as *mut WaylandConnection) };
        unsafe { wayland::xdg_surface_ack_configure(&ctx.libwayland_client, xdg_surface, serial) };
        ctx.did_ack_first_xdg_surface_configure = true;
    }

    wayland::xdg_surface_listener {
        configure: handle_configure,
    }
};

const XDG_TOPLEVEL_LISTENER: wayland::xdg_toplevel_listener = {
    unsafe extern "C" fn handle_configure(
        data: *mut c_void,
        _xdg_toplevel: *mut wayland::xdg_toplevel,
        width: i32,
        height: i32,
        _states: *mut wayland::wl_array,
    ) {
        println!("recv xdg_toplevel_listener.configure");

        assert!(width >= 0 && height >= 0);

        let ctx = unsafe { &mut *(data as *mut WaylandConnection) };

        let did_resize = ctx.width != width as u32 || ctx.height != height as u32;
        if did_resize {
            ctx.width = width as u32;
            ctx.height = height as u32;

            // NOTE: egl surface is not created yet if first ack was not received.
            if ctx.did_ack_first_xdg_surface_configure {
                assert!(!ctx.wl_egl_window.is_null());
                unsafe {
                    (ctx.libwayland_egl.wl_egl_window_resize)(
                        ctx.wl_egl_window,
                        width,
                        height,
                        0,
                        0,
                    )
                };
            }
        }
    }

    unsafe extern "C" fn handle_close(
        data: *mut c_void,
        _xdg_toplevel: *mut wayland::xdg_toplevel,
    ) {
        println!("recv xdg_toplevel_listener.close");

        let ctx = unsafe { &mut *(data as *mut WaylandConnection) };
        ctx.close_requested = true;
    }

    wayland::xdg_toplevel_listener {
        configure: handle_configure,
        close: handle_close,
        wm_capabilities: noop_listener!(),
        configure_bounds: noop_listener!(),
    }
};

fn main() -> anyhow::Result<()> {
    let mut wlconn = {
        let libwayland_client =
            wayland::ClientApi::load().context("could not load libwayland-client")?;
        let libwayland_egl = wayland::EglApi::load().context("could not load libwayland-egl")?;

        let wl_display = {
            let start = Instant::now();

            let wl_display = unsafe { (libwayland_client.wl_display_connect)(null_mut()) };
            if wl_display.is_null() {
                return Err(anyhow!("could not connect to wayland display"));
            }

            println!("init wayland conn in {:?}", start.elapsed());

            wl_display
        };

        WaylandConnection {
            libwayland_client,
            libwayland_egl,

            wl_display,

            wl_compositor: null_mut(),
            xdg_wm_base: null_mut(),

            wl_surface: null_mut(),
            xdg_surface: null_mut(),
            xdg_toplevel: null_mut(),
            did_ack_first_xdg_surface_configure: false,
            close_requested: false,
            width: 0,
            height: 0,
            wl_egl_window: null_mut(),
        }
    };

    let mut eglconn = {
        let libegl = egl::Api::load().context("could not load libegl")?;

        let egl_display = {
            let start = Instant::now();

            let egl_display = unsafe {
                libegl.GetPlatformDisplay(
                    egl::PLATFORM_WAYLAND_KHR,
                    wlconn.wl_display.cast(),
                    null(),
                )
            };
            if egl_display == egl::NO_DISPLAY {
                return Err(anyhow!("could not get egl display"));
            }

            let (mut major, mut minor) = (0, 0);
            let ok = unsafe { libegl.Initialize(egl_display, &mut major, &mut minor) };
            if ok == egl::FALSE {
                let error = unsafe { libegl.GetError() };
                return Err(anyhow!("could not initialize egl: {error}"));
            }

            println!("init egl ({major}.{minor}) conn in {:?}", start.elapsed());

            egl_display
        };

        let ok = unsafe { libegl.BindAPI(egl::OPENGL_API) };
        if ok == egl::FALSE {
            let error = unsafe { libegl.GetError() };
            return Err(anyhow!("could not bind opengl api: {error}"));
        }

        let egl_config = {
            #[rustfmt::skip]
        let config_attrs: [egl::EGLint; _] = [
            egl::SURFACE_TYPE as _, egl::WINDOW_BIT,
            egl::CONFORMANT as _, egl::OPENGL_BIT,
            egl::RENDERABLE_TYPE as _, egl::OPENGL_BIT,
            egl::COLOR_BUFFER_TYPE as _, egl::RGB_BUFFER as _,

            egl::RED_SIZE as _, 8,
            egl::GREEN_SIZE as _, 8,
            egl::BLUE_SIZE as _, 8,

            // NOTE: EGL_ALPHA_SIZE enables surface transparency.
            egl::ALPHA_SIZE as _, 8,

            // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enable anti aliasing.
            // egl::SAMPLE_BUFFERS, 1,
            // egl::SAMPLES, 4,

            egl::NONE as _,
        ];

            let mut configs = [null_mut(); 64];
            let mut num_configs = 0;
            let ok = unsafe {
                libegl.ChooseConfig(
                    egl_display,
                    config_attrs.as_ptr(),
                    configs.as_mut_ptr(),
                    configs.len() as egl::EGLint,
                    &mut num_configs,
                )
            };
            if ok == egl::FALSE || num_configs == 0 {
                let error = unsafe { libegl.GetError() };
                return Err(anyhow!("could not choose egl config: {error}"));
            }

            println!("got {num_configs} egl configs matching specififed attrs");

            // TODO: is it guarnateed that this is the best one?
            //   do i need to try all the possible configs one by one until i succeeed at creating
            //   surface or all configs fail?
            let ret = configs[0];
            assert!(!ret.is_null());
            ret
        };

        let egl_context = {
            // NOTE: c is soooooo lax about types; but all of them really are just ints.
            let mut attrs = [egl::NONE as egl::EGLint; 16];
            let mut num_attrs = 0;
            let mut push_attr = |k: egl::EGLenum, v: egl::EGLint| {
                let idx = num_attrs * 2;
                attrs[idx] = k as egl::EGLint;
                attrs[idx + 1] = v;
                num_attrs += 1
            };
            push_attr(egl::CONTEXT_MAJOR_VERSION, 3);
            push_attr(egl::CONTEXT_MINOR_VERSION, 3);
            push_attr(
                egl::CONTEXT_OPENGL_PROFILE_MASK,
                egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            );
            // NOTE: don't enable debug in release builds.
            #[cfg(debug_assertions)]
            push_attr(egl::CONTEXT_OPENGL_DEBUG, egl::TRUE as egl::EGLint);

            let ret = unsafe {
                libegl.CreateContext(egl_display, egl_config, egl::NO_CONTEXT, attrs.as_ptr())
            };
            if ret == egl::NO_CONTEXT {
                let error = unsafe { libegl.GetError() };
                return Err(anyhow!("could not create egl context: {error}"));
            }
            ret
        };

        EglConnection {
            libegl,

            egl_display,
            egl_config,
            egl_context,

            egl_window_surface: null_mut(),
        }
    };

    let libgl = {
        // NOTE: need to make context current to be able to load gl functions.
        //   don't care about the surface yet.
        let ok = unsafe {
            eglconn.libegl.MakeCurrent(
                eglconn.egl_display,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                eglconn.egl_context,
            )
        };
        if ok == egl::FALSE {
            let error = unsafe { eglconn.libegl.GetError() };
            return Err(anyhow!("could not make egl context current: {error}"));
        }

        let libgl = unsafe { gl::Api::load_with(|name| eglconn.libegl.GetProcAddress(name) as _) };

        let gl_version = {
            let bytes = unsafe { libgl.GetString(gl::VERSION) };
            if bytes.is_null() {
                return Err(anyhow!("could not get gl version string"));
            }
            unsafe { CStr::from_ptr(bytes.cast()) }
                .to_str()
                .context("invalid gl version string")?
        };
        println!("initialized gl {gl_version}");

        libgl
    };

    // NOTE: get wayland globals first
    {
        let wl_registry: *mut wayland::wl_registry = unsafe {
            wayland::wl_display_get_registry(&wlconn.libwayland_client, wlconn.wl_display)
        };
        if wl_registry.is_null() {
            return Err(anyhow!("could not get registry"));
        }

        unsafe {
            (wlconn.libwayland_client.wl_proxy_add_listener)(
                wl_registry as *mut wayland::wl_proxy,
                &WL_REGISTRY_LISTENER as *const wayland::wl_registry_listener as _,
                &mut wlconn as *mut WaylandConnection as *mut c_void,
            );
        }
        // NOTE: roundtrip immediately to get globals.
        unsafe { (wlconn.libwayland_client.wl_display_roundtrip)(wlconn.wl_display) };

        // NOTE: can't really proceed without these, can we?
        if wlconn.wl_compositor.is_null() {
            return Err(anyhow!("wl_compositor is unavailable"));
        }
        if wlconn.xdg_wm_base.is_null() {
            return Err(anyhow!("xdg_wm_base is unavailable"));
        }

        println!("initialized globals");
    }

    // NOTE: a client must respond to a ping event with a pong request or the client may be deemed
    // unresponsive.
    unsafe {
        (wlconn.libwayland_client.wl_proxy_add_listener)(
            wlconn.xdg_wm_base as *mut wayland::wl_proxy,
            &XDG_WM_BASE_LISTENER as *const wayland::xdg_wm_base_listener as _,
            &mut wlconn as *mut WaylandConnection as *mut c_void,
        )
    };

    // NOTE: now can create wayland surface.
    {
        wlconn.wl_surface = unsafe {
            wayland::wl_compositor_create_surface(&wlconn.libwayland_client, wlconn.wl_compositor)
        };
        if wlconn.wl_surface.is_null() {
            return Err(anyhow!("could not create wl_surface"));
        }

        wlconn.xdg_surface = unsafe {
            wayland::xdg_wm_base_get_xdg_surface(
                &wlconn.libwayland_client,
                wlconn.xdg_wm_base,
                wlconn.wl_surface,
            )
        };
        if wlconn.xdg_surface.is_null() {
            return Err(anyhow!("could not create xdg_surface"));
        }
        unsafe {
            (wlconn.libwayland_client.wl_proxy_add_listener)(
                wlconn.xdg_surface as *mut wayland::wl_proxy,
                &XDG_SURFACE_LISTENER as *const wayland::xdg_surface_listener as _,
                &mut wlconn as *mut WaylandConnection as *mut c_void,
            )
        };

        wlconn.xdg_toplevel = unsafe {
            wayland::xdg_surface_get_toplevel(&wlconn.libwayland_client, wlconn.xdg_surface)
        };
        if wlconn.xdg_toplevel.is_null() {
            return Err(anyhow!("could not get xdg_toplevel"));
        }
        unsafe {
            (wlconn.libwayland_client.wl_proxy_add_listener)(
                wlconn.xdg_toplevel as *mut wayland::wl_proxy,
                &XDG_TOPLEVEL_LISTENER as *const wayland::xdg_toplevel_listener as _,
                &mut wlconn as *mut WaylandConnection as *mut c_void,
            )
        };

        let (width, height) = DEFAULT_WINDOW_SIZE;
        unsafe {
            wayland::xdg_toplevel_set_min_size(
                &wlconn.libwayland_client,
                wlconn.xdg_toplevel,
                width as i32,
                height as i32,
            )
        };

        // NOTE: xdg_toplevel_set_min_size is double-buffered.
        unsafe { wayland::wl_surface_commit(&wlconn.libwayland_client, wlconn.wl_surface) };

        // NOTE: roundtip to get window be acually be created.
        unsafe { (wlconn.libwayland_client.wl_display_roundtrip)(wlconn.wl_display) };
        assert!(wlconn.did_ack_first_xdg_surface_configure);

        println!("initialized window");
    }

    // NOTE: now can hook up wayland surface to egl
    {
        let (mut width, mut height) = (wlconn.width, wlconn.height);
        if width == 0 && height == 0 {
            (width, height) = DEFAULT_WINDOW_SIZE;
        }
        wlconn.wl_egl_window = unsafe {
            (wlconn.libwayland_egl.wl_egl_window_create)(
                wlconn.wl_surface.cast(),
                width as i32,
                height as i32,
            )
        };
        if wlconn.wl_egl_window.is_null() {
            // TODO: can there be some kind of error code somewhere?
            //   libc errno perhaps?
            return Err(anyhow!("could not create wl_egl_window"));
        }

        eglconn.egl_window_surface = unsafe {
            eglconn.libegl.CreatePlatformWindowSurface(
                eglconn.egl_display,
                eglconn.egl_config,
                wlconn.wl_egl_window.cast(),
                null(),
            )
        };
        if eglconn.egl_window_surface == egl::NO_SURFACE {
            let error = unsafe { eglconn.libegl.GetError() };
            return Err(anyhow!("could not create egl window surface: {error}"));
        }
    }

    // NOTE: now let's activate the surface
    let ok = unsafe {
        eglconn.libegl.MakeCurrent(
            eglconn.egl_display,
            eglconn.egl_window_surface,
            eglconn.egl_window_surface,
            eglconn.egl_context,
        )
    };
    if ok == egl::FALSE {
        let error = unsafe { eglconn.libegl.GetError() };
        return Err(anyhow!("could not make egl context current: {error}"));
    }

    // NOTE: now we can get to triangle ..
    let (vertices, vao, vshader, pipeline) = {
        // NOTE: we want stable layout
        #[repr(C)]
        struct Vertex {
            position: [f32; 2],
            color: [u8; 3],
        }

        let vertices = [
            Vertex {
                position: [0.0, 0.5],
                color: [255, 0, 0],
            },
            Vertex {
                position: [0.5, -0.5],
                color: [0, 255, 0],
            },
            Vertex {
                position: [-0.5, -0.5],
                color: [0, 0, 255],
            },
        ];

        let vbo: gl::GLuint = unsafe {
            let mut vbo = 0;
            libgl.CreateBuffers(1, &mut vbo);
            vbo
        };

        unsafe {
            libgl.NamedBufferData(
                vbo,
                (vertices.len() * size_of::<Vertex>()) as gl::GLsizeiptr,
                vertices.as_ptr().cast(),
                gl::STATIC_DRAW,
            )
        };

        let vao: gl::GLuint = unsafe {
            let mut vao = 0;
            libgl.CreateVertexArrays(1, &mut vao);
            vao
        };

        unsafe {
            let binding_idx = 0;
            libgl.VertexArrayVertexBuffer(
                vao,
                binding_idx,
                vbo,
                0,
                size_of::<Vertex>() as gl::GLsizei,
            );

            let a_position = 0;
            libgl.EnableVertexArrayAttrib(vao, a_position);
            libgl.VertexArrayAttribFormat(
                vao,
                a_position,
                3,
                gl::FLOAT,
                gl::FALSE,
                offset_of!(Vertex, position) as gl::GLuint,
            );
            libgl.VertexArrayAttribBinding(vao, a_position, binding_idx);

            let a_color = 1;
            libgl.EnableVertexArrayAttrib(vao, a_color);
            libgl.VertexArrayAttribFormat(
                vao,
                a_color,
                3,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                offset_of!(Vertex, color) as gl::GLuint,
            );
            libgl.VertexArrayAttribBinding(vao, a_color, binding_idx);
        }

        let mut temp_buf = Vec::<u8>::new();

        unsafe fn get_program_info_log_in<'a>(
            libgl: &gl::Api,
            program: gl::GLuint,
            buf: &'a mut Vec<u8>,
        ) -> Result<&'a str, str::Utf8Error> {
            let mut len: gl::GLint = 0;
            unsafe { libgl.GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len) };

            buf.clear();
            buf.reserve(len as usize);

            unsafe {
                libgl.GetProgramInfoLog(program, len, &mut len, buf.as_mut_ptr().cast());
                buf.set_len(len as usize);
            }

            str::from_utf8(buf)
        }

        let vshader = unsafe {
            let vshader = libgl.CreateShaderProgramv(
                gl::VERTEX_SHADER,
                1,
                [VSHADER_SOURCE.as_ptr().cast()].as_ptr(),
            );

            let mut linked: gl::GLint = 0;
            libgl.GetProgramiv(vshader, gl::LINK_STATUS, &mut linked);
            if linked == gl::FALSE as _ {
                let info_log = get_program_info_log_in(&libgl, vshader, &mut temp_buf)?;
                return Err(anyhow!("could not create vertex shader: {info_log}"));
            }

            vshader
        };

        let fshader = unsafe {
            let fshader = libgl.CreateShaderProgramv(
                gl::FRAGMENT_SHADER,
                1,
                [FSHADER_SOURCE.as_ptr().cast()].as_ptr(),
            );

            let mut linked: gl::GLint = 0;
            libgl.GetProgramiv(fshader, gl::LINK_STATUS, &mut linked);
            if linked == gl::FALSE as _ {
                let info_log = get_program_info_log_in(&libgl, fshader, &mut temp_buf)?;
                return Err(anyhow!("could not create fragment shader: {info_log}"));
            }

            fshader
        };

        let pipeline: gl::GLuint = unsafe {
            let mut pipeline = 0;
            libgl.GenProgramPipelines(1, &mut pipeline);
            pipeline
        };

        unsafe {
            libgl.UseProgramStages(pipeline, gl::VERTEX_SHADER_BIT, vshader);
            libgl.UseProgramStages(pipeline, gl::FRAGMENT_SHADER_BIT, fshader);
        }

        (vertices, vao, vshader, pipeline)
    };

    let mut prev_time = Instant::now();

    let mut angle: f32 = 0.0;
    const FULL_ROTATION_SECS: f32 = 16.0;

    while !wlconn.close_requested {
        let ok =
            unsafe { (wlconn.libwayland_client.wl_display_dispatch_pending)(wlconn.wl_display) };
        if ok == -1 {
            return Err(anyhow!("wl_display_dispatch failed"));
        }

        let next_time = Instant::now();
        let dt = (next_time - prev_time).as_secs_f32();
        prev_time = next_time;

        let aspect_ratio = wlconn.height as f32 / wlconn.width as f32;
        let projection_matrix = [[aspect_ratio, 0.0], [0.0, 1.0]];

        angle += 2.0 * f32::consts::PI * dt / FULL_ROTATION_SECS;
        let (sin, cos) = angle.sin_cos();
        let rotation_matrix = [[cos, -sin], [sin, cos]];

        unsafe {
            // NOTE: this needs to be specififed.
            //   without i don't see anything being rendered on nvidia gpu,
            //   but on amd gpu it's fine.
            libgl.DrawBuffer(gl::BACK);

            libgl.Viewport(
                0,
                0,
                wlconn.width as gl::GLsizei,
                wlconn.height as gl::GLsizei,
            );

            libgl.Clear(gl::COLOR_BUFFER_BIT);
            libgl.ClearColor(0.0, 0.0, 0.4, 1.0);

            {
                let u_projection = 0;
                libgl.ProgramUniformMatrix2fv(
                    vshader,
                    u_projection,
                    1,
                    gl::FALSE,
                    projection_matrix.as_ptr().cast(),
                );

                let u_rotation = 1;
                libgl.ProgramUniformMatrix2fv(
                    vshader,
                    u_rotation,
                    1,
                    gl::FALSE,
                    rotation_matrix.as_ptr().cast(),
                );
            }

            libgl.BindProgramPipeline(pipeline);
            libgl.BindVertexArray(vao);
            libgl.DrawArrays(gl::TRIANGLES, 0, vertices.len() as gl::GLsizei);
        };

        let ok = unsafe {
            eglconn
                .libegl
                .SwapBuffers(eglconn.egl_display, eglconn.egl_window_surface)
        };
        if ok == egl::FALSE {
            let error = unsafe { eglconn.libegl.GetError() };
            return Err(anyhow!("could not swap buffers: {error}"));
        }
    }

    Ok(())
}
