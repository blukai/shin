use std::hash::Hash;
use std::mem;

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _};
use nohash::NoHashMap;

use crate::{
    Externs, Rect, TextureDesc, TextureFormat, TextureHandle, TexturePacker, TextureRegion,
    TextureService, Vec2,
};

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 256;
const TEXTURE_GAP: u32 = 1;

#[derive(Debug)]
pub struct TexturePage {
    tex_packer: TexturePacker,
    tex_handle: TextureHandle,
}

impl TexturePage {
    pub fn size(&self) -> (u32, u32) {
        self.tex_packer.texture_size()
    }

    pub fn handle(&self) -> TextureHandle {
        self.tex_handle
    }
}

#[derive(Debug)]
struct Glyph {
    tex_page_idx: usize,
    tex_coords: Rect,
    bounds: Rect,
    advance_width: f32,
}

fn rasterize_glyph<E: Externs>(
    ch: char,
    font: &FontArc,
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

    let mut tex_page_idx: Option<usize> = texture_pages.len().checked_sub(1);
    let mut tex_packer_entry_idx: Option<usize> = None;

    // try inserting into existing page if available
    if let Some(pi) = tex_page_idx {
        tex_packer_entry_idx = texture_pages[pi].tex_packer.insert(width, height);
    }

    // allocate new page if needed
    if let None = tex_packer_entry_idx {
        let mut tex_packer = TexturePacker::new(TEXTURE_WIDTH, TEXTURE_HEIGHT, TEXTURE_GAP);
        tex_packer_entry_idx = tex_packer.insert(width, height);
        // NOTE: this assert is somewhat redundant because there's another one above that
        // ensures that char size is <= texture size.
        assert!(tex_packer_entry_idx.is_some());
        tex_page_idx = Some(texture_pages.len());
        texture_pages.push(TexturePage {
            tex_packer,
            tex_handle: texture_service.enque_create(TextureDesc {
                format: TextureFormat::R8Unorm,
                w: TEXTURE_WIDTH,
                h: TEXTURE_HEIGHT,
            }),
        });
    }

    // NOTE: it is okay to unwrap because necessary allocations happened right above ^.
    let tex_page_idx = tex_page_idx.unwrap();
    let tex_packer_entry_idx = tex_packer_entry_idx.unwrap();

    let tex_page = &mut texture_pages[tex_page_idx];
    let tex_packer_entry = tex_page.tex_packer.get(tex_packer_entry_idx);

    if let Some(og) = &outlined_glyph {
        let buf = texture_service.enque_update(
            tex_page.tex_handle,
            TextureRegion {
                x: tex_packer_entry.x,
                y: tex_packer_entry.y,
                w: tex_packer_entry.w,
                h: tex_packer_entry.h,
            },
        );
        og.draw(|x, y, c| {
            assert!(x <= tex_packer_entry.w);
            assert!(y <= tex_packer_entry.h);
            let pixel = y * tex_packer_entry.w + x;
            buf[pixel as usize] = ((u8::MAX as f32) * c.clamp(0.0, 1.0)) as u8;
        });
    } else {
        // NOTE: should be true if char is empty
        assert_eq!(&bounds, &ab_glyph::Rect::default());
    }

    let min = Vec2::new(
        tex_packer_entry.x as f32 / TEXTURE_WIDTH as f32,
        tex_packer_entry.y as f32 / TEXTURE_HEIGHT as f32,
    );
    let size = Vec2::new(
        tex_packer_entry.w as f32 / TEXTURE_WIDTH as f32,
        tex_packer_entry.h as f32 / TEXTURE_HEIGHT as f32,
    );
    let max = min + size;
    let tex_coords = Rect::new(min, max);

    let bounds = Rect::new(
        Vec2::new(bounds.min.x, bounds.min.y) / scale_factor,
        Vec2::new(bounds.max.x, bounds.max.y) / scale_factor,
    );
    let advance_width = font.as_scaled(px_scale).h_advance(glyph_id) / scale_factor;

    Glyph {
        tex_page_idx,
        tex_coords,
        bounds,
        advance_width,
    }
}

