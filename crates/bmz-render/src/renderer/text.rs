use super::*;

#[derive(Clone)]
pub(super) struct FontFallbackFace {
    pub(super) cache_id: String,
    pub(super) font: FontArc,
}

#[derive(Clone, Default)]
pub(super) struct FontFallbackChain {
    pub(super) faces: Vec<FontFallbackFace>,
}

impl FontFallbackChain {
    pub(super) fn primary(&self) -> Option<&FontFallbackFace> {
        self.faces.first()
    }

    pub(super) fn select(&self, ch: char) -> Option<&FontFallbackFace> {
        self.faces.iter().find(|face| face.font.glyph_id(ch).0 != 0).or_else(|| self.primary())
    }

    #[cfg(test)]
    fn single(font: FontArc) -> Self {
        Self { faces: vec![FontFallbackFace { cache_id: DEFAULT_TEXT_FONT_ID.to_string(), font }] }
    }
}

#[derive(Clone, Copy)]
pub(super) enum VectorFontSet<'a> {
    Single { cache_id: &'a str, font: &'a FontArc },
    Fallback(&'a FontFallbackChain),
}

impl<'a> VectorFontSet<'a> {
    fn primary(self) -> Option<(&'a str, &'a FontArc)> {
        match self {
            Self::Single { cache_id, font } => Some((cache_id, font)),
            Self::Fallback(fonts) => {
                fonts.primary().map(|face| (face.cache_id.as_str(), &face.font))
            }
        }
    }

    fn select(self, ch: char) -> Option<(&'a str, &'a FontArc)> {
        match self {
            Self::Single { cache_id, font } => Some((cache_id, font)),
            Self::Fallback(fonts) => {
                fonts.select(ch).map(|face| (face.cache_id.as_str(), &face.font))
            }
        }
    }

    fn layout_cache_id(self, text: &str) -> String {
        match self {
            Self::Single { cache_id, .. } => cache_id.to_string(),
            Self::Fallback(_) => {
                let mut id = String::from(DEFAULT_TEXT_FONT_ID);
                for ch in text.chars() {
                    let selected = self.select(ch).map(|(id, _)| id).unwrap_or("<missing>");
                    id.push('|');
                    id.push_str(selected);
                }
                id
            }
        }
    }
}

pub(super) fn build_text_frame_with_fallback_cache(
    plan: &DrawPlan,
    default_fonts: &FontFallbackChain,
    fonts: &HashMap<String, FontArc>,
    bitmap_fonts: &HashMap<String, BitmapFont>,
    surface: SurfaceSize,
    atlas: &mut TextAtlasCache,
) -> TextFrame {
    if !surface.is_drawable() {
        return TextFrame::default();
    }

    atlas.begin_frame();
    let mut builder = CachedTextFrameBuilder::new(atlas);
    let mut command_quad_counts = Vec::new();
    let mut command_caret_rects = Vec::new();
    for command in &plan.commands {
        let DrawCommand::Text { origin, text, style, caret } = command else {
            continue;
        };
        let quads_before = builder.quads.len();
        if let Some(font_id) = style.font_id.as_deref()
            && let Some(bitmap_font) = bitmap_fonts.get(font_id)
        {
            builder.push_bitmap_text(origin, text, style.clone(), font_id, bitmap_font, surface);
            command_caret_rects.push(caret.and_then(|caret| {
                bitmap_text_caret_rect(origin, text, style, bitmap_font, surface, caret)
            }));
        } else {
            let vector_fonts = style
                .font_id
                .as_deref()
                .and_then(|font_id| {
                    fonts.get(font_id).map(|font| VectorFontSet::Single { cache_id: font_id, font })
                })
                .unwrap_or(VectorFontSet::Fallback(default_fonts));
            builder.push_text(origin, text, style.clone(), vector_fonts, surface);
            command_caret_rects.push(caret.and_then(|caret| {
                vector_text_caret_rect_with_fallback(
                    origin,
                    text,
                    style,
                    vector_fonts,
                    surface,
                    caret,
                )
            }));
        }
        command_quad_counts.push(builder.quads.len() - quads_before);
    }
    builder.finish(command_quad_counts, command_caret_rects)
}

#[cfg(test)]
pub(super) fn build_text_frame_with_cache(
    plan: &DrawPlan,
    default_font: &FontArc,
    fonts: &HashMap<String, FontArc>,
    bitmap_fonts: &HashMap<String, BitmapFont>,
    surface: SurfaceSize,
    atlas: &mut TextAtlasCache,
) -> TextFrame {
    build_text_frame_with_fallback_cache(
        plan,
        &FontFallbackChain::single(default_font.clone()),
        fonts,
        bitmap_fonts,
        surface,
        atlas,
    )
}

