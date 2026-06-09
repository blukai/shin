use std::cell::{RefCell, RefMut};
use std::collections::hash_map;
use std::fmt::Debug;
use std::ops::{Deref as _, DerefMut as _};
use std::{fmt, mem, slice};

use anyhow::anyhow;
use mars::alloc::{ErasedAllocator, TempAllocator};
use mars::array::ResizableArray;
use mars::boxed::Box;
use mars::fxhash::FxBuildHasher;
use mars::handlearray::{Handle, HandleArray};
use stb_sys::*;

use crate::fontservice::TexturePage;
use crate::{Rect, TextureDesc, TextureFormat, TexturePacker, TextureRegion, TextureService, Vec2};

const TEXTURE_WIDTH: u32 = 256;
const TEXTURE_HEIGHT: u32 = 256;
const TEXTURE_GAP: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    // NOTE: these are min,max across all glyph bounding boxes.
    //   see https://learn.microsoft.com/en-us/typography/opentype/spec/head
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

fn get_font_metrics(info: &stbtt_fontinfo, scale: f32) -> FontMetrics {
    let (mut ascent, mut descent, mut line_gap) = (0, 0, 0);
    let (mut x0, mut y0, mut x1, mut y1) = (0, 0, 0, 0);
    unsafe {
        stbtt_GetFontVMetrics(info, &mut ascent, &mut descent, &mut line_gap);
        stbtt_GetFontBoundingBox(info, &mut x0, &mut y0, &mut x1, &mut y1);
    }
    FontMetrics {
        ascent: ascent as f32 * scale,
        descent: descent as f32 * scale,
        line_gap: line_gap as f32 * scale,
        x0: x0 as f32 * scale,
        y0: y0 as f32 * scale,
        x1: x1 as f32 * scale,
        y1: y1 as f32 * scale,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    pub advance_width: f32,
    pub left_side_bearing: f32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

fn get_glyph_metrics(info: &stbtt_fontinfo, glyph_index: i32, px: f32, scale: f32) -> GlyphMetrics {
    let (mut advance_width, mut left_side_bearing) = (0, 0);
    let (mut x0, mut y0, mut x1, mut y1) = (0, 0, 0, 0);
    unsafe {
        stbtt_GetGlyphHMetrics(
            info,
            glyph_index,
            &mut advance_width,
            &mut left_side_bearing,
        );
        stbtt_GetGlyphBox(info, glyph_index, &mut x0, &mut y0, &mut x1, &mut y1);
    }
    let padding = SDF_PADDING as f32 * (px / SDF_PX);
    GlyphMetrics {
        advance_width: advance_width as f32 * scale,
        left_side_bearing: left_side_bearing as f32 * scale,
        x0: (x0 as f32 * scale - padding).floor(),
        y0: (-y1 as f32 * scale - padding).floor(),
        x1: (x1 as f32 * scale + padding).ceil(),
        y1: (-y0 as f32 * scale + padding).ceil(),
    }
}

// NOTE: see the "Font Size in Pixels or Points" section in stb_truetype.h for info abotut what this
// 96.0 / 72.0 = 1.333 is.
//
// NOTE: additionally see https://github.com/nothings/stb/issues/689 for the explanation on why
// stbtt_ScaleForMappingEmToPixels is used instead of stbtt_ScaleForPixelHeight.
// also https://github.com/flutter/flutter/issues/146080
//
// NOTE: whenever you update these make sure to also update shader. :SdfShader
const POINTS_TO_PIXELS: f32 = 96.0 / 72.0;
const SDF_PX: f32 = 64.0 * POINTS_TO_PIXELS;
const SDF_PADDING: i32 = 8;
const SDF_ON_EDGE_VALUE: u8 = 128;
const SDF_PIXEL_DIST_SCALE: f32 = 32.0;

#[derive(Clone, Copy)]
struct GlyphSdf<'temp> {
    pixels: &'temp [u8],
    width: i32,
    height: i32,
    xoff: i32,
    yoff: i32,
}

impl<'temp> Debug for GlyphSdf<'temp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlyphSdf")
            .field("pixels", &(&[] as &[u8]) as _)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("xoff", &self.xoff)
            .field("yoff", &self.yoff)
            .finish()
    }
}