#[derive(Debug)]
pub struct GlyphRef<'a> {
    glyph: &'a Glyph,
    tex_page: &'a TexturePage,
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
    pub fn tex_handle(&self) -> TextureHandle {
        self.tex_page.tex_handle
    }

    #[inline]
    pub fn tex_coords(&self) -> Rect {
        self.glyph.tex_coords
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle {
    idx: u32,
}

// NOTE: to many fidgeting is needed to hash floats. this is easier.
#[inline(always)]
fn make_font_instance_key(font_handle: FontHandle, pt_size: f32) -> u64 {
    (font_handle.idx as u64) << 32 | (unsafe { mem::transmute::<_, u32>(pt_size) } as u64)
}

#[derive(Debug)]
struct FontInstance {
    glyphs: NoHashMap<u32, Glyph>,

    px_scale: PxScale,
    scale_factor: f32,

    height: f32,
    ascent: f32,
    /// see https://developer.mozilla.org/en-US/docs/Web/CSS/length#ch
    typical_advance_width: f32,
}

impl FontInstance {
    fn new(font: &FontArc, pt_size: f32, scale_factor: f32) -> Self {
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
            glyphs: NoHashMap::default(),

            px_scale,
            scale_factor,

            height: ascent - descent + line_gap,
            ascent,
            typical_advance_width,
        }
    }
}

// NOTE: font instance != font. a single font may parent multiple font instances.
#[derive(Debug)]
pub struct FontInstanceRefMut<'a> {
    font: &'a FontArc,
    font_instance: &'a mut FontInstance,
    tex_pages: &'a mut Vec<TexturePage>,
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
    ) -> GlyphRef {
        let glyph = self
            .font_instance
            .glyphs
            .entry(ch as u32)
            // glyph does not exist
            .or_insert_with(|| {
                rasterize_glyph(
                    ch,
                    self.font,
                    self.font_instance.px_scale,
                    self.font_instance.scale_factor,
                    &mut self.tex_pages,
                    texture_service,
                )
            });
        let tex_page = &self.tex_pages[glyph.tex_page_idx];
        GlyphRef { glyph, tex_page }
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

    /// the most obvious use case for this is to check something / do some assertions in tests
    /// maybe.
    pub fn iter_glyphs(&self) -> impl Iterator<Item = GlyphRef> {
        self.font_instance.glyphs.values().map(|glyph| GlyphRef {
            glyph,
            tex_page: &self.tex_pages[glyph.tex_page_idx],
        })
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
            font: self.font,
            font_instance: unsafe { &mut *(self.font_instance as *mut FontInstance) },
            tex_pages: unsafe { &mut *(self.tex_pages as *mut Vec<TexturePage>) },
        }
    }
}

#[derive(Default)]
pub struct FontService {
    scale_factor: Option<f32>,
    // NOTE: i don't need an Arc, but whatever. FontArc makes it convenient because it wraps both
    // FontRef and FontVec.
    fonts: Vec<FontArc>,
    tex_pages: Vec<TexturePage>,
    font_instances: NoHashMap<u64, FontInstance>,
}

impl FontService {
    pub fn set_scale_factor<E: Externs>(
        &mut self,
        scale_factor: f32,
        texture_service: &mut TextureService<E>,
    ) {
        self.scale_factor = Some(scale_factor);

        for tex_page in self.tex_pages.drain(..) {
            texture_service.enque_destroy(tex_page.tex_handle);
        }

        self.font_instances.clear();
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

    pub fn get_font_instance(
        &mut self,
        font_handle: FontHandle,
        pt_size: f32,
    ) -> FontInstanceRefMut {
        assert!(pt_size > 0.0);

        let font = &self.fonts[font_handle.idx as usize];
        let font_instance = self
            .font_instances
            .entry(make_font_instance_key(font_handle, pt_size))
            .or_insert_with(|| {
                FontInstance::new(font, pt_size, self.scale_factor.unwrap_or(1.0) as f32)
            });
        FontInstanceRefMut {
            font,
            font_instance,
            tex_pages: &mut self.tex_pages,
        }
    }

    /// allows to view font texture atlases.
    pub fn iter_texture_pages(&self) -> impl Iterator<Item = &TexturePage> {
        self.tex_pages.iter()
    }
}