pub(super) const DEFAULT_TEXT_FONT_ID: &str = "<default>";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextGlyphKey {
    kind: TextGlyphKind,
    pub(super) font_id: String,
    pub(super) ch: char,
    scale_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum TextGlyphKind {
    Vector,
    Bitmap,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextLayoutKey {
    kind: TextGlyphKind,
    pub(super) font_id: String,
    pub(super) text: String,
    origin_x_bits: u32,
    origin_y_bits: u32,
    surface_width: u32,
    surface_height: u32,
    style: TextLayoutStyleKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TextLayoutStyleKey {
    size_bits: u32,
    bitmap_size_bits: Option<u32>,
    color: ColorKey,
    align: u8,
    max_width_bits: u32,
    overflow: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ColorKey {
    r_bits: u32,
    g_bits: u32,
    b_bits: u32,
    a_bits: u32,
}

#[derive(Debug, Clone)]
pub(super) struct CachedGlyph {
    atlas_origin: (u32, u32),
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) display_width: f32,
    pub(super) display_height: f32,
    offset_x: f32,
    offset_y: f32,
}

#[derive(Debug)]
pub(super) struct TextLayoutCache {
    pub(super) entries: HashMap<TextLayoutKey, Vec<TextQuad>>,
}

impl TextLayoutCache {
    fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn cached(&self, key: &TextLayoutKey) -> Option<Vec<TextQuad>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: TextLayoutKey, quads: &[TextQuad]) {
        if self.entries.len() >= TEXT_LAYOUT_CACHE_MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert(key, quads.to_vec());
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug)]
pub(super) struct TextAtlasCache {
    width: u32,
    pub(super) pen_x: u32,
    pub(super) pen_y: u32,
    row_height: u32,
    pixels: Vec<u8>,
    pub(super) glyphs: HashMap<TextGlyphKey, CachedGlyph>,
    pub(super) layouts: TextLayoutCache,
    dirty_regions: Vec<TextAtlasDirtyRegion>,
}

impl TextAtlasCache {
    pub(super) fn new(width: u32) -> Self {
        Self {
            width,
            pen_x: 0,
            pen_y: 0,
            row_height: 0,
            pixels: Vec::new(),
            glyphs: HashMap::new(),
            layouts: TextLayoutCache::new(),
            dirty_regions: Vec::new(),
        }
    }

    pub(super) fn begin_frame(&mut self) {
        self.dirty_regions.clear();
        // アトラス高さが上限に達したらキャッシュを捨てて作り直す。フレーム描画前に
        // 行うので、このフレームのグリフは新しいアトラスへ再ラスタライズされる。
        if self.atlas_height() >= TEXT_ATLAS_MAX_HEIGHT {
            self.clear();
        }
    }

    fn clear(&mut self) {
        self.pen_x = 0;
        self.pen_y = 0;
        self.row_height = 0;
        self.pixels.clear();
        self.glyphs.clear();
        self.layouts.clear();
        self.dirty_regions.clear();
    }

    pub(super) fn atlas_height(&self) -> u32 {
        (self.pen_y + self.row_height).max(1)
    }

    fn size(&self) -> AtlasSize {
        AtlasSize { width: self.width, height: self.atlas_height() }
    }

    pub(super) fn pixels_for_size(&self, size: AtlasSize) -> Vec<u8> {
        let mut pixels = self.pixels.clone();
        pixels.resize((size.width * size.height * 4) as usize, 0);
        pixels
    }

    fn cached_layout(&self, key: &TextLayoutKey) -> Option<Vec<TextQuad>> {
        self.layouts.cached(key)
    }

    fn insert_layout(&mut self, key: TextLayoutKey, quads: &[TextQuad]) {
        self.layouts.insert(key, quads);
    }

    pub(super) fn cached_vector_glyph(
        &mut self,
        font_id: &str,
        ch: char,
        scale: PxScale,
        font: &FontArc,
    ) -> Option<CachedGlyph> {
        let key = TextGlyphKey {
            kind: TextGlyphKind::Vector,
            font_id: font_id.to_string(),
            ch,
            scale_bits: scale.x.to_bits(),
        };
        if let Some(glyph) = self.glyphs.get(&key) {
            return Some(glyph.clone());
        }

        let raster_scale = PxScale {
            x: scale.x * VECTOR_TEXT_SUPERSAMPLE_SCALE,
            y: scale.y * VECTOR_TEXT_SUPERSAMPLE_SCALE,
        };
        let scaled_font = font.as_scaled(raster_scale);
        let baseline_y = scaled_font.ascent();
        let glyph =
            Glyph { id: font.glyph_id(ch), scale: raster_scale, position: point(0.0, baseline_y) };
        let outlined = font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();
        let width = bounds.width().ceil().max(0.0) as u32;
        let height = bounds.height().ceil().max(0.0) as u32;
        if width == 0 || height == 0 {
            return None;
        }

        let mut pixels = vec![0; (width * height * 4) as usize];
        outlined.draw(|x, y, coverage| {
            let index = ((y * width + x) * 4) as usize;
            let coverage_u8 = (coverage * 255.0).clamp(0.0, 255.0) as u8;
            if let Some(pixel) = pixels.get_mut(index..index + 4)
                && coverage_u8 >= pixel[3]
            {
                pixel[0] = 255;
                pixel[1] = 255;
                pixel[2] = 255;
                pixel[3] = coverage_u8;
            }
        });

        Some(self.insert_glyph_pixels(
            key,
            width,
            height,
            width as f32 / VECTOR_TEXT_SUPERSAMPLE_SCALE,
            height as f32 / VECTOR_TEXT_SUPERSAMPLE_SCALE,
            bounds.min.x / VECTOR_TEXT_SUPERSAMPLE_SCALE,
            (bounds.min.y - baseline_y) / VECTOR_TEXT_SUPERSAMPLE_SCALE,
            pixels,
        ))
    }

    fn cached_bitmap_glyph(
        &mut self,
        font_id: &str,
        ch: char,
        glyph: crate::bitmap_font::BitmapFontGlyph,
        page: &crate::bitmap_font::BitmapFontPage,
        font: &BitmapFont,
        scale: f32,
    ) -> CachedGlyph {
        let key = TextGlyphKey {
            kind: TextGlyphKind::Bitmap,
            font_id: font_id.to_string(),
            ch,
            scale_bits: scale.to_bits(),
        };
        if let Some(glyph) = self.glyphs.get(&key) {
            return glyph.clone();
        }

        let width = (glyph.width as f32 * scale).ceil().max(1.0) as u32;
        let height = (glyph.height as f32 * scale).ceil().max(1.0) as u32;
        let pixels = rasterized_bitmap_glyph_pixels(glyph, page, scale, width, height);

        self.insert_glyph_pixels(
            key,
            width,
            height,
            width as f32,
            height as f32,
            glyph.xoffset as f32 * scale,
            (glyph.yoffset as f32 - font.ascent) * scale,
            pixels,
        )
    }

    fn insert_glyph_pixels(
        &mut self,
        key: TextGlyphKey,
        width: u32,
        height: u32,
        display_width: f32,
        display_height: f32,
        offset_x: f32,
        offset_y: f32,
        pixels: Vec<u8>,
    ) -> CachedGlyph {
        let atlas_origin = self.reserve(width, height);
        self.blit_region(atlas_origin, width, height, &pixels);
        self.dirty_regions.push(TextAtlasDirtyRegion {
            origin: atlas_origin,
            size: AtlasSize { width, height },
            pixels,
        });
        let cached = CachedGlyph {
            atlas_origin,
            width,
            height,
            display_width,
            display_height,
            offset_x,
            offset_y,
        };
        self.glyphs.insert(key, cached.clone());
        cached
    }

    pub(super) fn reserve(&mut self, glyph_width: u32, glyph_height: u32) -> (u32, u32) {
        let padded_width = glyph_width + TEXT_ATLAS_PADDING * 2;
        let padded_height = glyph_height + TEXT_ATLAS_PADDING * 2;
        if self.pen_x + padded_width > self.width {
            self.pen_x = 0;
            self.pen_y += self.row_height;
            self.row_height = 0;
        }

        let origin = (self.pen_x + TEXT_ATLAS_PADDING, self.pen_y + TEXT_ATLAS_PADDING);
        self.pen_x += padded_width;
        self.row_height = self.row_height.max(padded_height);
        self.ensure_height(self.pen_y + self.row_height);
        origin
    }

    fn ensure_height(&mut self, height: u32) {
        let needed = (self.width * height * 4) as usize;
        if self.pixels.len() < needed {
            self.pixels.resize(needed, 0);
        }
    }

    fn blit_region(&mut self, atlas_origin: (u32, u32), width: u32, height: u32, pixels: &[u8]) {
        for y in 0..height {
            let dst_start = (((atlas_origin.1 + y) * self.width + atlas_origin.0) * 4) as usize;
            let src_start = (y * width * 4) as usize;
            let len = (width * 4) as usize;
            if let (Some(dst), Some(src)) = (
                self.pixels.get_mut(dst_start..dst_start + len),
                pixels.get(src_start..src_start + len),
            ) {
                dst.copy_from_slice(src);
            }
        }
    }
}

pub(super) fn text_layout_key(
    kind: TextGlyphKind,
    font_id: &str,
    origin: &Point,
    text: &str,
    style: &TextStyle,
    surface: SurfaceSize,
) -> Option<TextLayoutKey> {
    if style.wrapping || style.outline.is_some() || style.shadow.is_some() {
        return None;
    }
    Some(TextLayoutKey {
        kind,
        font_id: font_id.to_string(),
        text: text.to_string(),
        origin_x_bits: origin.x.to_bits(),
        origin_y_bits: origin.y.to_bits(),
        surface_width: surface.width,
        surface_height: surface.height,
        style: TextLayoutStyleKey {
            size_bits: style.size.to_bits(),
            bitmap_size_bits: style.bitmap_size.map(f32::to_bits),
            color: ColorKey {
                r_bits: style.color.r.to_bits(),
                g_bits: style.color.g.to_bits(),
                b_bits: style.color.b.to_bits(),
                a_bits: style.color.a.to_bits(),
            },
            align: text_align_key(style.align),
            max_width_bits: style.max_width.to_bits(),
            overflow: text_overflow_key(style.overflow),
        },
    })
}

pub(super) fn text_align_key(align: TextAlign) -> u8 {
    match align {
        TextAlign::Left => 0,
        TextAlign::Center => 1,
        TextAlign::Right => 2,
    }
}

pub(super) fn text_overflow_key(overflow: TextOverflow) -> u8 {
    match overflow {
        TextOverflow::Overflow => 0,
        TextOverflow::Shrink => 1,
        TextOverflow::Truncate => 2,
    }
}

pub(super) struct CachedTextFrameBuilder<'a> {
    atlas: &'a mut TextAtlasCache,
    quads: Vec<TextQuad>,
}

impl<'a> CachedTextFrameBuilder<'a> {
    fn new(atlas: &'a mut TextAtlasCache) -> Self {
        Self { atlas, quads: Vec::new() }
    }

    fn push_bitmap_text(
        &mut self,
        origin: &Point,
        text: &str,
        style: TextStyle,
        font_id: &str,
        font: &BitmapFont,
        surface: SurfaceSize,
    ) {
        if let Some(shadow) = style.shadow.filter(|shadow| shadow.color.a > 0.0) {
            let mut shadow_style = style.clone();
            shadow_style.color = shadow.color;
            shadow_style.outline = None;
            shadow_style.shadow = None;
            let shadow_origin =
                Point { x: origin.x + shadow.offset.x, y: origin.y + shadow.offset.y };
            self.push_bitmap_text(&shadow_origin, text, shadow_style, font_id, font, surface);
        }
        if let Some(outline) = style.outline.filter(|outline| outline.color.a > 0.0) {
            let mut outline_style = style.clone();
            outline_style.color = outline.color;
            outline_style.outline = None;
            outline_style.shadow = None;
            let outline_y = outline.width;
            let outline_x = outline.width * surface.height as f32 / surface.width as f32;
            for (offset_x, offset_y) in [
                (-outline_x, -outline_y),
                (0.0, -outline_y),
                (outline_x, -outline_y),
                (-outline_x, 0.0),
                (outline_x, 0.0),
                (-outline_x, outline_y),
                (0.0, outline_y),
                (outline_x, outline_y),
            ] {
                let outline_origin = Point { x: origin.x + offset_x, y: origin.y + offset_y };
                self.push_bitmap_text(
                    &outline_origin,
                    text,
                    outline_style.clone(),
                    font_id,
                    font,
                    surface,
                );
            }
        }

        let layout_key =
            text_layout_key(TextGlyphKind::Bitmap, font_id, origin, text, &style, surface);
        if let Some(cached) = layout_key.as_ref().and_then(|key| self.atlas.cached_layout(key)) {
            self.quads.extend(cached);
            return;
        }
        let quads_before = self.quads.len();

        let design_size = if font.size > 0 { font.size } else { font.line_height.max(1) };
        let bitmap_size = style.bitmap_size.unwrap_or(style.size);
        let mut scale = (bitmap_size * surface.height as f32 / design_size as f32).max(0.01);
        let original_scale = scale;
        let mut text_width = bitmap_text_width_px(text, font, scale);
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let text = if max_width > 0.0 && text_width > max_width {
            match style.overflow {
                TextOverflow::Overflow => std::borrow::Cow::Borrowed(text),
                TextOverflow::Shrink => {
                    scale = (scale * max_width / text_width).max(0.01);
                    std::borrow::Cow::Borrowed(text)
                }
                TextOverflow::Truncate => std::borrow::Cow::Owned(truncate_bitmap_text_to_width(
                    text, font, max_width, scale,
                )),
            }
        } else {
            std::borrow::Cow::Borrowed(text)
        };
        text_width = bitmap_text_width_px(&text, font, scale);
        let align_offset = text_align_offset_px(style.align, max_width, text_width);
        let mut cursor_x = origin.x * surface.width as f32 + align_offset;
        let shrink_offset_y =
            if matches!(style.overflow, TextOverflow::Shrink) && scale < original_scale {
                (design_size as f32 * (original_scale - scale)) / 2.0
            } else {
                0.0
            };
        let text_top_y = origin.y * surface.height as f32 + shrink_offset_y;

        for ch in text.chars() {
            let Some(glyph) = font.glyphs.get(&ch) else {
                continue;
            };
            if glyph.width > 0
                && glyph.height > 0
                && let Some(page) = font.pages.get(&glyph.page)
            {
                let cached = self.atlas.cached_bitmap_glyph(font_id, ch, *glyph, page, font, scale);
                self.quads.push(TextQuad {
                    x: (cursor_x + cached.offset_x) / surface.width as f32,
                    y: (text_top_y + cached.offset_y) / surface.height as f32,
                    width: cached.display_width / surface.width as f32,
                    height: cached.display_height / surface.height as f32,
                    atlas_origin: cached.atlas_origin,
                    glyph_width: cached.width,
                    glyph_height: cached.height,
                    color: style.color,
                });
            }
            cursor_x += glyph.xadvance as f32 * scale;
        }

        if let Some(key) = layout_key {
            self.atlas.insert_layout(key, &self.quads[quads_before..]);
        }
    }

    fn push_text(
        &mut self,
        origin: &Point,
        text: &str,
        style: TextStyle,
        fonts: VectorFontSet<'_>,
        surface: SurfaceSize,
    ) {
        if let Some(shadow) = style.shadow.filter(|shadow| shadow.color.a > 0.0) {
            let mut shadow_style = style.clone();
            shadow_style.color = shadow.color;
            shadow_style.outline = None;
            shadow_style.shadow = None;
            let shadow_origin =
                Point { x: origin.x + shadow.offset.x, y: origin.y + shadow.offset.y };
            self.push_text(&shadow_origin, text, shadow_style, fonts, surface);
        }
        if let Some(outline) = style.outline.filter(|outline| outline.color.a > 0.0) {
            let mut outline_style = style.clone();
            outline_style.color = outline.color;
            outline_style.outline = None;
            outline_style.shadow = None;
            let outline_y = outline.width;
            let outline_x = outline.width * surface.height as f32 / surface.width as f32;
            for (offset_x, offset_y) in [
                (-outline_x, -outline_y),
                (0.0, -outline_y),
                (outline_x, -outline_y),
                (-outline_x, 0.0),
                (outline_x, 0.0),
                (-outline_x, outline_y),
                (0.0, outline_y),
                (outline_x, outline_y),
            ] {
                let outline_origin = Point { x: origin.x + offset_x, y: origin.y + offset_y };
                self.push_text(&outline_origin, text, outline_style.clone(), fonts, surface);
            }
        }

        let layout_font_id = fonts.layout_cache_id(text);
        let layout_key =
            text_layout_key(TextGlyphKind::Vector, &layout_font_id, origin, text, &style, surface);
        if let Some(cached) = layout_key.as_ref().and_then(|key| self.atlas.cached_layout(key)) {
            self.quads.extend(cached);
            return;
        }
        let quads_before = self.quads.len();

        let mut px_size = (style.size * surface.height as f32).max(1.0);
        let original_px_size = px_size;
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let mut text = std::borrow::Cow::Borrowed(text);
        let mut scale = PxScale::from(px_size);
        let Some((_, primary_font)) = fonts.primary() else {
            return;
        };
        let mut scaled_primary = primary_font.as_scaled(scale);
        let mut text_width = fallback_text_width_px(&text, fonts, scale);
        if style.wrapping && max_width > 0.0 {
            let lines = wrap_fallback_text_to_width(&text, fonts, scale, max_width);
            let line_height = (scaled_primary.ascent() - scaled_primary.descent()
                + scaled_primary.line_gap())
            .max(px_size);
            let origin_x = origin.x * surface.width as f32;
            let first_baseline_y = origin.y * surface.height as f32 + scaled_primary.ascent();
            for (index, line) in lines.iter().enumerate() {
                let line_width = fallback_text_width_px(line, fonts, scale);
                let baseline_y = first_baseline_y + line_height * index as f32;
                self.push_text_line(
                    origin_x,
                    baseline_y,
                    line,
                    line_width,
                    scale,
                    style.clone(),
                    fonts,
                    surface,
                );
            }
            return;
        }
        if max_width > 0.0 && text_width > max_width {
            match style.overflow {
                TextOverflow::Overflow => {}
                TextOverflow::Shrink => {
                    px_size = (px_size * max_width / text_width).max(1.0);
                    scale = PxScale::from(px_size);
                    scaled_primary = primary_font.as_scaled(scale);
                    text_width = fallback_text_width_px(&text, fonts, scale);
                }
                TextOverflow::Truncate => {
                    text = std::borrow::Cow::Owned(truncate_fallback_text_to_width(
                        &text, fonts, scale, max_width,
                    ));
                    text_width = fallback_text_width_px(&text, fonts, scale);
                }
            }
        }
        let cursor_x = origin.x * surface.width as f32;
        let shrink_offset_y =
            if matches!(style.overflow, TextOverflow::Shrink) && px_size < original_px_size {
                (original_px_size - px_size) / 2.0
            } else {
                0.0
            };
        let baseline_y =
            origin.y * surface.height as f32 + shrink_offset_y + scaled_primary.ascent();

        self.push_text_line(cursor_x, baseline_y, &text, text_width, scale, style, fonts, surface);

        if let Some(key) = layout_key {
            self.atlas.insert_layout(key, &self.quads[quads_before..]);
        }
    }

    fn push_text_line(
        &mut self,
        origin_x: f32,
        baseline_y: f32,
        text: &str,
        text_width: f32,
        scale: PxScale,
        style: TextStyle,
        fonts: VectorFontSet<'_>,
        surface: SurfaceSize,
    ) {
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let align_offset = text_align_offset_px(style.align, max_width, text_width);
        let mut cursor_x = origin_x + align_offset;

        for ch in text.chars() {
            let Some((font_id, font)) = fonts.select(ch) else {
                continue;
            };
            let scaled_font = font.as_scaled(scale);
            let glyph_id = font.glyph_id(ch);
            let advance = scaled_font.h_advance(glyph_id);
            if let Some(cached) = self.atlas.cached_vector_glyph(font_id, ch, scale, font) {
                self.quads.push(TextQuad {
                    x: (cursor_x + cached.offset_x) / surface.width as f32,
                    y: (baseline_y + cached.offset_y) / surface.height as f32,
                    width: cached.display_width / surface.width as f32,
                    height: cached.display_height / surface.height as f32,
                    atlas_origin: cached.atlas_origin,
                    glyph_width: cached.width,
                    glyph_height: cached.height,
                    color: style.color,
                });
            }
            cursor_x += advance;
        }
    }

    fn finish(
        self,
        command_quad_counts: Vec<usize>,
        command_caret_rects: Vec<Option<RectCommand>>,
    ) -> TextFrame {
        let size = self.atlas.size();
        let instances = encode_text_quads(&self.quads, size.width, size.height);
        TextFrame {
            size,
            #[cfg(test)]
            pixels: Vec::new(),
            dirty_regions: std::mem::take(&mut self.atlas.dirty_regions),
            instances,
            command_quad_counts,
            command_caret_rects,
        }
    }
}

#[cfg(test)]
pub(super) struct TextAtlasBuilder {
    width: u32,
    pen_x: u32,
    pen_y: u32,
    row_height: u32,
    pixels: Vec<u8>,
    pub(super) quads: Vec<TextQuad>,
}

#[derive(Debug, Clone)]
pub(super) struct TextQuad {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    atlas_origin: (u32, u32),
    glyph_width: u32,
    glyph_height: u32,
    color: Color,
}

#[cfg(test)]
impl TextAtlasBuilder {
    pub(super) fn new(width: u32) -> Self {
        Self { width, pen_x: 0, pen_y: 0, row_height: 0, pixels: Vec::new(), quads: Vec::new() }
    }

    pub(super) fn push_bitmap_text(
        &mut self,
        origin: &Point,
        text: &str,
        style: TextStyle,
        font: &BitmapFont,
        surface: SurfaceSize,
    ) {
        if let Some(shadow) = style.shadow.filter(|shadow| shadow.color.a > 0.0) {
            let mut shadow_style = style.clone();
            shadow_style.color = shadow.color;
            shadow_style.outline = None;
            shadow_style.shadow = None;
            let shadow_origin =
                Point { x: origin.x + shadow.offset.x, y: origin.y + shadow.offset.y };
            self.push_bitmap_text(&shadow_origin, text, shadow_style, font, surface);
        }
        if let Some(outline) = style.outline.filter(|outline| outline.color.a > 0.0) {
            let mut outline_style = style.clone();
            outline_style.color = outline.color;
            outline_style.outline = None;
            outline_style.shadow = None;
            let outline_y = outline.width;
            let outline_x = outline.width * surface.height as f32 / surface.width as f32;
            for (offset_x, offset_y) in [
                (-outline_x, -outline_y),
                (0.0, -outline_y),
                (outline_x, -outline_y),
                (-outline_x, 0.0),
                (outline_x, 0.0),
                (-outline_x, outline_y),
                (0.0, outline_y),
                (outline_x, outline_y),
            ] {
                let outline_origin = Point { x: origin.x + offset_x, y: origin.y + offset_y };
                self.push_bitmap_text(&outline_origin, text, outline_style.clone(), font, surface);
            }
        }

        let design_size = if font.size > 0 { font.size } else { font.line_height.max(1) };
        let bitmap_size = style.bitmap_size.unwrap_or(style.size);
        let mut scale = (bitmap_size * surface.height as f32 / design_size as f32).max(0.01);
        let original_scale = scale;
        let mut text_width = bitmap_text_width_px(text, font, scale);
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let text = if max_width > 0.0 && text_width > max_width {
            match style.overflow {
                TextOverflow::Overflow => std::borrow::Cow::Borrowed(text),
                TextOverflow::Shrink => {
                    scale = (scale * max_width / text_width).max(0.01);
                    std::borrow::Cow::Borrowed(text)
                }
                TextOverflow::Truncate => std::borrow::Cow::Owned(truncate_bitmap_text_to_width(
                    text, font, max_width, scale,
                )),
            }
        } else {
            std::borrow::Cow::Borrowed(text)
        };
        text_width = bitmap_text_width_px(&text, font, scale);
        let align_offset = text_align_offset_px(style.align, max_width, text_width);
        let mut cursor_x = origin.x * surface.width as f32 + align_offset;
        let shrink_offset_y =
            if matches!(style.overflow, TextOverflow::Shrink) && scale < original_scale {
                (design_size as f32 * (original_scale - scale)) / 2.0
            } else {
                0.0
            };
        let text_top_y = origin.y * surface.height as f32 + shrink_offset_y;

        for ch in text.chars() {
            let Some(glyph) = font.glyphs.get(&ch) else {
                continue;
            };
            if glyph.width > 0 && glyph.height > 0 {
                let Some(page) = font.pages.get(&glyph.page) else {
                    cursor_x += glyph.xadvance as f32 * scale;
                    continue;
                };
                let glyph_width = (glyph.width as f32 * scale).ceil().max(1.0) as u32;
                let glyph_height = (glyph.height as f32 * scale).ceil().max(1.0) as u32;
                let atlas_origin = self.reserve(glyph_width, glyph_height);
                self.blit_bitmap_glyph(
                    atlas_origin,
                    glyph_width,
                    glyph_height,
                    *glyph,
                    page,
                    scale,
                );
                let x = (cursor_x + glyph.xoffset as f32 * scale) / surface.width as f32;
                let y = (text_top_y + (glyph.yoffset as f32 - font.ascent) * scale)
                    / surface.height as f32;
                self.quads.push(TextQuad {
                    x,
                    y,
                    width: glyph_width as f32 / surface.width as f32,
                    height: glyph_height as f32 / surface.height as f32,
                    atlas_origin,
                    glyph_width,
                    glyph_height,
                    color: style.color,
                });
            }
            cursor_x += glyph.xadvance as f32 * scale;
        }
    }

    fn blit_bitmap_glyph(
        &mut self,
        atlas_origin: (u32, u32),
        glyph_width: u32,
        glyph_height: u32,
        glyph: crate::bitmap_font::BitmapFontGlyph,
        page: &crate::bitmap_font::BitmapFontPage,
        scale: f32,
    ) {
        let pixels = rasterized_bitmap_glyph_pixels(glyph, page, scale, glyph_width, glyph_height);
        for dst_y in 0..glyph_height {
            for dst_x in 0..glyph_width {
                let src_index = ((dst_y * glyph_width + dst_x) * 4) as usize;
                let Some(src) = pixels.get(src_index..src_index + 4) else {
                    continue;
                };
                let dst_index =
                    (((atlas_origin.1 + dst_y) * self.width + atlas_origin.0 + dst_x) * 4) as usize;
                let Some(dst) = self.pixels.get_mut(dst_index..dst_index + 4) else {
                    continue;
                };
                // ビットマップフォントは RGBA を保持して描画したい (色付きグリフ対応)。
                // 同じアトラス位置に重ねたい場合は alpha が大きい方を採用。
                if src[3] >= dst[3] {
                    dst.copy_from_slice(src);
                }
            }
        }
    }

    pub(super) fn push_text(
        &mut self,
        origin: &Point,
        text: &str,
        style: TextStyle,
        font: &FontArc,
        surface: SurfaceSize,
    ) {
        if let Some(shadow) = style.shadow.filter(|shadow| shadow.color.a > 0.0) {
            let mut shadow_style = style.clone();
            shadow_style.color = shadow.color;
            shadow_style.outline = None;
            shadow_style.shadow = None;
            let shadow_origin =
                Point { x: origin.x + shadow.offset.x, y: origin.y + shadow.offset.y };
            self.push_text(&shadow_origin, text, shadow_style, font, surface);
        }
        if let Some(outline) = style.outline.filter(|outline| outline.color.a > 0.0) {
            let mut outline_style = style.clone();
            outline_style.color = outline.color;
            outline_style.outline = None;
            outline_style.shadow = None;
            let outline_y = outline.width;
            let outline_x = outline.width * surface.height as f32 / surface.width as f32;
            for (offset_x, offset_y) in [
                (-outline_x, -outline_y),
                (0.0, -outline_y),
                (outline_x, -outline_y),
                (-outline_x, 0.0),
                (outline_x, 0.0),
                (-outline_x, outline_y),
                (0.0, outline_y),
                (outline_x, outline_y),
            ] {
                let outline_origin = Point { x: origin.x + offset_x, y: origin.y + offset_y };
                self.push_text(&outline_origin, text, outline_style.clone(), font, surface);
            }
        }

        let mut px_size = (style.size * surface.height as f32).max(1.0);
        let original_px_size = px_size;
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let mut text = std::borrow::Cow::Borrowed(text);
        let mut scale = PxScale::from(px_size);
        let mut scaled_font = font.as_scaled(scale);
        let mut text_width = text_width_px(&text, font, &scaled_font);
        if style.wrapping && max_width > 0.0 {
            let lines = wrap_text_to_width(&text, font, &scaled_font, max_width);
            let line_height = (scaled_font.ascent() - scaled_font.descent()
                + scaled_font.line_gap())
            .max(px_size);
            let origin_x = origin.x * surface.width as f32;
            let first_baseline_y = origin.y * surface.height as f32 + scaled_font.ascent();
            for (index, line) in lines.iter().enumerate() {
                let line_width = text_width_px(line, font, &scaled_font);
                let baseline_y = first_baseline_y + line_height * index as f32;
                self.push_text_line(
                    origin_x,
                    baseline_y,
                    line,
                    line_width,
                    scale,
                    style.clone(),
                    font,
                    surface,
                );
            }
            return;
        }
        if max_width > 0.0 && text_width > max_width {
            match style.overflow {
                TextOverflow::Overflow => {}
                TextOverflow::Shrink => {
                    px_size = (px_size * max_width / text_width).max(1.0);
                    scale = PxScale::from(px_size);
                    scaled_font = font.as_scaled(scale);
                    text_width = text_width_px(&text, font, &scaled_font);
                }
                TextOverflow::Truncate => {
                    text = std::borrow::Cow::Owned(truncate_text_to_width(
                        &text,
                        font,
                        &scaled_font,
                        max_width,
                    ));
                    text_width = text_width_px(&text, font, &scaled_font);
                }
            }
        }
        let cursor_x = origin.x * surface.width as f32;
        let shrink_offset_y =
            if matches!(style.overflow, TextOverflow::Shrink) && px_size < original_px_size {
                (original_px_size - px_size) / 2.0
            } else {
                0.0
            };
        let baseline_y = origin.y * surface.height as f32 + shrink_offset_y + scaled_font.ascent();

        self.push_text_line(cursor_x, baseline_y, &text, text_width, scale, style, font, surface);
    }

    fn push_text_line(
        &mut self,
        origin_x: f32,
        baseline_y: f32,
        text: &str,
        text_width: f32,
        scale: PxScale,
        style: TextStyle,
        font: &FontArc,
        surface: SurfaceSize,
    ) {
        let scaled_font = font.as_scaled(scale);
        let max_width = style.max_width.max(0.0) * surface.width as f32;
        let align_offset = text_align_offset_px(style.align, max_width, text_width);
        let mut cursor_x = origin_x + align_offset;

        for ch in text.chars() {
            let glyph_id = font.glyph_id(ch);
            let advance = scaled_font.h_advance(glyph_id);
            let glyph = Glyph { id: glyph_id, scale, position: point(cursor_x, baseline_y) };
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let glyph_width = bounds.width().ceil().max(0.0) as u32;
                let glyph_height = bounds.height().ceil().max(0.0) as u32;
                if glyph_width > 0 && glyph_height > 0 {
                    let atlas_origin = self.reserve(glyph_width, glyph_height);
                    outlined.draw(|x, y, coverage| {
                        let dst_x = atlas_origin.0 + x;
                        let dst_y = atlas_origin.1 + y;
                        let index = ((dst_y * self.width + dst_x) * 4) as usize;
                        let coverage_u8 = (coverage * 255.0).clamp(0.0, 255.0) as u8;
                        if let Some(pixel) = self.pixels.get_mut(index..index + 4) {
                            // TTF グリフは白 RGB に coverage を alpha として書く。
                            // 既存値より coverage が大きい場合のみ上書き。
                            if coverage_u8 >= pixel[3] {
                                pixel[0] = 255;
                                pixel[1] = 255;
                                pixel[2] = 255;
                                pixel[3] = coverage_u8;
                            }
                        }
                    });
                    self.quads.push(TextQuad {
                        x: bounds.min.x / surface.width as f32,
                        y: bounds.min.y / surface.height as f32,
                        width: glyph_width as f32 / surface.width as f32,
                        height: glyph_height as f32 / surface.height as f32,
                        atlas_origin,
                        glyph_width,
                        glyph_height,
                        color: style.color,
                    });
                }
            }
            cursor_x += advance;
        }
    }

    fn reserve(&mut self, glyph_width: u32, glyph_height: u32) -> (u32, u32) {
        let padded_width = glyph_width + TEXT_ATLAS_PADDING * 2;
        let padded_height = glyph_height + TEXT_ATLAS_PADDING * 2;
        if self.pen_x + padded_width > self.width {
            self.pen_x = 0;
            self.pen_y += self.row_height;
            self.row_height = 0;
        }

        let origin = (self.pen_x + TEXT_ATLAS_PADDING, self.pen_y + TEXT_ATLAS_PADDING);
        self.pen_x += padded_width;
        self.row_height = self.row_height.max(padded_height);
        self.ensure_height(self.pen_y + self.row_height);
        origin
    }

    fn ensure_height(&mut self, height: u32) {
        let needed = (self.width * height * 4) as usize;
        if self.pixels.len() < needed {
            self.pixels.resize(needed, 0);
        }
    }

    fn atlas_height(&self) -> u32 {
        (self.pen_y + self.row_height).max(1)
    }

    pub(super) fn finish(mut self) -> TextFrame {
        let height = self.atlas_height();
        self.pixels.resize((self.width * height * 4) as usize, 0);
        let instances = encode_text_quads(&self.quads, self.width, height);
        TextFrame {
            size: AtlasSize { width: self.width, height },
            #[cfg(test)]
            pixels: self.pixels,
            dirty_regions: Vec::new(),
            instances,
            command_quad_counts: Vec::new(),
            command_caret_rects: Vec::new(),
        }
    }
}

