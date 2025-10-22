use std::hash::{BuildHasherDefault, Hash};

use ab_glyph::{Font as _, FontArc, ScaleFont as _};
use nohash::NoHashMap;

use crate::{
    Rect, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion, TextureService,
    Vec2,
};

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 256;
const TEXTURE_GAP: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    idx: u32,
}

// NOTE: FontDesc cannot be used as a hash map key because f32 does not implement Eq.
#[derive(Debug, Clone, PartialEq)]
pub struct FontDesc {
    pub handle: FontHandle,
    pub pt_size: f32,
    pub scale_factor: f32,
}

// NOTE: to many fidgeting is needed to hash floats. this is easier.
#[inline(always)]
fn pack_font_desc(desc: &FontDesc) -> u64 {
    debug_assert!(desc.pt_size <= 65500.0);
    debug_assert!(desc.scale_factor <= 65500.0);
    ((desc.handle.idx as u64) << 32)
        | ((desc.pt_size.to_bits() as u64 as u64) << 16)
        | (desc.scale_factor.to_bits() as u64 as u64)
}

#[derive(Debug)]
pub struct TexturePage {
    pub texture_packer: TexturePacker,
    pub texture_handle: TextureHandle,
}

#[derive(Debug)]
struct Glyph {
    texture_page_idx: usize,
    _texture_packer_entry_idx: usize,
    texture_coords: Rect,
    bounds: Rect,
    advance_width: f32,
}

fn rasterize_glyph(
    ch: char,
    font: &FontArc,
    px_scale: ab_glyph::PxScale,
    scale_factor: f32,
    texture_pages: &mut Vec<TexturePage>,
    texture_service: &mut TextureService,
) -> Glyph {
    let glyph_id = font.glyph_id(ch);
    let glyph = glyph_id.with_scale(px_scale);
    let outlined_glyph = font.outline_glyph(glyph);

    let bounds = outlined_glyph
        .as_ref()
        .map(|og| og.px_bounds())
        .unwrap_or_else(|| match ch {
            ch if ch.is_whitespace() => ab_glyph::Rect::default(),
            other => todo!("need fallback/replacement chars: {other}"),
        });

    let width = bounds.width() as u32;
    let height = bounds.height() as u32;
    // TODO: maybe do not assert, but return an error indicating that the page is too small to
    // fit font of this size.
    assert!(width <= TEXTURE_WIDTH);
    assert!(height <= TEXTURE_HEIGHT);

    let mut texture_page_idx: Option<usize> = None;
    let mut texture_packer_entry_idx: Option<usize> = None;
    // try inserting into existing pages
    for (page_idx, texture_page) in texture_pages.iter_mut().enumerate() {
        if let Some(packer_entry_idx) = texture_page.texture_packer.insert(width, height) {
            texture_page_idx = Some(page_idx);
            texture_packer_entry_idx = Some(packer_entry_idx);
        }
    }
    // allocate new page if needed
    let (texture_page_idx, texture_packer_entry_idx) =
        match (texture_page_idx, texture_packer_entry_idx) {
            (Some(page_idx), Some(packer_entry_idx)) => (page_idx, packer_entry_idx),
            (None, None) => {
                let mut texture_packer =
                    TexturePacker::new(TEXTURE_WIDTH, TEXTURE_HEIGHT, TEXTURE_GAP);
                let texture_handle = texture_service.create(TextureDesc {
                    format: TextureFormat::R8Unorm,
                    w: TEXTURE_WIDTH,
                    h: TEXTURE_HEIGHT,
                });
                // NOTE: this unwrap is somewhat redundant because there's an assertion above that
                // ensures that char size is <= texture size.
                let packer_entry_idx = texture_packer.insert(width, height).unwrap();
                let page_idx = texture_pages.len();
                texture_pages.push(TexturePage {
                    texture_packer,
                    texture_handle,
                });
                (page_idx, packer_entry_idx)
            }
            _ => unreachable!(),
        };

    let texture_page = &mut texture_pages[texture_page_idx];
    let texture_packer_entry = texture_page
        .texture_packer
        .get_entry(texture_packer_entry_idx);

    if let Some(og) = &outlined_glyph {
        let upload_buffer = texture_service.get_upload_buf_mut(
            texture_page.texture_handle,
            TextureRegion {
                x: texture_packer_entry.x,
                y: texture_packer_entry.y,
                w: texture_packer_entry.w,
                h: texture_packer_entry.h,
            },
        );
        og.draw(|x, y, c| {
            assert!(x <= texture_packer_entry.w);
            assert!(y <= texture_packer_entry.h);
            let pixel = y * texture_packer_entry.w + x;
            upload_buffer[pixel as usize] = ((u8::MAX as f32) * c.clamp(0.0, 1.0)) as u8;
        });
    } else {
        // NOTE: should be true if char is empty
        assert_eq!(&bounds, &ab_glyph::Rect::default());
    }

    let min = Vec2::new(
        texture_packer_entry.x as f32 / TEXTURE_WIDTH as f32,
        texture_packer_entry.y as f32 / TEXTURE_HEIGHT as f32,
    );
    let size = Vec2::new(
        texture_packer_entry.w as f32 / TEXTURE_WIDTH as f32,
        texture_packer_entry.h as f32 / TEXTURE_HEIGHT as f32,
    );
    let max = min + size;
    let texture_coords = Rect::new(min, max);

    let bounds = Rect::new(
        Vec2::new(bounds.min.x, bounds.min.y) / scale_factor,
        Vec2::new(bounds.max.x, bounds.max.y) / scale_factor,
    );
    let advance_width = font.as_scaled(px_scale).h_advance(glyph_id) / scale_factor;

    Glyph {
        texture_page_idx,
        _texture_packer_entry_idx: texture_packer_entry_idx,
        texture_coords,
        bounds,
        advance_width,
    }
}

