use crate::{FontTextureCache, Rect, Renderer, TextureHandle, TextureService};

use anyhow::anyhow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    index: usize,
}

struct Font {
    fontdue_font: fontdue::Font,
    size: f32,
}

#[derive(Default)]
pub struct FontService {
    fonts: Vec<Font>,
    font_texture_cache: FontTextureCache,
}

impl FontService {
    pub fn create_font<D>(&mut self, data: D, size: f32) -> anyhow::Result<FontHandle>
    where
        D: AsRef<[u8]>,
    {
        let fontdue_font = fontdue::Font::from_bytes(
            data.as_ref(),
            fontdue::FontSettings {
                scale: size,
                ..Default::default()
            },
        )
        .map_err(|err| anyhow!("could not create fontdue font: {err:?}"))?;

        if self
            .fonts
            .iter()
            .find(|it| it.fontdue_font.file_hash() == fontdue_font.file_hash() && it.size == size)
            .is_some()
        {
            return Err(anyhow!("such font already exists"));
        }

        let index = self.fonts.len();
        self.fonts.push(Font { fontdue_font, size });
        Ok(FontHandle { index })
    }

    // NOTE: Rect that this method returns is gpu texture coords (in range of 0..1).
    pub fn get_texture_for_char<R: Renderer>(
        &mut self,
        font_handle: FontHandle,
        ch: char,
        texture_service: &mut TextureService<R>,
    ) -> (TextureHandle, Rect) {
        let font = &self.fonts[font_handle.index];
        self.font_texture_cache.get_texture_for_char(
            &font.fontdue_font,
            font.size,
            ch,
            texture_service,
        )
    }

    // TODO: do not expose this. instead extend functionality. font service must be capable of
    // providing layout computations (possibly cached).
    pub fn get_fontdue_font(&self, font_handle: FontHandle) -> &fontdue::Font {
        &self.fonts[font_handle.index].fontdue_font
    }
}