#[cfg(test)]
pub(super) fn text_width_px<F: Font>(
    text: &str,
    font: &FontArc,
    scaled_font: &impl ScaleFont<F>,
) -> f32 {
    text.chars().map(|ch| scaled_font.h_advance(font.glyph_id(ch))).sum()
}

pub(super) fn fallback_char_advance(ch: char, fonts: VectorFontSet<'_>, scale: PxScale) -> f32 {
    let Some((_, font)) = fonts.select(ch) else {
        return 0.0;
    };
    font.as_scaled(scale).h_advance(font.glyph_id(ch))
}

pub(super) fn fallback_text_width_px(text: &str, fonts: VectorFontSet<'_>, scale: PxScale) -> f32 {
    text.chars().map(|ch| fallback_char_advance(ch, fonts, scale)).sum()
}

pub(super) fn bitmap_text_width_px(text: &str, font: &BitmapFont, scale: f32) -> f32 {
    text.chars()
        .filter_map(|ch| font.glyphs.get(&ch))
        .map(|glyph| glyph.xadvance as f32 * scale)
        .sum()
}

pub(super) fn rasterized_bitmap_glyph_pixels(
    glyph: crate::bitmap_font::BitmapFontGlyph,
    page: &crate::bitmap_font::BitmapFontPage,
    scale: f32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut pixels = vec![0; (width * height * 4) as usize];
    let nearest = is_integer_scale(scale);
    for dst_y in 0..height {
        for dst_x in 0..width {
            let color = if nearest {
                sample_bitmap_glyph_nearest(glyph, page, dst_x, dst_y, scale)
            } else {
                sample_bitmap_glyph_bilinear(glyph, page, dst_x, dst_y, scale)
            };
            let dst_index = ((dst_y * width + dst_x) * 4) as usize;
            if let Some(dst) = pixels.get_mut(dst_index..dst_index + 4)
                && color[3] >= dst[3]
            {
                dst.copy_from_slice(&color);
            }
        }
    }
    pixels
}

