use std::time::{Duration, Instant};

use crate::{DrawBuffer, Externs, FontService, InteractionState, TextureService};

pub struct Context<E: Externs> {
    pub font_service: FontService,
    pub texture_service: TextureService<E>,
    pub draw_buffer: DrawBuffer<E>,
    pub interaction_state: InteractionState,

    previous_frame_start: Instant,
    current_frame_start: Instant,
    delta_time: Duration,
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self {
            texture_service: TextureService::default(),
            font_service: FontService::default(),
            draw_buffer: DrawBuffer::default(),
            interaction_state: InteractionState::default(),

            previous_frame_start: Instant::now(),
            current_frame_start: Instant::now(),
            delta_time: Duration::ZERO,
        }
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