#[derive(Debug)]
pub struct GlyphRef<'a> {
    glyph: &'a Glyph,
    texture_page: &'a TexturePage,
}

impl<'a> GlyphRef<'a> {
    #[inline]
    pub fn bounds(&self) -> Rect {
        self.glyph.bounds
    }

    #[inline]
    pub fn advance_width(&self) -> f32 {
        self.glyph.advance_width
    }

    #[inline]
    pub fn texture_handle(&self) -> TextureHandle {
        self.texture_page.texture_handle
    }

    #[inline]
    pub fn texture_coords(&self) -> Rect {
        self.glyph.texture_coords
    }
}

#[derive(Debug)]
pub struct FontInstance {
    // NOTE: TexturePacker's partitioning is theoretically fine for same-sized rects.
    // but not for distinct ones.
    // even monospace fonts will want to allocate distinct rects because bounds of majority of the
    // glyphs differ.
    // thuse it makes more sense for font instance to own texture pages and when font instance need
    // to be dropped - drop texture packers (and glyphs) along with it.
    texture_pages: Vec<TexturePage>,
    glyphs: NoHashMap<u32, Glyph>,

    px_scale: ab_glyph::PxScale,
    scale_factor: f32,

    height: f32,
    ascent: f32,
    /// see https://developer.mozilla.org/en-US/docs/Web/CSS/length#ch
    typical_advance_width: f32,

    // NOTE: touched_this_iteration is a flag that determines whether this font instance needs to
    // be evicted or not.
    in_use: bool,
    font: FontArc,
}

impl FontInstance {
    fn new(font: FontArc, pt_size: f32, scale_factor: f32) -> Self {
        // NOTE: ab_glyph is weird, for more infor on what's going on with pt_size and scale_factor
        // see https://github.com/alexheretic/ab-glyph/issues/14.

        let font_scale = font
            .units_per_em()
            .map(|units_per_em| font.height_unscaled() / units_per_em)
            .unwrap_or(1.0);
        let px_scale = ab_glyph::PxScale::from(pt_size * scale_factor * font_scale);
        let scaled = font.as_scaled(px_scale);

        let ascent = scaled.ascent() / scale_factor;
        let descent = scaled.descent() / scale_factor;
        let line_gap = scaled.line_gap() / scale_factor;

        // see https://developer.mozilla.org/en-US/docs/Web/CSS/length#ch
        let typical_advance_width = scaled.h_advance(font.glyph_id('0')) / scale_factor;

        Self {
            texture_pages: Vec::default(),
            // NOTE: 128 is num of ascii code points.
            glyphs: NoHashMap::with_capacity_and_hasher(128, BuildHasherDefault::default()),

            px_scale,
            scale_factor,

            height: ascent - descent + line_gap,
            ascent,
            typical_advance_width,

            in_use: false,
            font,
        }
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.height
    }