pub(super) fn is_integer_scale(scale: f32) -> bool {
    (scale - scale.round()).abs() <= 0.001
}

pub(super) fn sample_bitmap_glyph_nearest(
    glyph: crate::bitmap_font::BitmapFontGlyph,
    page: &crate::bitmap_font::BitmapFontPage,
    dst_x: u32,
    dst_y: u32,
    scale: f32,
) -> [u8; 4] {
    let local_x = (dst_x as f32 / scale).floor() as i32;
    let local_y = (dst_y as f32 / scale).floor() as i32;
    bitmap_glyph_pixel(glyph, page, local_x, local_y)
}

pub(super) fn sample_bitmap_glyph_bilinear(
    glyph: crate::bitmap_font::BitmapFontGlyph,
    page: &crate::bitmap_font::BitmapFontPage,
    dst_x: u32,
    dst_y: u32,
    scale: f32,
) -> [u8; 4] {
    let max_x = glyph.width.saturating_sub(1) as f32;
    let max_y = glyph.height.saturating_sub(1) as f32;
    let src_x = ((dst_x as f32 + 0.5) / scale - 0.5).clamp(0.0, max_x);
    let src_y = ((dst_y as f32 + 0.5) / scale - 0.5).clamp(0.0, max_y);
    let x0 = src_x.floor() as i32;
    let y0 = src_y.floor() as i32;
    let x1 = (x0 + 1).min(max_x as i32);
    let y1 = (y0 + 1).min(max_y as i32);
    let tx = src_x - x0 as f32;
    let ty = src_y - y0 as f32;

    let p00 = bitmap_glyph_pixel(glyph, page, x0, y0);
    let p10 = bitmap_glyph_pixel(glyph, page, x1, y0);
    let p01 = bitmap_glyph_pixel(glyph, page, x0, y1);
    let p11 = bitmap_glyph_pixel(glyph, page, x1, y1);
    blend_bitmap_pixels([
        (p00, (1.0 - tx) * (1.0 - ty)),
        (p10, tx * (1.0 - ty)),
        (p01, (1.0 - tx) * ty),
        (p11, tx * ty),
    ])
}

