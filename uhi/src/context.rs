use crate::{DrawBuffer, Externs, FontService, InteractionState, TextureService};

pub struct Context<E: Externs> {
    pub font_service: FontService,
    pub texture_service: TextureService<E>,
    pub draw_buffer: DrawBuffer<E>,
    pub interaction_state: InteractionState,
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self {
            texture_service: TextureService::default(),
            font_service: FontService::default(),
            draw_buffer: DrawBuffer::default(),
            interaction_state: InteractionState::default(),
        }
    }
}

impl<E: Externs> Context<E> {
    pub fn begin_frame(&mut self) {
        self.interaction_state.begin_frame();
    }

    pub fn end_frame(&mut self) {
        self.draw_buffer.clear();
        self.interaction_state.end_frame();
    }
}
