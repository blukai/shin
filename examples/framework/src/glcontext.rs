use std::ptr::null_mut;
use std::{array, ffi::c_void};

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;

// TODO: experiment with multiple surfaces in multiple threads and vsync?

// TODO: should i bring wayland dependency in here? if you do - don't forget to remove .cast()s.

#[cfg(unix)]
struct GlContextEgl {
    egl_connection: egl::wrap::Connection,
    egl_context: egl::wrap::Context,
    // NOTE: would you want more then 16? 16 is prob too excessive?
    egl_window_surfaces: [Option<(*mut c_void, egl::wrap::WindowSurface)>; 16],
}

#[cfg(unix)]
impl GlContextEgl {
    fn from_egl_connection(mut egl_connection: egl::wrap::Connection) -> anyhow::Result<Self> {
        // NOTE: c is soooooo lax about types; but all of them really are just ints.

        let egl_config = {
            #[rustfmt::skip]
            let config_attrs: [egl::EGLint; _] = [
                egl::SURFACE_TYPE  as _, egl::WINDOW_BIT,
                egl::CONFORMANT as _, egl::OPENGL_BIT,
                egl::RENDERABLE_TYPE as _, egl::OPENGL_BIT,
                egl::COLOR_BUFFER_TYPE as _, egl::RGB_BUFFER as _,

                egl::RED_SIZE as _, 8,
                egl::GREEN_SIZE as _, 8,
                egl::BLUE_SIZE as _, 8,
                // NOTE: EGL_ALPHA_SIZE enables surface transparency.
                egl::ALPHA_SIZE as _, 8,

                // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enable some kind of don't care anti aliasing.
                egl::SAMPLE_BUFFERS as _, 1,
                egl::SAMPLES as _, 4,

                egl::NONE as _,
            ];

            // NOTE: 64 is enough configs, isn't it?
            let mut configs = [null_mut(); 64];
            let mut num_configs = 0;
            let ok = unsafe {
                egl_connection.api.ChooseConfig(
                    *egl_connection.display,
                    config_attrs.as_ptr(),
                    configs.as_mut_ptr(),
                    configs.len() as egl::EGLint,
                    &mut num_configs,
                )
            };
            if ok == egl::FALSE || num_configs == 0 {
                return Err(egl_connection.unwrap_err()).context("could not choose config");
            }

            let ret = configs[0];
            assert!(!ret.is_null());
            ret
        };

        let egl_context = {
            #[rustfmt::skip]
            let context_attrs = [
                egl::CONTEXT_MAJOR_VERSION as _, 3,
                // TODO: can't get gl 3.3 working AND core profile, figure out why.
                // CONTEXT_MINOR_VERSION as _, 3,
                // CONTEXT_OPENGL_PROFILE_MASK as _, CONTEXT_OPENGL_CORE_PROFILE_BIT,
                egl::NONE as _,
            ];
            egl_connection.create_context(
                egl::OPENGL_API,
                egl_config,
                None,
                Some(&context_attrs),
            )?
        };

        if unsafe {
            egl_connection.api.MakeCurrent(
                *egl_connection.display,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                egl_context.context,
            )
        } == egl::FALSE
        {
            return Err(egl_connection.unwrap_err()).context("could not make current");
        }

        // TODO: figure out an okay way to include vsync toggle.
        // unsafe { egl_connection.api.SwapInterval(*egl_connection.display, 0) };

        Ok(Self {
            egl_connection,
            egl_context,
            egl_window_surfaces: array::from_fn(|_| None),
        })
    }

    fn from_wayland_display(wl_display: *mut c_void) -> anyhow::Result<Self> {
        let egl_connection = egl::wrap::Connection::from_wayland_display(wl_display.cast(), None)
            .context("could not create egl connection")?;
        Self::from_egl_connection(egl_connection)
    }

    fn find_or_create_wayland_window_surface(
        &mut self,
        wl_surface: *mut c_void,
        width: u32,
        height: u32,
    ) -> anyhow::Result<usize> {
        // NOTE: have to find index, and not the thing itself because of borrow checker's
        // stupidity; see note on @BorrowDoesNotContinue.
        for (i, it) in self.egl_window_surfaces.iter().enumerate() {
            if let Some(it) = it {
                if it.0 == wl_surface {
                    return Ok(i);
                }
            }
        }

        let egl_window_surface = self.egl_connection.create_wayland_window_surface(
            self.egl_context.config,
            wl_surface.cast(),
            width,
            height,
            None,
        )?;
        let index = self
            .egl_window_surfaces
            .iter()
            .position(|it| it.is_none())
            .expect("exhausted egl window surfaces capacity");
        self.egl_window_surfaces[index] = Some((wl_surface, egl_window_surface));
        Ok(index)
    }
}

