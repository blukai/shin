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