fn compute_glyph_sdf<'temp>(
    info: &mut stbtt_fontinfo,
    glyph_index: i32,
    temp: &'temp TempAllocator<'temp>,
) -> Option<GlyphSdf<'temp>> {
    unsafe {
        // NOTE: stbtt_GetGlyphSDF does not store anything it allocates during rasterization in
        // its context(/stbtt_fontinfo) or wherever.
        // all allocations are temporary.
        let userdata_backup = info.userdata;

        // NOTE: if you get straight reference as
        //   info.userdata = &ErasedAllocator::new(temp) as *const _ as _;
        //   you'll be getting segfaults in release builds.
        //   rust compiler bug?
        let erased_temp = ErasedAllocator::new(temp);
        info.userdata = &erased_temp as *const _ as _;

        let scale = stbtt_ScaleForMappingEmToPixels(info, SDF_PX);
        let (mut width, mut height, mut xoff, mut yoff) = (0, 0, 0, 0);
        let pixels = stbtt_GetGlyphSDF(
            info,                 // info: *const stbtt_fontinfo,
            scale,                // scale: f32,
            glyph_index,          // codepoint: ::core::ffi::c_int,
            SDF_PADDING,          // padding: ::core::ffi::c_int,
            SDF_ON_EDGE_VALUE,    // onedge_value: ::core::ffi::c_uchar,
            SDF_PIXEL_DIST_SCALE, // pixel_dist_scale: f32,
            &mut width,           // width: *mut ::core::ffi::c_int,
            &mut height,          // height: *mut ::core::ffi::c_int,
            &mut xoff,            // xoff: *mut ::core::ffi::c_int,
            &mut yoff,            // yoff: *mut ::core::ffi::c_int,
        );

        // NOTE: safe to put backedup userdata thing back in.
        //   note that things would be segfaulting if the above temp assignment of userdata
        //   would escape boundary of this function.
        info.userdata = userdata_backup;

        if pixels.is_null() {
            return None;
        }
        let pixels: &'temp [u8] = slice::from_raw_parts(pixels, width as usize * height as usize);
        Some(GlyphSdf {
            width,
            height,
            xoff,
            yoff,
            pixels,
        })
    }
}

// ----
// the font service thing

pub enum FontData {
    Static(&'static [u8]),
    Boxed(Box<[u8], ErasedAllocator>),
}

impl FontData {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Static(slice) => slice,
            Self::Boxed(boo) => boo.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontHandle(Handle<RefCell<Font>>);

#[derive(Clone, Copy)]
struct GlyphTextureInfo {
    texture_page_idx: usize,
    _texture_packer_entry_idx: usize,
    texture_coords: Rect,
}

#[derive(Clone, Copy)]
struct Glyph {
    index_within_font: i32,
    texture_info: GlyphTextureInfo,
}

#[derive(Clone, Copy)]
pub struct ScaledGlyph<'a> {
    font: &'a RefCell<Font>,
    index_within_font: i32,
    texture_info: GlyphTextureInfo,
    pub metrics: GlyphMetrics,
}

struct Font {
    self_handle: FontHandle,
    data: FontData,
    // NOTE: each time you pass stbtt_fontinfo to stb you MUST "refresh" its
    // `userdata` (allocator context) pointers. :RefreshUserdata
    //
    //   stb does not store it anywhere (from what i gathered).
    //   none of the stbtt functions except init mutate stbtt_fontinfo.
    //
    //   stbtt_InitFont puts font_data_ptr into stbtt_fontinfo struct. :FontDataPtrIsStable
    //   you do not need to "refresh" it's `data` pointer because that is stable (static or boxed).
    info: stbtt_fontinfo,
    glyphs: hash_map::HashMap<char, Glyph, FxBuildHasher>,
}

#[derive(Clone, Copy)]
pub struct ScaledFont<'a> {
    font: &'a RefCell<Font>,
    size_px: f32,
    scale: f32,
    pub metrics: FontMetrics,

    texture_pages: &'a RefCell<ResizableArray<TexturePage, ErasedAllocator>>,
}

impl<'a> ScaledFont<'a> {
    pub fn get_glyph(
        &self,
        c: char,
        temp: &TempAllocator<'_>,
        texture_service: &mut TextureService,
    ) -> Option<ScaledGlyph<'a>> {
        let (mut info, mut glyphs) = RefMut::map_split(self.font.borrow_mut(), |borrow| {
            (&mut borrow.info, &mut borrow.glyphs)
        });

