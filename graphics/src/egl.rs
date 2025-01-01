use std::{ptr::null_mut, rc::Rc};

use anyhow::{Context as _, anyhow};

use crate::libegl;

pub fn unwrap_err(egl: &libegl::Lib) -> anyhow::Error {
    match unsafe { (egl.eglGetError)() } as libegl::EGLenum {
        libegl::EGL_SUCCESS => unreachable!(),
        code => anyhow!(format!("egl error 0x{:x}", code)),
    }
}

pub struct EglContext {
    egl: Rc<libegl::Lib>,

    pub config: libegl::EGLConfig,
    pub context: libegl::EGLContext,
    pub display: libegl::EGLDisplay,
}

impl EglContext {
    pub unsafe fn new(
        egl: &Rc<libegl::Lib>,
        display_id: libegl::EGLNativeDisplayType,
    ) -> anyhow::Result<Self> {
        // TODO: make api configurable
        if (egl.eglBindAPI)(libegl::EGL_OPENGL_API) == libegl::EGL_FALSE {
            return Err(unwrap_err(egl)).context("could not bind api");
        }

        let display = (egl.eglGetDisplay)(display_id);
        if display == libegl::EGL_NO_DISPLAY {
            return Err(unwrap_err(egl)).context("could not get display");
        }

        let (mut major, mut minor) = (0, 0);
        if (egl.eglInitialize)(display, &mut major, &mut minor) == libegl::EGL_FALSE {
            return Err(unwrap_err(egl)).context("could not initialize");
        }
        log::info!("initialized egl version {major}.{minor}");

        // TODO: make config attrs configurable
        let config_attrs = &[
            libegl::EGL_RED_SIZE,
            8,
            libegl::EGL_GREEN_SIZE,
            8,
            libegl::EGL_BLUE_SIZE,
            8,
            // NOTE: it is important to set EGL_ALPHA_SIZE, it enables transparency
            libegl::EGL_ALPHA_SIZE,
            8,
            libegl::EGL_CONFORMANT,
            libegl::EGL_OPENGL_ES3_BIT,
            libegl::EGL_RENDERABLE_TYPE,
            libegl::EGL_OPENGL_ES3_BIT,
            // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enables some kind of don't care anti aliasing
            libegl::EGL_SAMPLE_BUFFERS,
            1,
            libegl::EGL_SAMPLES,
            4,
            libegl::EGL_NONE,
        ];
        let mut num_configs = 0;
        if (egl.eglGetConfigs)(display, null_mut(), 0, &mut num_configs) == libegl::EGL_FALSE {
            return Err(unwrap_err(egl)).context("could not get num of available configs");
        }
        let mut configs = vec![std::mem::zeroed(); num_configs as usize];
        if (egl.eglChooseConfig)(
            display,
            config_attrs.as_ptr() as _,
            configs.as_mut_ptr(),
            num_configs,
            &mut num_configs,
        ) == libegl::EGL_FALSE
        {
            return Err(unwrap_err(egl)).context("could not choose config");
        }
        configs.set_len(num_configs as usize);
        if configs.is_empty() {
            return Err(anyhow!("could not choose config (/ no compatible ones)"));
        }
        let config = *configs.first().unwrap();

        let context_attrs = &[libegl::EGL_CONTEXT_MAJOR_VERSION, 3, libegl::EGL_NONE];
        let context = (egl.eglCreateContext)(
            display,
            config,
            libegl::EGL_NO_CONTEXT,
            context_attrs.as_ptr() as _,
        );
        if context == libegl::EGL_NO_CONTEXT {
            return Err(unwrap_err(egl)).context("could not create context");
        }

        Ok(EglContext {
            egl: Rc::clone(egl),

            display,
            config,
            context,
        })
    }

    pub unsafe fn make_current(&self, surface: libegl::EGLSurface) -> anyhow::Result<()> {
        if (self.egl.eglMakeCurrent)(self.display, surface, surface, self.context)
            == libegl::EGL_FALSE
        {
            Err(unwrap_err(&self.egl)).context("could not make current")
        } else {
            Ok(())
        }
    }

    pub unsafe fn make_current_surfaceless(&self) -> anyhow::Result<()> {
        self.make_current(libegl::EGL_NO_SURFACE)
    }

    pub unsafe fn swap_buffers(&self, surface: libegl::EGLSurface) -> anyhow::Result<()> {
        if (self.egl.eglSwapBuffers)(self.display, surface) == libegl::EGL_FALSE {
            Err(unwrap_err(&self.egl)).context("could not swap buffers")
        } else {
            Ok(())
        }
    }
}
