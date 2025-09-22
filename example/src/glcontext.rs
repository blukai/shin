use std::mem;
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
        let egl_config = {
            use egl::*;

            #[rustfmt::skip]
            let config_attrs = [
                RED_SIZE, 8,
                GREEN_SIZE, 8,
                BLUE_SIZE, 8,
                // NOTE: EGL_ALPHA_SIZE enables surface transparency.
                ALPHA_SIZE, 8,
                CONFORMANT, OPENGL_BIT as _,
                RENDERABLE_TYPE, OPENGL_BIT as _,
                // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enable some kind of don't care anti aliasing.
                SAMPLE_BUFFERS, 1,
                SAMPLES, 4,
                NONE,
            ];
            // TODO: might need/want MIN_SWAP_INTERVAL and MAX_SWAP_INTERVAL these to disable vsync?
            let mut num_configs = 0;
            if unsafe {
                egl_connection.api.GetConfigs(
                    *egl_connection.display,
                    null_mut(),
                    0,
                    &mut num_configs,
                )
            } == FALSE
            {
                return Err(egl_connection.unwrap_err()).context("could not get num configs");
            }

            let mut configs = vec![unsafe { mem::zeroed() }; num_configs as usize];
            if unsafe {
                egl_connection.api.ChooseConfig(
                    *egl_connection.display,
                    config_attrs.as_ptr() as _,
                    configs.as_mut_ptr(),
                    num_configs,
                    &mut num_configs,
                )
            } == FALSE
            {
                return Err(egl_connection.unwrap_err()).context("could not choose config");
            }
            unsafe { configs.set_len(num_configs as usize) };
            configs
                .first()
                .copied()
                .context("could not choose config (no compatible ones probably)")?
        };

        let egl_context = egl_connection.create_context(
            egl::OPENGL_API,
            egl_config,
            None,
            Some(&[
                egl::CONTEXT_MAJOR_VERSION as egl::EGLint,
                3,
                egl::NONE as egl::EGLint,
            ]),
        )?;

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
        // context.set_swap_interval(&egl, 0)?;

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
    pub api: gl::Api,
}

impl GlContext {
    #[cfg(unix)]
    pub fn from_wayland_display(wl_display: *mut c_void) -> anyhow::Result<Self> {
        let ctx = GlContextEgl::from_wayland_display(wl_display)?;
        let api = unsafe {
            gl::Api::load_with(|procname| {
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
        let api =
            gl::Api::from_canvas_selector(canvas_selector).context("could not load gl api")?;
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
