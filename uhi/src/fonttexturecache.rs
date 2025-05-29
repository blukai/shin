use std::collections::BTreeMap;

use crate::{
    Rect, Renderer, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion,
    TextureService,
};

use glam::Vec2;

// TODO: might want to split furtner into this into FontProvider, FontTextureCache,
// FontLayoutCache.

const DEFAULT_TEXTURE_WIDTH: u32 = 256;
const DEFAULT_TEXTURE_HEIGHT: u32 = 256;

struct TexturePage {
    packer: TexturePacker,
    handle: TextureHandle,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    font_file_hash: usize,
    font_size: [u8; 4],
    ch: char,
}

impl CacheKey {
    fn new(font: &fontdue::Font, font_size: f32, ch: char) -> Self {
        Self {
            font_file_hash: font.file_hash(),
            font_size: font_size.to_ne_bytes(),
            ch,
        }
    }
}

struct CacheValue {
    // allows to retrieve TexturePage
    page_index: usize,
    // allows to rertive TexturePackerEntry
    packer_entry_index: usize,
}

#[derive(Default)]
pub struct FontTextureCache {
    texture_pages: Vec<TexturePage>,
    cache: BTreeMap<CacheKey, CacheValue>,
}

impl FontTextureCache {
    fn allocate_page<R: Renderer>(&mut self, texture_service: &mut TextureService<R>) -> usize {
        let old_len = self.texture_pages.len();
        self.texture_pages.push(TexturePage {
            packer: TexturePacker::default(),
            handle: texture_service.enque_create(TextureDesc {
                format: TextureFormat::R8Unorm,
                w: DEFAULT_TEXTURE_WIDTH,
                h: DEFAULT_TEXTURE_HEIGHT,
            }),
        });
        old_len
    }

    fn allocate_char<R: Renderer>(
        &mut self,
        font: &fontdue::Font,
        font_size: f32,
        ch: char,
        texture_service: &mut TextureService<R>,
    ) {
        let char_cache_key = CacheKey::new(font, font_size, ch);
        debug_assert!(!self.cache.contains_key(&char_cache_key));

        let (metrics, bitmap) = font.rasterize(ch, font_size);
        // TODO: maybe do not assert, but return an error indicating that the page is too small to
        // fit font of this size.
        assert!(metrics.width as u32 <= DEFAULT_TEXTURE_WIDTH);
        assert!(metrics.height as u32 <= DEFAULT_TEXTURE_HEIGHT);

        let mut page_index = self.texture_pages.len().saturating_sub(1);

        let mut maybe_packer_entry_index =
            self.texture_pages.get_mut(page_index).and_then(|page| {
                page.packer
                    .insert(metrics.width as u32, metrics.height as u32)
            });
        // new page is needed
        if maybe_packer_entry_index.is_none() {
            page_index = self.allocate_page(texture_service);
            maybe_packer_entry_index = self.texture_pages[page_index]
                .packer
                .insert(metrics.width as u32, metrics.height as u32);
            assert!(maybe_packer_entry_index.is_some());
        }
        // SAFETY: inserted new page above ^
        let packer_entry_index = unsafe { maybe_packer_entry_index.unwrap_unchecked() };

        let page = &mut self.texture_pages[page_index];
        let packer_entry = page.packer.get(packer_entry_index);

        texture_service.enque_update(
            page.handle,
            TextureRegion {
                x: packer_entry.x,
                y: packer_entry.y,
                w: packer_entry.w,
                h: packer_entry.h,
            },
            bitmap,
        );

        self.cache.insert(
            char_cache_key,
            CacheValue {
                page_index,
                packer_entry_index,
            },
        );
    }

    // NOTE: Rect that this method returns is gpu texture coords (in range of 0..1).
    pub fn get_texture_for_char<R: Renderer>(
        &mut self,
        font: &fontdue::Font,
        font_size: f32,
        ch: char,
        texture_service: &mut TextureService<R>,
    ) -> (TextureHandle, Rect) {
        let key = CacheKey::new(font, font_size, ch);

        if !self.cache.contains_key(&key) {
            self.allocate_char(font, font_size, ch, texture_service);
        }

        // SAFETY: allocated above ^
        let cache_value = unsafe { self.cache.get(&key).unwrap_unchecked() };

        let page = &self.texture_pages[cache_value.page_index];
        let packer_entry = page.packer.get(cache_value.packer_entry_index);

        (
            page.handle,
            Rect::new(
                Vec2::new(
                    packer_entry.x as f32 / DEFAULT_TEXTURE_WIDTH as f32, // x1
                    packer_entry.y as f32 / DEFAULT_TEXTURE_HEIGHT as f32, // y1
                ),
                Vec2::new(
                    (packer_entry.x + packer_entry.w) as f32 / DEFAULT_TEXTURE_WIDTH as f32, // x2
                    (packer_entry.y + packer_entry.h) as f32 / DEFAULT_TEXTURE_HEIGHT as f32, // y2
                ),
            ),
        )
    }
}