pub(super) fn bitmap_glyph_pixel(
    glyph: crate::bitmap_font::BitmapFontGlyph,
    page: &crate::bitmap_font::BitmapFontPage,
    local_x: i32,
    local_y: i32,
) -> [u8; 4] {
    if glyph.width == 0 || glyph.height == 0 {
        return [0, 0, 0, 0];
    }
    let local_x = local_x.clamp(0, glyph.width.saturating_sub(1) as i32) as u32;
    let local_y = local_y.clamp(0, glyph.height.saturating_sub(1) as i32) as u32;
    let src_x = glyph.x + local_x;
    let src_y = glyph.y + local_y;
    if src_x >= page.image.width || src_y >= page.image.height {
        return [0, 0, 0, 0];
    }
    let src_index = ((src_y * page.image.width + src_x) * 4) as usize;
    page.image
        .pixels
        .get(src_index..src_index + 4)
        .map(|src| [src[0], src[1], src[2], src[3]])
        .unwrap_or([0, 0, 0, 0])
}

pub(super) fn blend_bitmap_pixels(samples: [([u8; 4], f32); 4]) -> [u8; 4] {
    let mut alpha = 0.0;
    let mut premul = [0.0; 3];
    for (pixel, weight) in samples {
        let a = pixel[3] as f32 / 255.0;
        alpha += a * weight;
        for channel in 0..3 {
            premul[channel] += (pixel[channel] as f32 / 255.0) * a * weight;
        }
    }
    if alpha <= f32::EPSILON {
        return [0, 0, 0, 0];
    }
    [
        ((premul[0] / alpha) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((premul[1] / alpha) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((premul[2] / alpha) * 255.0).round().clamp(0.0, 255.0) as u8,
        (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

pub(super) fn text_align_offset_px(align: TextAlign, max_width: f32, text_width: f32) -> f32 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center if max_width > 0.0 => (max_width - text_width) / 2.0,
        TextAlign::Center => -text_width / 2.0,
        TextAlign::Right if max_width > 0.0 => max_width - text_width,
        TextAlign::Right => -text_width,
    }
}

#[cfg(test)]
pub(super) fn vector_text_caret_rect(
    origin: &Point,
    text: &str,
    style: &TextStyle,
    font: &FontArc,
    surface: SurfaceSize,
    caret: TextCaret,
) -> Option<RectCommand> {
    if style.wrapping || !surface.is_drawable() {
        return None;
    }
    let mut px_size = (style.size * surface.height as f32).max(1.0);
    let original_px_size = px_size;
    let max_width = style.max_width.max(0.0) * surface.width as f32;
    let mut visible = Cow::Borrowed(text);
    let mut scale = PxScale::from(px_size);
    let mut scaled_font = font.as_scaled(scale);
    let mut text_width = text_width_px(&visible, font, &scaled_font);
    if max_width > 0.0 && text_width > max_width {
        match style.overflow {
            TextOverflow::Overflow => {}
            TextOverflow::Shrink => {
                px_size = (px_size * max_width / text_width).max(1.0);
                scale = PxScale::from(px_size);
                scaled_font = font.as_scaled(scale);
                text_width = text_width_px(&visible, font, &scaled_font);
            }
            TextOverflow::Truncate => {
                visible =
                    Cow::Owned(truncate_text_to_width(&visible, font, &scaled_font, max_width));
                text_width = text_width_px(&visible, font, &scaled_font);
            }
        }
    }
    let align_offset = text_align_offset_px(style.align, max_width, text_width);
    let cursor = clamp_text_byte_index(&visible, caret.byte_index);
    let prefix_width = text_width_px(&visible[..cursor], font, &scaled_font);
    let shrink_offset_y =
        if matches!(style.overflow, TextOverflow::Shrink) && px_size < original_px_size {
            (original_px_size - px_size) / 2.0
        } else {
            0.0
        };
    let x = (origin.x * surface.width as f32 + align_offset + prefix_width) / surface.width as f32;
    let y = (origin.y * surface.height as f32 + shrink_offset_y) / surface.height as f32;
    Some(RectCommand {
        rect: Rect {
            x,
            y,
            width: (2.0 / surface.width as f32).max(0.001),
            height: (px_size / surface.height as f32).max(0.001),
        },
        color: caret.color,
    })
}

pub(super) fn vector_text_caret_rect_with_fallback(
    origin: &Point,
    text: &str,
    style: &TextStyle,
    fonts: VectorFontSet<'_>,
    surface: SurfaceSize,
    caret: TextCaret,
) -> Option<RectCommand> {
    if style.wrapping || !surface.is_drawable() || fonts.primary().is_none() {
        return None;
    }
    let mut px_size = (style.size * surface.height as f32).max(1.0);
    let original_px_size = px_size;
    let max_width = style.max_width.max(0.0) * surface.width as f32;
    let mut visible = Cow::Borrowed(text);
    let mut scale = PxScale::from(px_size);
    let mut text_width = fallback_text_width_px(&visible, fonts, scale);
    if max_width > 0.0 && text_width > max_width {
        match style.overflow {
            TextOverflow::Overflow => {}
            TextOverflow::Shrink => {
                px_size = (px_size * max_width / text_width).max(1.0);
                scale = PxScale::from(px_size);
                text_width = fallback_text_width_px(&visible, fonts, scale);
            }
            TextOverflow::Truncate => {
                visible =
                    Cow::Owned(truncate_fallback_text_to_width(&visible, fonts, scale, max_width));
                text_width = fallback_text_width_px(&visible, fonts, scale);
            }
        }
    }
    let align_offset = text_align_offset_px(style.align, max_width, text_width);
    let cursor = clamp_text_byte_index(&visible, caret.byte_index);
    let prefix_width = fallback_text_width_px(&visible[..cursor], fonts, scale);
    let shrink_offset_y =
        if matches!(style.overflow, TextOverflow::Shrink) && px_size < original_px_size {
            (original_px_size - px_size) / 2.0
        } else {
            0.0
        };
    let x = (origin.x * surface.width as f32 + align_offset + prefix_width) / surface.width as f32;
    let y = (origin.y * surface.height as f32 + shrink_offset_y) / surface.height as f32;
    Some(RectCommand {
        rect: Rect {
            x,
            y,
            width: (2.0 / surface.width as f32).max(0.001),
            height: (px_size / surface.height as f32).max(0.001),
        },
        color: caret.color,
    })
}

pub(super) fn bitmap_text_caret_rect(
    origin: &Point,
    text: &str,
    style: &TextStyle,
    font: &BitmapFont,
    surface: SurfaceSize,
    caret: TextCaret,
) -> Option<RectCommand> {
    if style.wrapping || !surface.is_drawable() {
        return None;
    }
    let design_size = if font.size > 0 { font.size } else { font.line_height.max(1) };
    let bitmap_size = style.bitmap_size.unwrap_or(style.size);
    let mut scale = (bitmap_size * surface.height as f32 / design_size as f32).max(0.01);
    let original_scale = scale;
    let max_width = style.max_width.max(0.0) * surface.width as f32;
    let mut text_width = bitmap_text_width_px(text, font, scale);
    let visible = if max_width > 0.0 && text_width > max_width {
        match style.overflow {
            TextOverflow::Overflow => Cow::Borrowed(text),
            TextOverflow::Shrink => {
                scale = (scale * max_width / text_width).max(0.01);
                Cow::Borrowed(text)
            }
            TextOverflow::Truncate => {
                Cow::Owned(truncate_bitmap_text_to_width(text, font, max_width, scale))
            }
        }
    } else {
        Cow::Borrowed(text)
    };
    text_width = bitmap_text_width_px(&visible, font, scale);
    let align_offset = text_align_offset_px(style.align, max_width, text_width);
    let cursor = clamp_text_byte_index(&visible, caret.byte_index);
    let prefix_width = bitmap_text_width_px(&visible[..cursor], font, scale);
    let shrink_offset_y =
        if matches!(style.overflow, TextOverflow::Shrink) && scale < original_scale {
            (design_size as f32 * (original_scale - scale)) / 2.0
        } else {
            0.0
        };
    let caret_height = design_size as f32 * scale;
    let x = (origin.x * surface.width as f32 + align_offset + prefix_width) / surface.width as f32;
    let y = (origin.y * surface.height as f32 + shrink_offset_y) / surface.height as f32;
    Some(RectCommand {
        rect: Rect {
            x,
            y,
            width: (2.0 / surface.width as f32).max(0.001),
            height: (caret_height / surface.height as f32).max(0.001),
        },
        color: caret.color,
    })
}

pub(super) fn clamp_text_byte_index(text: &str, byte_index: usize) -> usize {
    let mut byte_index = byte_index.min(text.len());
    while byte_index > 0 && !text.is_char_boundary(byte_index) {
        byte_index -= 1;
    }
    byte_index
}

pub(super) fn truncate_bitmap_text_to_width(
    text: &str,
    font: &BitmapFont,
    max_width: f32,
    scale: f32,
) -> String {
    let mut width = 0.0;
    let mut result = String::new();
    for ch in text.chars() {
        let Some(glyph) = font.glyphs.get(&ch) else {
            continue;
        };
        let advance = glyph.xadvance as f32 * scale;
        if width + advance > max_width {
            break;
        }
        width += advance;
        result.push(ch);
    }
    result
}

#[cfg(test)]
pub(super) fn wrap_text_to_width<F: Font>(
    text: &str,
    font: &FontArc,
    scaled_font: &impl ScaleFont<F>,
    max_width: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    for source_line in text.split('\n') {
        let mut line = String::new();
        let mut width = 0.0;
        for ch in source_line.chars() {
            let advance = scaled_font.h_advance(font.glyph_id(ch));
            if !line.is_empty() && width + advance > max_width {
                lines.push(std::mem::take(&mut line));
                width = 0.0;
            }
            line.push(ch);
            width += advance;
        }
        lines.push(line);
    }
    lines
}

pub(super) fn wrap_fallback_text_to_width(
    text: &str,
    fonts: VectorFontSet<'_>,
    scale: PxScale,
    max_width: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    for source_line in text.split('\n') {
        let mut line = String::new();
        let mut width = 0.0;
        for ch in source_line.chars() {
            let advance = fallback_char_advance(ch, fonts, scale);
            if !line.is_empty() && width + advance > max_width {
                lines.push(std::mem::take(&mut line));
                width = 0.0;
            }
            line.push(ch);
            width += advance;
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
pub(super) fn truncate_text_to_width<F: Font>(
    text: &str,
    font: &FontArc,
    scaled_font: &impl ScaleFont<F>,
    max_width: f32,
) -> String {
    let mut width = 0.0;
    let mut result = String::new();
    for ch in text.chars() {
        let advance = scaled_font.h_advance(font.glyph_id(ch));
        if width + advance > max_width {
            break;
        }
        width += advance;
        result.push(ch);
    }
    result
}

pub(super) fn truncate_fallback_text_to_width(
    text: &str,
    fonts: VectorFontSet<'_>,
    scale: PxScale,
    max_width: f32,
) -> String {
    let mut width = 0.0;
    let mut result = String::new();
    for ch in text.chars() {
        let advance = fallback_char_advance(ch, fonts, scale);
        if width + advance > max_width {
            break;
        }
        width += advance;
        result.push(ch);
    }
    result
}

pub(super) fn encode_text_quads(
    quads: &[TextQuad],
    atlas_width: u32,
    atlas_height: u32,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(quads.len() * TEXT_INSTANCE_BYTES);
    for quad in quads {
        for value in [
            quad.x,
            quad.y,
            quad.width,
            quad.height,
            quad.atlas_origin.0 as f32 / atlas_width as f32,
            quad.atlas_origin.1 as f32 / atlas_height as f32,
            quad.glyph_width as f32 / atlas_width as f32,
            quad.glyph_height as f32 / atlas_height as f32,
            quad.color.r,
            quad.color.g,
            quad.color.b,
            quad.color.a,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}
