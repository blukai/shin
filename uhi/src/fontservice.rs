use std::collections::BTreeMap;

use crate::{
    Rect, Renderer, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion,
    TextureService,
};

use anyhow::anyhow;
use glam::Vec2;

// TODO: might want to split furtner into this into FontProvider, FontTextureCache,
// FontLayoutCache.

const DEFAULT_TEXTURE_WIDTH: u32 = 256;
const DEFAULT_TEXTURE_HEIGHT: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    index: usize,
}

struct Font {
    fontdue_font: fontdue::Font,
    size: f32,
}

pub struct TexturePage {
    packer: TexturePacker,
    handle: TextureHandle,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CharCacheKey {
    font_index: usize,
    ch: char,
}

struct CharCacheValue {
    // allows to retrieve TexturePage
    page_index: usize,
    // allows to rertive TexturePackerEntry
    entry_index: usize,
}

#[derive(Default)]
pub struct FontService {
    fonts: Vec<Font>,

    texture_pages: Vec<TexturePage>,
    char_cache: BTreeMap<CharCacheKey, CharCacheValue>,
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

    // TODO: do not expose
    pub fn contains_char(&self, font_handle: FontHandle, ch: char) -> bool {
        self.char_cache.contains_key(&CharCacheKey {
            font_index: font_handle.index,
            ch,
        })
    }

    fn allocate_page<R: Renderer>(&mut self, texture_service: &mut TextureService<R>) -> usize {
        let old_len = self.texture_pages.len();
        self.texture_pages.push(TexturePage {
            packer: TexturePacker::default(),
            handle: texture_service.create_texture(TextureDesc {
                format: TextureFormat::R8Unorm,
                w: DEFAULT_TEXTURE_WIDTH,
                h: DEFAULT_TEXTURE_HEIGHT,
            }),
        });
        old_len
    }

    // TODO: do not expose
    pub fn allocate_char<R: Renderer>(
        &mut self,
        font_handle: FontHandle,
        ch: char,
        texture_service: &mut TextureService<R>,
    ) {
        debug_assert!(!self.contains_char(font_handle, ch));

        let font = &self.fonts[font_handle.index];
        let (metrics, bitmap) = font.fontdue_font.rasterize(ch, font.size);

        // TODO: maybe do not assert, but return an error indicating that the page is too small to
        // fit font of this size.
        assert!(metrics.width as u32 <= DEFAULT_TEXTURE_WIDTH);
        assert!(metrics.height as u32 <= DEFAULT_TEXTURE_HEIGHT);

        let mut page_index = self.texture_pages.len().saturating_sub(1);
        let mut maybe_entry_index = self.texture_pages.get_mut(page_index).and_then(|page| {
            page.packer
                .insert(metrics.width as u32, metrics.height as u32)
        });

        // new page is needed
        if maybe_entry_index.is_none() {
            page_index = self.allocate_page(texture_service);
            maybe_entry_index = self.texture_pages[page_index]
                .packer
                .insert(metrics.width as u32, metrics.height as u32);
            assert!(maybe_entry_index.is_some());
        }

        // SAFETY: inserted new page above ^
        let entry_index = unsafe { maybe_entry_index.unwrap_unchecked() };

        let page = &mut self.texture_pages[page_index];
        let entry = page.packer.get(entry_index);

        texture_service.update_texture(
            page.handle,
            TextureRegion {
                x: entry.x,
                y: entry.y,
                w: entry.w,
                h: entry.h,
            },
            bitmap,
        );

        self.char_cache.insert(
            CharCacheKey {
                font_index: font_handle.index,
                ch,
            },
            CharCacheValue {
                page_index,
                entry_index,
            },
        );
    }

    // NOTE: Rect that this method returns is gpu texture coords (in range of 0..1).
    pub fn get_texture_for_char<R: Renderer>(
        &mut self,
        font_handle: FontHandle,
        ch: char,
        texture_service: &mut TextureService<R>,
    ) -> (TextureHandle, Rect) {
        let cache_key = CharCacheKey {
            font_index: font_handle.index,
            ch,
        };

        if !self.char_cache.contains_key(&cache_key) {
            self.allocate_char(font_handle, ch, texture_service);
        }

        // SAFETY: allocated above ^
        let cache_value = unsafe { self.char_cache.get(&cache_key).unwrap_unchecked() };

        let page = &self.texture_pages[cache_value.page_index];
        let entry = page.packer.get(cache_value.entry_index);

        (
            page.handle,
            Rect::new(
                Vec2::new(
                    entry.x as f32 / DEFAULT_TEXTURE_WIDTH as f32,  // x1
                    entry.y as f32 / DEFAULT_TEXTURE_HEIGHT as f32, // y1
                ),
                Vec2::new(
                    (entry.x + entry.w) as f32 / DEFAULT_TEXTURE_WIDTH as f32, // x2
                    (entry.y + entry.h) as f32 / DEFAULT_TEXTURE_HEIGHT as f32, // y2
                ),
            ),
        )
    }

    // TODO: do not expose this. instead extend functionality. font service must be capable of
    // providing layout computations (possibly cached).
    pub fn get_fontdue_font(&self, font_handle: FontHandle) -> &fontdue::Font {
        &self.fonts[font_handle.index].fontdue_font
    }
}
