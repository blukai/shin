use std::time::{Duration, Instant};

use crate::{DrawBuffer, Externs, FontHandle, FontService, InteractionState, TextureService};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

pub struct Context<E: Externs> {
    pub font_service: FontService,
    pub texture_service: TextureService<E>,
    pub draw_buffer: DrawBuffer<E>,
    pub interaction_state: InteractionState,

    pub default_font_handle: FontHandle,
    pub default_font_size: f32,

    previous_frame_start: Instant,
    current_frame_start: Instant,
    delta_time: Duration,
}

impl<E: Externs> Context<E> {
    pub fn with_default_font_slice(
        font_data: &'static [u8],
        default_font_size: f32,
    ) -> anyhow::Result<Self> {
        let mut font_service = FontService::default();
        let default_font_handle = font_service.register_font_slice(font_data)?;

        Ok(Self {
            texture_service: TextureService::default(),
            font_service,
            draw_buffer: DrawBuffer::default(),
            interaction_state: InteractionState::default(),

            default_font_handle,
            default_font_size,

            previous_frame_start: Instant::now(),
            current_frame_start: Instant::now(),
            delta_time: Duration::ZERO,
        })
    }
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self::with_default_font_slice(DEFAULT_FONT_DATA, 16.0)
            .expect("somebody fucked things up; default font is invalid?")
    }
}

impl<E: Externs> Context<E> {
    pub fn begin_frame(&mut self) {
        self.interaction_state.begin_frame();

        self.current_frame_start = Instant::now();
        self.delta_time = self.current_frame_start - self.previous_frame_start;
        self.previous_frame_start = self.current_frame_start;
    }

    pub fn end_frame(&mut self) {
        self.draw_buffer.clear();
        self.interaction_state.end_frame();
    }

    pub fn dt(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }
}