enum GlContextKind {
    #[cfg(unix)]
    Egl(GlContextEgl),
    #[cfg(target_family = "wasm")]
    Web,
}

pub struct GlContext {
    kind: GlContextKind,
    pub api: gl::wrap::Api,
}

impl GlContext {
    #[cfg(unix)]
    pub fn from_wayland_display(wl_display: *mut c_void) -> anyhow::Result<Self> {
        let ctx = GlContextEgl::from_wayland_display(wl_display)?;
        let api = unsafe {
            gl::wrap::Api::load_with(|procname| {
                ctx.egl_connection.api.GetProcAddress(procname) as *mut c_void
            })
        };
        Ok(Self {
            kind: GlContextKind::Egl(ctx),
            api,
        })
    }

    #[cfg(target_family = "wasm")]
    pub fn from_canvas_selector(canvas_selector: &str) -> anyhow::Result<Self> {
        let api = gl::wrap::Api::from_canvas_selector(canvas_selector)
            .context("could not load gl api")?;
        Ok(Self {
            kind: GlContextKind::Web,
            api,
        })
    }

    pub fn make_window_current(
        &mut self,
        raw_window_handle: rwh::RawWindowHandle,
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        match (&mut self.kind, raw_window_handle) {
            #[cfg(unix)]
            (
                GlContextKind::Egl(ctx),
                rwh::RawWindowHandle::Wayland(rwh::WaylandWindowHandle {
                    surface: wl_surface,
                    ..
                }),
            ) => {
                let idx =
                    ctx.find_or_create_wayland_window_surface(wl_surface.as_ptr(), width, height)?;
                let Some((_, ref mut egl_window_surface)) = ctx.egl_window_surfaces[idx] else {
                    unreachable!();
                };
                match egl_window_surface.wsi {
                    egl::wrap::Wsi::Wayland(ref mut wayland) => {
                        if wayland.size() != (width, height) {
                            wayland.resize(width, height);
                        }
                    }
                }
                let ok = unsafe {
                    ctx.egl_connection.api.MakeCurrent(
                        *ctx.egl_connection.display,
                        egl_window_surface.surface,
                        egl_window_surface.surface,
                        ctx.egl_context.context,
                    )
                };
                if ok == egl::FALSE {
                    Err(ctx.egl_connection.unwrap_err().into())
                } else {
                    Ok(())
                }
            }

            #[cfg(target_family = "wasm")]
            (GlContextKind::Web, rwh::RawWindowHandle::Web(_)) => Ok(()),

            _ => Err(anyhow!("unsupported window: {raw_window_handle:?}")),
        }
    }

    pub fn swap_window_buffers(
        &self,
        raw_window_handle: rwh::RawWindowHandle,
    ) -> anyhow::Result<()> {
        match (&self.kind, raw_window_handle) {
            #[cfg(unix)]
            (
                GlContextKind::Egl(ctx),
                rwh::RawWindowHandle::Wayland(rwh::WaylandWindowHandle {
                    surface: wl_surface,
                    ..
                }),
            ) => {
                let Some(Some((_, egl_window_surface))) = ctx.egl_window_surfaces.iter().find(
                    |it| matches!(it, Some((it_wl_surface, _)) if *it_wl_surface == wl_surface.as_ptr()),
                ) else {
                    return Err(anyhow!("unknown surface"));
                };
                let ok = unsafe {
                    ctx.egl_connection
                        .api
                        .SwapBuffers(*ctx.egl_connection.display, egl_window_surface.surface)
                };
                if ok == egl::FALSE {
                    Err(ctx.egl_connection.unwrap_err().into())
                } else {
                    Ok(())
                }
            }

            #[cfg(target_family = "wasm")]
            (GlContextKind::Web, rwh::RawWindowHandle::Web(_)) => Ok(()),

            _ => Err(anyhow!("unsupported window: {raw_window_handle:?}")),
        }
    }
}