    #[inline]
    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    #[inline]
    pub fn typical_advance_width(&self) -> f32 {
        self.typical_advance_width
    }

    /// gets a glyph for a given character, rasterizing and caching it if not already cached.
    /// glyphs are cached per font instance (font + size combination) for subsequent lookups.
    ///
    /// NOTE: this techinaclly dones't have to be &mut, but &mut prevents overlapping borrows from
    /// the same handle (e.g., holding one glyph while requesting another).
    pub fn get_or_rasterize_glyph(
        &mut self,
        ch: char,
        texture_service: &mut TextureService,
    ) -> GlyphRef<'_> {
        let glyph = self.glyphs.entry(ch as u32).or_insert_with(|| {
            rasterize_glyph(
                ch,
                &self.font,
                self.px_scale,
                self.scale_factor,
                &mut self.texture_pages,
                texture_service,
            )
        });
        let texture_page = &mut self.texture_pages[glyph.texture_page_idx];
        GlyphRef {
            glyph,
            texture_page,
        }
    }

    pub fn compute_text_width(&mut self, text: &str, texture_service: &mut TextureService) -> f32 {
        let mut width: f32 = 0.0;
        for ch in text.chars() {
            let glyph = self.get_or_rasterize_glyph(ch, texture_service);
            width += glyph.glyph.advance_width;
        }
        width
    }

    pub fn iter_texture_pages(&self) -> impl Iterator<Item = &TexturePage> {
        self.texture_pages.iter()
    }
}

#[derive(Default)]
pub struct FontService {
    fonts: Vec<FontArc>,
    font_instances: NoHashMap<u64, FontInstance>,
}

impl FontService {
    pub fn remove_unused_font_instances(&mut self, texture_service: &mut TextureService) {
        let mut num_removed: usize = 0;

        self.font_instances.retain(|_, font_instance| {
            if font_instance.in_use {
                // NOTE: we want to reset this to start a new round of tracking.
                font_instance.in_use = false;
                return true;
            }

            font_instance.texture_pages.iter().for_each(|texture_page| {
                texture_service.delete(texture_page.texture_handle);
            });
            num_removed += 1;
            false
        });

        if num_removed > 0 {
            log::debug!("removed {num_removed} unused font instances");
        }
    }

    pub fn register_font_slice(&mut self, font_data: &'static [u8]) -> anyhow::Result<FontHandle> {
        let idx = self.fonts.len();
        self.fonts.push(FontArc::try_from_slice(font_data)?);
        assert!(idx <= u32::MAX as usize);
        Ok(FontHandle { idx: idx as u32 })
    }

    pub fn register_font_vec(&mut self, font_data: Vec<u8>) -> anyhow::Result<FontHandle> {
        let idx = self.fonts.len();
        self.fonts.push(FontArc::try_from_vec(font_data)?);
        assert!(idx <= u32::MAX as usize);
        Ok(FontHandle { idx: idx as u32 })
    }

    pub fn get_disjoint_font_instances_mut<const N: usize>(
        &mut self,
        descs: [FontDesc; N],
    ) -> [&'_ mut FontInstance; N] {
        let ptrs = descs.map(|desc| {
            assert!(desc.pt_size > 0.0);
            let key = pack_font_desc(&desc);
            let font_instance = self.font_instances.entry(key).or_insert_with(|| {
                let font = &self.fonts[desc.handle.idx as usize];
                FontInstance::new(FontArc::clone(&font), desc.pt_size, desc.scale_factor)
            });
            font_instance.in_use = true;
            font_instance as *mut _
        });

        for (i, ptr) in ptrs.iter().enumerate() {
            if ptrs[..i].contains(ptr) {
                panic!("duplicate keys");
            }
        }

        ptrs.map(|ptr| unsafe { &mut *ptr })
    }

    pub fn get_font_instance_mut(&mut self, desc: FontDesc) -> &mut FontInstance {
        self.get_disjoint_font_instances_mut([desc])[0]
    }

    pub fn iter_font_instances(&self) -> impl Iterator<Item = &FontInstance> {
        self.font_instances.values()
    }
}