        let entry = glyphs.entry(c);
        let (index_within_font, texture_info) = match entry {
            hash_map::Entry::Occupied(occupied) => {
                let glyph = occupied.get();
                (glyph.index_within_font, glyph.texture_info)
            }
            hash_map::Entry::Vacant(vacant) => {
                // TODO: index is 0 if char is not found; do i care?
                let index_within_font = unsafe { stbtt_FindGlyphIndex(info.deref(), c as i32) };
                assert!(index_within_font >= 0);

                let start = std::time::Instant::now();
                let sdf = compute_glyph_sdf(info.deref_mut(), index_within_font, temp)?;
                log::debug!("rasterized glyph for '{c}' in {:?}", start.elapsed());
                assert!(sdf.width > 0);
                assert!(sdf.height > 0);

                // NOTE: try inserting into existing pages
                let mut maybe_page_idx_and_packer_entry_idx: Option<(usize, usize)> = None;
                let mut texture_pages = self.texture_pages.borrow_mut();
                for (page_idx, texture_page) in texture_pages.iter_mut().enumerate() {
                    if let Some(packer_entry_idx) = texture_page
                        .texture_packer
                        .insert(sdf.width as u32, sdf.height as u32)
                    {
                        maybe_page_idx_and_packer_entry_idx = Some((page_idx, packer_entry_idx));
                    }
                }

                let (page_idx, packer_entry_idx) = match maybe_page_idx_and_packer_entry_idx {
                    Some(yep) => yep,
                    None => {
                        let mut texture_packer =
                            TexturePacker::new(TEXTURE_WIDTH, TEXTURE_HEIGHT, TEXTURE_GAP);
                        let texture_handle = texture_service.create(TextureDesc {
                            format: TextureFormat::R8Unorm,
                            w: TEXTURE_WIDTH,
                            h: TEXTURE_HEIGHT,
                        });
                        // NOTE: this unwrap is somewhat redundant because there's an assertion above that
                        // ensures that char size is <= texture size.
                        let packer_entry_idx = texture_packer
                            .insert(sdf.width as u32, sdf.height as u32)
                            .unwrap();
                        let page_idx = texture_pages.len();
                        texture_pages.push(TexturePage {
                            texture_packer,
                            texture_handle,
                        });
                        (page_idx, packer_entry_idx)
                    }
                };

                let texture_page = &mut texture_pages[page_idx];
                let texture_packer_entry = texture_page.texture_packer.get_entry(packer_entry_idx);

                let upload_buf = texture_service.get_upload_buf(
                    texture_page.texture_handle,
                    TextureRegion {
                        x: texture_packer_entry.x,
                        y: texture_packer_entry.y,
                        w: texture_packer_entry.w,
                        h: texture_packer_entry.h,
                    },
                );
                upload_buf.copy_from_slice(sdf.pixels);

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
                let texture_info = GlyphTextureInfo {
                    texture_page_idx: page_idx,
                    _texture_packer_entry_idx: packer_entry_idx,
                    texture_coords,
                };

                vacant.insert(Glyph {
                    index_within_font,
                    texture_info,
                });

                (index_within_font, texture_info)
            }
        };

        let metrics = get_glyph_metrics(info.deref(), index_within_font, self.size_px, self.scale);
        Some(ScaledGlyph {
            font: self.font,
            index_within_font,
            texture_info,
            metrics,
        })
    }
}

const INITIAL_GLYPH_COUNT: usize = 255;

pub struct FontService {
    // NOTE: font is ref-cell'ed because i want to be able to have multiple scaled instances of the
    // same font at a time.
    fonts: HandleArray<RefCell<Font>, ErasedAllocator>,
    texture_pages: RefCell<ResizableArray<TexturePage, ErasedAllocator>>,
    alloc: ErasedAllocator,
}

impl FontService {
    pub fn new_in(alloc: ErasedAllocator) -> Self {
        Self {
            fonts: HandleArray::new_in(alloc),
            texture_pages: RefCell::new(ResizableArray::new_in(alloc)),
            alloc,
        }
    }

    pub fn register_font(&mut self, data: FontData) -> anyhow::Result<FontHandle> {
        let info = unsafe {
            let mut info = stbtt_fontinfo {
                userdata: &self.alloc as *const _ as _,
                ..mem::zeroed()
            };
            // :FontDataPtrIsStable
            let font_data_ptr = data.as_bytes().as_ptr();
            let offset = stbtt_GetFontOffsetForIndex(
                font_data_ptr,
                // NOTE: index probably bears a meaning for some kind of font collections or something,
                // but here i am dealing with ttf/otf fonts.
                0,
            );
            let ok = stbtt_InitFont(&mut info, font_data_ptr, offset);
            if ok == 0 {
                return Err(anyhow!("could not init font"));
            }
            info
        };
        let glyphs = hash_map::HashMap::with_capacity_and_hasher(
            INITIAL_GLYPH_COUNT,
            FxBuildHasher::default(),
        );
        let handle = self.fonts.push_with(|self_handle| {
            RefCell::new(Font {
                self_handle: FontHandle(self_handle),
                data,
                info,
                glyphs,
            })
        });
        Ok(FontHandle(handle))
    }

    pub fn get_font<'a>(&'a self, handle: FontHandle, size_pt: f32) -> ScaledFont<'a> {
        let size_px = size_pt * POINTS_TO_PIXELS;

        let font = self.fonts.get(handle.0);
        let mut borrow = font.borrow_mut();
        let info = &mut borrow.info;
        // :RefreshUserdata, :FontDataPtrIsStable
        info.userdata = &self.alloc as *const _ as _;
        let scale = unsafe { stbtt_ScaleForMappingEmToPixels(info, size_px) };
        let metrics = get_font_metrics(&info, scale);
        ScaledFont {
            font,
            size_px,
            scale,
            metrics,

            texture_pages: &self.texture_pages,
        }
    }
}
