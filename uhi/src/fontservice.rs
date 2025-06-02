use crate::{
    Rect, Renderer, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion,
    TextureService,
};

use anyhow::anyhow;
use fontdue::{Font, FontSettings, LineMetrics, Metrics as CharMetrics};
use glam::Vec2;
use nohash::NoHashMap;

// NOTE: fontdue's layouting is way too inconvenient; and there's no way to control its
// allocations.

const DEFAULT_TEXTURE_WIDTH: u32 = 256;
const DEFAULT_TEXTURE_HEIGHT: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    idx: usize,
}

struct CharData {
    metrics: CharMetrics,
    tex_page_idx: usize,
    tex_packer_entry_idx: usize,
}

struct FontData {
    font: fontdue::Font,
    size: f32,
    chars: NoHashMap<u32, CharData>,
    line_metrics: LineMetrics,
}

struct TexturePage {
    tex_packer: TexturePacker,
    tex_handle: TextureHandle,
}

pub struct Char<'a> {
    tex_page: &'a TexturePage,
    font_data: &'a FontData,
    char_data: &'a CharData,
}

impl<'a> Char<'a> {
    #[inline]
    pub fn font_ascent(&self) -> f32 {
        self.font_data.line_metrics.ascent
    }

    #[inline]
    pub fn tex_handle(&self) -> TextureHandle {
        self.tex_page.tex_handle
    }

    #[inline]
    pub fn tex_coords(&self) -> Rect {
        let entry = self
            .tex_page
            .tex_packer
            .get(self.char_data.tex_packer_entry_idx);
        let size = Vec2::new(
            entry.w as f32 / DEFAULT_TEXTURE_WIDTH as f32,
            entry.h as f32 / DEFAULT_TEXTURE_HEIGHT as f32,
        );
        let min = Vec2::new(
            entry.x as f32 / DEFAULT_TEXTURE_WIDTH as f32,
            entry.y as f32 / DEFAULT_TEXTURE_HEIGHT as f32,
        );
        let max = min + size;
        Rect::new(min, max)
    }

    #[inline]
    pub fn metrics(&self) -> &CharMetrics {
        &self.char_data.metrics
    }
}

#[derive(Default)]
pub struct FontService {
    fonts: Vec<FontData>,
    tex_pages: Vec<TexturePage>,
}

impl FontService {
    pub fn create_font<D>(&mut self, data: D, size: f32) -> anyhow::Result<FontHandle>
    where
        D: AsRef<[u8]>,
    {
        let font = Font::from_bytes(
            data.as_ref(),
            FontSettings {
                scale: size,
                ..Default::default()
            },
        )
        .map_err(|err| anyhow!("could not create font: {err:?}"))?;

        if self
            .fonts
            .iter()
            .find(|it| it.font.file_hash() == font.file_hash() && it.size == size)
            .is_some()
        {
            return Err(anyhow!("such font already exists"));
        }

        let Some(line_metrics) = font.horizontal_line_metrics(size) else {
            return Err(anyhow!("could not get line metrics"));
        };

        let idx = self.fonts.len();
        self.fonts.push(FontData {
            font,
            size,
            chars: NoHashMap::default(),
            line_metrics,
        });
        Ok(FontHandle { idx })
    }

    fn create_char_if_not_exists<R: Renderer>(
        &mut self,
        ch: char,
        font_handle: FontHandle,
        texture_service: &mut TextureService<R>,
    ) {
        let font_data = &mut self.fonts[font_handle.idx];
        if font_data.chars.contains_key(&(ch as u32)) {
            return;
        }

        let (metrics, bitmap) = font_data.font.rasterize(ch, font_data.size);
        // TODO: maybe do not assert, but return an error indicating that the page is too small to
        // fit font of this size.
        assert!(metrics.width as u32 <= DEFAULT_TEXTURE_WIDTH);
        assert!(metrics.height as u32 <= DEFAULT_TEXTURE_HEIGHT);

        let mut tex_page_idx: Option<usize> = self.tex_pages.len().checked_sub(1);
        let mut tex_packer_entry_idx: Option<usize> = None;
        // try inserting into existing page if available
        if let Some(pi) = tex_page_idx {
            tex_packer_entry_idx = self.tex_pages[pi]
                .tex_packer
                .insert(metrics.width as u32, metrics.height as u32);
        }
        // allocate new page if needed
        if let None = tex_packer_entry_idx {
            let mut tex_packer = TexturePacker::default();
            tex_packer_entry_idx = tex_packer.insert(metrics.width as u32, metrics.height as u32);
            // NOTE: this assert is somewhat redundant because there's another one above that
            // ensures that char size is <= texture size.
            assert!(tex_packer_entry_idx.is_some());
            tex_page_idx = Some(self.tex_pages.len());
            self.tex_pages.push(TexturePage {
                tex_packer,
                tex_handle: texture_service.enque_create(TextureDesc {
                    format: TextureFormat::R8Unorm,
                    w: DEFAULT_TEXTURE_WIDTH,
                    h: DEFAULT_TEXTURE_HEIGHT,
                }),
            });
        }
        let tex_page_idx = tex_page_idx.unwrap();
        let tex_packer_entry_idx = tex_packer_entry_idx.unwrap();

        let tex_page = &mut self.tex_pages[tex_page_idx];
        let tex_packer_entry = tex_page.tex_packer.get(tex_packer_entry_idx);

        texture_service.enque_update(
            tex_page.tex_handle,
            TextureRegion {
                x: tex_packer_entry.x,
                y: tex_packer_entry.y,
                w: tex_packer_entry.w,
                h: tex_packer_entry.h,
            },
            bitmap,
        );

        font_data.chars.insert(
            ch as u32,
            CharData {
                metrics,
                tex_page_idx,
                tex_packer_entry_idx,
            },
        );
    }

    #[inline]
    pub fn get_or_allocate_char<'a, R: Renderer>(
        &'a mut self,
        ch: char,
        font_handle: FontHandle,
        texture_service: &mut TextureService<R>,
    ) -> Char<'a> {
        self.create_char_if_not_exists(ch, font_handle, texture_service);

        let font_data = &self.fonts[font_handle.idx];
        let char_data = font_data.chars.get(&(ch as u32)).unwrap();
        let tex_page = &self.tex_pages[char_data.tex_page_idx];

        Char {
            font_data,
            char_data,
            tex_page,
        }
    }

    pub fn get_text_width<R: Renderer>(
        &mut self,
        text: &str,
        font_handle: FontHandle,
        texture_service: &mut TextureService<R>,
    ) -> f32 {
        let mut width: f32 = 0.0;
        for ch in text.chars() {
            let ch = self.get_or_allocate_char(ch, font_handle, texture_service);
            width += ch.char_data.metrics.advance_width;
        }
        width
    }

    #[inline]
    pub fn get_font_line_height(&self, font_handle: FontHandle) -> f32 {
        self.fonts[font_handle.idx].line_metrics.new_line_size
    }
}
