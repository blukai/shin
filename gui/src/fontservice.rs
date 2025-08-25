use std::hash::Hash;

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _};
use nohash::NoHashMap;

use crate::{
    Externs, Rect, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion,
    TextureService, Vec2,
};

// TODO: maybe do not depend on texture service. instead produce output?

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 256;
const TEXTURE_GAP: u32 = 1;

#[derive(Debug)]
pub struct TexturePage {
    pub texture_packer: TexturePacker,
    pub texture_handle: TextureHandle,
}

#[derive(Debug)]
struct Glyph {
    texture_page_idx: usize,
    #[allow(dead_code, reason = "useful for debugging")]
    texture_packer_entry_idx: usize,
    texture_coords: Rect,
    bounds: Rect,
    advance_width: f32,
}

fn rasterize_glyph<E: Externs>(
    ch: char,
    font: FontArc,
    px_scale: PxScale,
    scale_factor: f32,
    texture_pages: &mut Vec<TexturePage>,
    texture_service: &mut TextureService<E>,
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
                // NOTE: this unwrap is somewhat redundant because there's an assertion above that
                // ensures that char size is <= texture size.
                let packer_entry_idx = texture_packer.insert(width, height).unwrap();
                let page_idx = texture_pages.len();
                texture_pages.push(TexturePage {
                    texture_packer,
                    texture_handle: texture_service.enque_create(TextureDesc {
                        format: TextureFormat::R8Unorm,
                        w: TEXTURE_WIDTH,
                        h: TEXTURE_HEIGHT,
                    }),
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
        let buf = texture_service.enque_update(
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
            buf[pixel as usize] = ((u8::MAX as f32) * c.clamp(0.0, 1.0)) as u8;
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
        texture_packer_entry_idx,
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
    pub fn bounding_rect(&self) -> Rect {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    idx: u32,
}

// NOTE: to many fidgeting is needed to hash floats. this is easier.
#[inline(always)]
fn make_font_instance_key(font_handle: FontHandle, pt_size: f32, scale_factor: f32) -> u64 {
    (font_handle.idx as u64) << 32 | ((pt_size * scale_factor).to_bits() as u64)
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

    px_scale: PxScale,
    scale_factor: f32,

    height: f32,
    ascent: f32,
    /// see https://developer.mozilla.org/en-US/docs/Web/CSS/length#ch
    typical_advance_width: f32,

    // TODO: this will produce bad results when rendering to multiple surfaces.
    // a single "pass" will consist of mutlple "frames" on different "surfaces".
    touched_this_frame: bool,
}

impl FontInstance {
    fn new(font: FontArc, pt_size: f32, scale_factor: f32) -> Self {
        // NOTE: see https://github.com/alexheretic/ab-glyph/issues/14 for details.
        let font_scale = font
            .units_per_em()
            .map(|units_per_em| font.height_unscaled() / units_per_em)
            .unwrap_or(1.0);
        let px_scale = PxScale::from(pt_size * scale_factor * font_scale);
        let scaled = font.as_scaled(px_scale);

        let ascent = scaled.ascent() / scale_factor;
        let descent = scaled.descent() / scale_factor;
        let line_gap = scaled.line_gap() / scale_factor;
        // see https://developer.mozilla.org/en-US/docs/Web/CSS/length#ch
        let typical_advance_width = scaled.h_advance(font.glyph_id('0')) / scale_factor;

        Self {
            texture_pages: Vec::default(),
            glyphs: NoHashMap::default(),

            px_scale,
            scale_factor,

            height: ascent - descent + line_gap,
            ascent,
            typical_advance_width,

            touched_this_frame: false,
        }
    }

    pub fn iter_texture_pages(&self) -> impl Iterator<Item = &TexturePage> {
        self.texture_pages.iter()
    }
}

// NOTE: font instance != font. a single font may parent multiple font instances.
#[derive(Debug)]
pub struct FontInstanceRefMut<'a> {
    font: FontArc,
    font_instance: &'a mut FontInstance,
}

impl<'a> FontInstanceRefMut<'a> {
    #[inline]
    pub fn height(&self) -> f32 {
        self.font_instance.height
    }

    #[inline]
    pub fn ascent(&self) -> f32 {
        self.font_instance.ascent
    }

    #[inline]
    pub fn typical_advance_width(&self) -> f32 {
        self.font_instance.typical_advance_width
    }

    /// gets a glyph for a given character, rasterizing and caching it if not already cached.
    /// glyphs are cached per font instance (font + size combination) for subsequent lookups.
    pub fn get_or_rasterize_glyph<E: Externs>(
        &mut self,
        ch: char,
        texture_service: &mut TextureService<E>,
    ) -> GlyphRef<'_> {
        let glyph = self
            .font_instance
            .glyphs
            .entry(ch as u32)
            .or_insert_with(|| {
                rasterize_glyph(
                    ch,
                    FontArc::clone(&self.font),
                    self.font_instance.px_scale,
                    self.font_instance.scale_factor,
                    &mut self.font_instance.texture_pages,
                    texture_service,
                )
            });
        let texture_page = &self.font_instance.texture_pages[glyph.texture_page_idx];
        GlyphRef {
            glyph,
            texture_page,
        }
    }

    pub fn compute_text_width<E: Externs>(
        &mut self,
        text: &str,
        texture_service: &mut TextureService<E>,
    ) -> f32 {
        let mut width: f32 = 0.0;
        for ch in text.chars() {
            let glyph = self.get_or_rasterize_glyph(ch, texture_service);
            width += glyph.glyph.advance_width;
        }
        width
    }

    /// all this really is is an alias for `clone` xd.
    ///
    /// you don't want to pass a reference to a thing that is carrying references; that creates
    /// more indirection that i am willing to tolerate for no good reason.
    ///
    /// this is a hack somewhat and i am totally fine with it being a hack. rust really-really
    /// sucks at certain things. just don't fuck up.
    ///
    /// sometimes multiple functions may want [`FontInstanceRefMut`], but i do not believe that
    /// doing a lookup on [`FontService`] for it multiple times is sane, and you can't derive Clone
    /// for thisbecause it contains mutable references that are not "clonable">
    pub fn reborrow_mut(&mut self) -> FontInstanceRefMut<'a> {
        Self {
            font: FontArc::clone(&self.font),
            font_instance: unsafe { &mut *(self.font_instance as *mut FontInstance) },
        }
    }
}

#[derive(Default)]
pub struct FontService {
    // NOTE: i don't need an Arc, but whatever. FontArc makes it convenient because it wraps both
    // FontRef and FontVec.
    fonts: Vec<FontArc>,
    font_instances: NoHashMap<u64, FontInstance>,
}

impl FontService {
    pub fn begin_frame(&mut self) {
        // TODO: should i just reset this at the end of the frame?
        self.font_instances.values_mut().for_each(|font_instance| {
            font_instance.touched_this_frame = false;
        });
    }

    pub fn end_frame<E: Externs>(&mut self, texture_service: &mut TextureService<E>) {
        let mut num_font_instances_evicted: usize = 0;

        self.font_instances.retain(|_, font_instance| {
            if font_instance.touched_this_frame {
                return true;
            }

            font_instance.texture_pages.iter().for_each(|texture_page| {
                texture_service.enque_destroy(texture_page.texture_handle);
            });
            num_font_instances_evicted += 1;
            false
        });

        if num_font_instances_evicted > 0 {
            log::debug!(
                "FontService::end_frame: evicted {num_font_instances_evicted} unused font instances"
            );
        }
    }

    pub fn register_font_slice(&mut self, font_data: &'static [u8]) -> anyhow::Result<FontHandle> {
        let idx = self.fonts.len();
        self.fonts.push(FontArc::try_from_slice(font_data)?);
        Ok(FontHandle { idx: idx as u32 })
    }

    pub fn register_font_vec(&mut self, font_data: Vec<u8>) -> anyhow::Result<FontHandle> {
        let idx = self.fonts.len();
        self.fonts.push(FontArc::try_from_vec(font_data)?);
        Ok(FontHandle { idx: idx as u32 })
    }

    pub fn get_or_create_font_instance(
        &mut self,
        font_handle: FontHandle,
        pt_size: f32,
        scale_factor: f32,
    ) -> FontInstanceRefMut<'_> {
        assert!(pt_size > 0.0);

        let font = &self.fonts[font_handle.idx as usize];
        let font_instance = self
            .font_instances
            .entry(make_font_instance_key(font_handle, pt_size, scale_factor))
            .or_insert_with(|| FontInstance::new(FontArc::clone(font), pt_size, scale_factor));
        font_instance.touched_this_frame = true;

        FontInstanceRefMut {
            font: FontArc::clone(font),
            font_instance,
        }
    }

    pub fn iter_font_instances(&self) -> impl Iterator<Item = &FontInstance> {
        self.font_instances.values()
    }
}
