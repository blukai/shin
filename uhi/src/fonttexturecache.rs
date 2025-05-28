use std::{collections::BTreeMap, vec};

use anyhow::anyhow;
use glam::Vec2;

use crate::{Rect, TexturePackerEntryHandle, texturepacker::TexturePacker};

const DEFAULT_TEXTURE_WIDTH: u32 = 256;
const DEFAULT_TEXTURE_HEIGHT: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontTextureCacheFontHandle {
    index: usize,
}

struct FontTextureCacheFont {
    fontdue_font: fontdue::Font,
    size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontTextureCachePageHandle {
    index: usize,
}

pub struct FontTextureCachePage {
    tex_packer: TexturePacker,
    bitmaps: Vec<(Rect, Vec<u8>)>,
}

impl FontTextureCachePage {
    pub fn drain_bitmaps(&mut self) -> vec::Drain<'_, (Rect, Vec<u8>)> {
        self.bitmaps.drain(..)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    font_index: usize,
    ch: char,
}

struct CacheValue {
    page_index: usize,
    entry_handle: TexturePackerEntryHandle,
}

#[derive(Default)]
pub struct FontTextureCache {
    fonts: Vec<FontTextureCacheFont>,
    pages: Vec<FontTextureCachePage>,
    cache: BTreeMap<CacheKey, CacheValue>,
}

impl FontTextureCache {
    pub fn create_font<D>(
        &mut self,
        data: D,
        size: f32,
    ) -> anyhow::Result<FontTextureCacheFontHandle>
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
        self.fonts.push(FontTextureCacheFont { fontdue_font, size });
        Ok(FontTextureCacheFontHandle { index })
    }

    fn allocate_page(&mut self) -> usize {
        let old_len = self.pages.len();
        self.pages.push(FontTextureCachePage {
            tex_packer: TexturePacker::default(),
            bitmaps: Vec::new(),
        });
        old_len
    }

    // TODO: do not expose
    pub fn contains_char(&self, font_handle: FontTextureCacheFontHandle, ch: char) -> bool {
        self.cache.contains_key(&CacheKey {
            font_index: font_handle.index,
            ch,
        })
    }

    // TODO: do not expose
    pub fn allocate_char(&mut self, font_handle: FontTextureCacheFontHandle, ch: char) {
        debug_assert!(!self.contains_char(font_handle, ch));

        let font = &self.fonts[font_handle.index];
        let (metrics, bitmap) = font.fontdue_font.rasterize(ch, font.size);

        // TODO: maybe do not assert, but return an error indicating that the page is too small to
        // fit font of this size.
        assert!(metrics.width as u32 <= DEFAULT_TEXTURE_WIDTH);
        assert!(metrics.height as u32 <= DEFAULT_TEXTURE_HEIGHT);

        let mut page_index = self.pages.len().saturating_sub(1);
        let mut maybe_entry_index = self.pages.get_mut(page_index).and_then(|page| {
            page.tex_packer
                .insert(metrics.width as u32, metrics.height as u32)
        });

        // new page is needed
        if maybe_entry_index.is_none() {
            page_index = self.allocate_page();
            maybe_entry_index = self.pages[page_index]
                .tex_packer
                .insert(metrics.width as u32, metrics.height as u32);
            assert!(maybe_entry_index.is_some());
        }

        // SAFETY: inserted new page above ^
        let entry_handle = unsafe { maybe_entry_index.unwrap_unchecked() };

        let page = &mut self.pages[page_index];
        let entry = page.tex_packer.get(entry_handle);

        let min = Vec2::new(entry.x as f32, entry.y as f32);
        let size = Vec2::new(entry.w as f32, entry.h as f32);
        page.bitmaps.push((Rect::new(min, min + size), bitmap));

        self.cache.insert(
            CacheKey {
                font_index: font_handle.index,
                ch,
            },
            CacheValue {
                page_index,
                entry_handle,
            },
        );
    }

    /// returns a texture and coords for the given character and font; generates and uploads
    /// texture if necessary.
    pub fn get_texture_for_char(
        &mut self,
        font_handle: FontTextureCacheFontHandle,
        ch: char,
    ) -> (FontTextureCachePageHandle, Rect) {
        let cache_key = CacheKey {
            font_index: font_handle.index,
            ch,
        };

        if !self.cache.contains_key(&cache_key) {
            self.allocate_char(font_handle, ch);
        }

        // SAFETY: allocated above ^
        let cache_value = unsafe { self.cache.get(&cache_key).unwrap_unchecked() };
        let page = &self.pages[cache_value.page_index];
        let entry = page.tex_packer.get(cache_value.entry_handle);

        (
            FontTextureCachePageHandle {
                index: cache_value.page_index,
            },
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

    // TODO: do not store fonts within the fonttexturecache
    pub fn get_fontdue_font(&self, font_handle: FontTextureCacheFontHandle) -> &fontdue::Font {
        &self.fonts[font_handle.index].fontdue_font
    }

    // TODO: store all bitmaps, undrainable; and find a way to do deltas or something?
    pub fn iter_dirty_pages_mut(
        &mut self,
    ) -> impl Iterator<Item = (FontTextureCachePageHandle, &mut FontTextureCachePage)> {
        self.pages
            .iter_mut()
            .enumerate()
            .filter_map(|(index, page)| {
                (!page.bitmaps.is_empty()).then(|| (FontTextureCachePageHandle { index }, page))
            })
    }

    pub fn get_page_texture_size(&self) -> (u32, u32) {
        (DEFAULT_TEXTURE_WIDTH, DEFAULT_TEXTURE_HEIGHT)
    }
}
