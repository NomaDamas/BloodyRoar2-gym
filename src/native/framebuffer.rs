pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;
pub const DEFAULT_DISPLAY_WIDTH: usize = 320;
pub const DEFAULT_DISPLAY_HEIGHT: usize = 240;

#[derive(Clone, Debug)]
pub struct NativeFrameBuffer {
    pixels: Vec<u32>,
    raw_pixels: Vec<u16>,
    clip: Option<ClipRect>,
}

impl Default for NativeFrameBuffer {
    fn default() -> Self {
        Self {
            pixels: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
            raw_pixels: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
            clip: None,
        }
    }
}

impl NativeFrameBuffer {
    pub fn set_clip(&mut self, clip: Option<ClipRect>) {
        self.clip = clip;
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if !self.in_clip(x, y) {
            return;
        }
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return;
        }

        let index = y as usize * VRAM_WIDTH + x as usize;
        let rgb = color & 0x00ff_ffff;
        self.pixels[index] = rgb;
        self.raw_pixels[index] = rgb888_to_rgb555(rgb);
    }

    pub fn set_raw_pixel(&mut self, x: i32, y: i32, color: u16) {
        if !self.in_clip(x, y) {
            return;
        }
        self.set_raw_pixel_unclipped(x, y, color);
    }

    fn set_raw_pixel_unclipped(&mut self, x: i32, y: i32, color: u16) {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return;
        }

        let index = y as usize * VRAM_WIDTH + x as usize;
        self.raw_pixels[index] = color;
        self.pixels[index] = rgb555_to_rgb888(color);
    }

    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return 0;
        }

        self.pixels[y as usize * VRAM_WIDTH + x as usize]
    }

    pub fn raw_pixel(&self, x: i32, y: i32) -> u16 {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return 0;
        }

        self.raw_pixels[y as usize * VRAM_WIDTH + x as usize]
    }

    pub fn fill_rect_unclipped(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        if width <= 0 || height <= 0 {
            return;
        }

        let left = x.clamp(0, VRAM_WIDTH as i32) as usize;
        let top = y.clamp(0, VRAM_HEIGHT as i32) as usize;
        let right = x.saturating_add(width).clamp(0, VRAM_WIDTH as i32) as usize;
        let bottom = y.saturating_add(height).clamp(0, VRAM_HEIGHT as i32) as usize;
        if left >= right || top >= bottom {
            return;
        }

        self.fill_rect_region(left, top, right, bottom, color);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        if width <= 0 || height <= 0 {
            return;
        }

        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        let left = x.clamp(clip_left, clip_right) as usize;
        let top = y.clamp(clip_top, clip_bottom) as usize;
        let right = x.saturating_add(width).clamp(clip_left, clip_right) as usize;
        let bottom = y.saturating_add(height).clamp(clip_top, clip_bottom) as usize;
        if left >= right || top >= bottom {
            return;
        }

        self.fill_rect_region(left, top, right, bottom, color);
    }

    fn fill_rect_region(
        &mut self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
        color: u32,
    ) {
        let rgb = color & 0x00ff_ffff;
        let raw = rgb888_to_rgb555(rgb);
        for row in top..bottom {
            let offset = row * VRAM_WIDTH;
            for col in left..right {
                self.pixels[offset + col] = rgb;
                self.raw_pixels[offset + col] = raw;
            }
        }
    }

    pub fn copy_rect(
        &mut self,
        source_x: i32,
        source_y: i32,
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let mut copied = Vec::with_capacity((width as usize).saturating_mul(height as usize));
        for row in 0..height {
            for col in 0..width {
                copied.push((
                    self.pixel(source_x + col, source_y + row),
                    self.raw_pixel(source_x + col, source_y + row),
                ));
            }
        }

        for row in 0..height {
            for col in 0..width {
                let index = (row as usize)
                    .saturating_mul(width as usize)
                    .saturating_add(col as usize);
                if let Some((rgb, raw)) = copied.get(index) {
                    let x = dest_x + col;
                    let y = dest_y + row;
                    if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
                        continue;
                    }
                    let dest_index = y as usize * VRAM_WIDTH + x as usize;
                    self.pixels[dest_index] = *rgb;
                    self.raw_pixels[dest_index] = *raw;
                }
            }
        }
    }

    pub fn write_rgb555_image(&mut self, x: i32, y: i32, width: i32, height: i32, words: &[u32]) {
        if width <= 0 || height <= 0 {
            return;
        }

        let pixels = (width as usize).saturating_mul(height as usize);
        for index in 0..pixels {
            let Some(word) = words.get(index / 2) else {
                break;
            };
            let raw = if index & 1 == 0 {
                word & 0xffff
            } else {
                word >> 16
            };
            let col = (index % width as usize) as i32;
            let row = (index / width as usize) as i32;
            self.set_raw_pixel_unclipped(x + col, y + row, raw as u16);
        }
    }

    pub fn draw_line(&mut self, a: Point, b: Point, color: u32) {
        let mut x0 = a.x;
        let mut y0 = a.y;
        let x1 = b.x;
        let y1 = b.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn draw_triangle(&mut self, a: Point, b: Point, c: Point, color: u32) {
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return;
        }

        let min_x = a.x.min(b.x).min(c.x).clamp(clip_left, clip_right - 1);
        let max_x = a.x.max(b.x).max(c.x).clamp(clip_left, clip_right - 1);
        let min_y = a.y.min(b.y).min(c.y).clamp(clip_top, clip_bottom - 1);
        let max_y = a.y.max(b.y).max(c.y).clamp(clip_top, clip_bottom - 1);
        if min_x > max_x || min_y > max_y {
            return;
        }

        let area = edge(a, b, c);
        if area == 0 {
            return;
        }

        let rgb = color & 0x00ff_ffff;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point { x, y };
                let w0 = edge(b, c, point);
                let w1 = edge(c, a, point);
                let w2 = edge(a, b, point);
                let inside = if area > 0 {
                    w0 >= 0 && w1 >= 0 && w2 >= 0
                } else {
                    w0 <= 0 && w1 <= 0 && w2 <= 0
                };
                if inside {
                    self.set_pixel(x, y, rgb);
                }
            }
        }
    }

    pub fn draw_textured_triangle(
        &mut self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
        texture_page: u16,
        clut: u16,
        texture_window: TextureWindow,
    ) {
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return;
        }

        let min_x = a
            .point
            .x
            .min(b.point.x)
            .min(c.point.x)
            .clamp(clip_left, clip_right - 1);
        let max_x = a
            .point
            .x
            .max(b.point.x)
            .max(c.point.x)
            .clamp(clip_left, clip_right - 1);
        let min_y = a
            .point
            .y
            .min(b.point.y)
            .min(c.point.y)
            .clamp(clip_top, clip_bottom - 1);
        let max_y = a
            .point
            .y
            .max(b.point.y)
            .max(c.point.y)
            .clamp(clip_top, clip_bottom - 1);
        let area = edge(a.point, b.point, c.point);
        if area == 0 {
            return;
        }

        let source = self.clone();
        let denom = area.unsigned_abs() as i64;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point { x, y };
                let mut w0 = edge(b.point, c.point, point);
                let mut w1 = edge(c.point, a.point, point);
                let mut w2 = edge(a.point, b.point, point);
                let inside = if area > 0 {
                    w0 >= 0 && w1 >= 0 && w2 >= 0
                } else {
                    w0 <= 0 && w1 <= 0 && w2 <= 0
                };
                if !inside {
                    continue;
                }

                if area < 0 {
                    w0 = -w0;
                    w1 = -w1;
                    w2 = -w2;
                }
                let u = ((a.u as i64 * w0 + b.u as i64 * w1 + c.u as i64 * w2) / denom) as u8;
                let v = ((a.v as i64 * w0 + b.v as i64 * w1 + c.v as i64 * w2) / denom) as u8;
                let (u, v) = texture_window.apply(u, v);
                if let Some(color) = source.sample_texture(texture_page, clut, u, v) {
                    self.set_raw_pixel(x, y, color);
                }
            }
        }
    }

    pub fn draw_textured_rect(
        &mut self,
        dest: Point,
        size: (i32, i32),
        texture_page: u16,
        clut: u16,
        uv: TextureCoordinate,
        texture_window: TextureWindow,
    ) {
        let (width, height) = size;
        if width <= 0 || height <= 0 {
            return;
        }

        let source = self.clone();
        for row in 0..height {
            for col in 0..width {
                let u = uv.u.wrapping_add(col as u8);
                let v = uv.v.wrapping_add(row as u8);
                let (u, v) = texture_window.apply(u, v);
                if let Some(color) = source.sample_texture(texture_page, clut, u, v) {
                    self.set_raw_pixel(dest.x + col, dest.y + row, color);
                }
            }
        }
    }

    fn in_clip(&self, x: i32, y: i32) -> bool {
        self.clip.is_none_or(|clip| {
            x >= clip.left && x <= clip.right && y >= clip.top && y <= clip.bottom
        })
    }

    fn clip_bounds(&self) -> (i32, i32, i32, i32) {
        self.clip
            .map_or((0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32), |clip| {
                (
                    clip.left.clamp(0, VRAM_WIDTH as i32),
                    clip.top.clamp(0, VRAM_HEIGHT as i32),
                    clip.right.saturating_add(1).clamp(0, VRAM_WIDTH as i32),
                    clip.bottom.saturating_add(1).clamp(0, VRAM_HEIGHT as i32),
                )
            })
    }

    fn sample_texture(&self, texture_page: u16, clut: u16, u: u8, v: u8) -> Option<u16> {
        let page_x = ((texture_page & 0x0f) as i32) * 64;
        let page_y = (((texture_page >> 4) & 0x01) as i32) * 256;
        let mode = (texture_page >> 7) & 0x03;
        let u = u as i32;
        let v = v as i32;

        let color = match mode {
            0 => {
                let packed = self.raw_pixel(page_x + u / 4, page_y + v);
                let index = ((packed >> ((u & 3) * 4)) & 0x0f) as i32;
                self.raw_pixel(
                    ((clut & 0x3f) as i32) * 16 + index,
                    ((clut >> 6) & 0x01ff) as i32,
                )
            }
            1 => {
                let packed = self.raw_pixel(page_x + u / 2, page_y + v);
                let index = if u & 1 == 0 {
                    packed & 0x00ff
                } else {
                    packed >> 8
                } as i32;
                self.raw_pixel(
                    ((clut & 0x3f) as i32) * 16 + index,
                    ((clut >> 6) & 0x01ff) as i32,
                )
            }
            _ => self.raw_pixel(page_x + u, page_y + v),
        };

        (color & 0x7fff != 0).then_some(color)
    }

    pub fn png_base64(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> String {
        base64_encode(&self.png(start_x, start_y, width, height))
    }

    pub fn png(&self, start_x: usize, start_y: usize, width: usize, height: usize) -> Vec<u8> {
        png_rgb(
            width.max(1),
            height.max(1),
            &self.rgb_rows(start_x, start_y, width.max(1), height.max(1)),
        )
    }

    pub fn display_stats(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> FrameBufferStats {
        let mut nonzero_pixels = 0_u64;
        let mut checksum = 0x811c_9dc5_u32;
        for y in 0..height {
            let source_y = start_y + y;
            for x in 0..width {
                let source_x = start_x + x;
                let rgb = if source_x < VRAM_WIDTH && source_y < VRAM_HEIGHT {
                    self.pixels[source_y * VRAM_WIDTH + source_x]
                } else {
                    0
                };
                if rgb != 0 {
                    nonzero_pixels += 1;
                }
                checksum ^= rgb;
                checksum = checksum.wrapping_mul(16_777_619);
            }
        }
        FrameBufferStats {
            nonzero_pixels,
            checksum,
        }
    }

    pub fn stats(&self) -> FrameBufferStats {
        self.display_stats(0, 0, VRAM_WIDTH, VRAM_HEIGHT)
    }

    pub fn densest_window(
        &self,
        width: usize,
        height: usize,
        step: usize,
    ) -> Option<FrameBufferWindow> {
        if width == 0 || height == 0 || width > VRAM_WIDTH || height > VRAM_HEIGHT {
            return None;
        }

        let step = step.max(1);
        let integral = self.nonzero_integral_image();
        let max_x = VRAM_WIDTH - width;
        let max_y = VRAM_HEIGHT - height;
        let mut best: Option<(usize, usize, u64)> = None;

        for y in stepped_positions(max_y, step) {
            for x in stepped_positions(max_x, step) {
                let nonzero_pixels = integral_rect(&integral, x, y, width, height);
                if best.is_none_or(|(_, _, best_count)| nonzero_pixels > best_count) {
                    best = Some((x, y, nonzero_pixels));
                }
            }
        }

        let (x, y, nonzero_pixels) = best?;
        (nonzero_pixels > 0).then(|| FrameBufferWindow {
            x,
            y,
            stats: self.display_stats(x, y, width, height),
        })
    }

    pub fn nonzero_bounds(&self) -> Option<FrameBufferBounds> {
        let mut left = VRAM_WIDTH;
        let mut top = VRAM_HEIGHT;
        let mut right = 0;
        let mut bottom = 0;

        for y in 0..VRAM_HEIGHT {
            for x in 0..VRAM_WIDTH {
                if self.pixels[y * VRAM_WIDTH + x] == 0 {
                    continue;
                }
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }

        (left <= right && top <= bottom).then_some(FrameBufferBounds {
            left,
            top,
            right,
            bottom,
        })
    }

    fn nonzero_integral_image(&self) -> Vec<u32> {
        let stride = VRAM_WIDTH + 1;
        let mut integral = vec![0_u32; stride * (VRAM_HEIGHT + 1)];

        for y in 0..VRAM_HEIGHT {
            let mut row_total = 0_u32;
            for x in 0..VRAM_WIDTH {
                if self.pixels[y * VRAM_WIDTH + x] != 0 {
                    row_total += 1;
                }
                let index = (y + 1) * stride + x + 1;
                integral[index] = integral[y * stride + x + 1] + row_total;
            }
        }

        integral
    }

    fn rgb_rows(&self, start_x: usize, start_y: usize, width: usize, height: usize) -> Vec<u8> {
        let mut rows = Vec::with_capacity(height * (1 + width * 3));
        for y in 0..height {
            rows.push(0);
            let source_y = start_y + y;
            for x in 0..width {
                let source_x = start_x + x;
                let rgb = if source_x < VRAM_WIDTH && source_y < VRAM_HEIGHT {
                    self.pixels[source_y * VRAM_WIDTH + source_x]
                } else {
                    0
                };
                rows.push(((rgb >> 16) & 0xff) as u8);
                rows.push(((rgb >> 8) & 0xff) as u8);
                rows.push((rgb & 0xff) as u8);
            }
        }
        rows
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferStats {
    pub nonzero_pixels: u64,
    pub checksum: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferBounds {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

impl FrameBufferBounds {
    pub fn json(self) -> String {
        format!(
            "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
            self.left, self.top, self.right, self.bottom
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferWindow {
    pub x: usize,
    pub y: usize,
    pub stats: FrameBufferStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ClipRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
        if left > right || top > bottom {
            return None;
        }

        Some(Self {
            left,
            top,
            right,
            bottom,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TexturedPoint {
    pub point: Point,
    pub u: u8,
    pub v: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureCoordinate {
    pub u: u8,
    pub v: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextureWindow {
    mask_x: u8,
    mask_y: u8,
    offset_x: u8,
    offset_y: u8,
}

impl TextureWindow {
    pub fn from_gp0_e2(value: u32) -> Self {
        Self {
            mask_x: ((value & 0x1f) as u8) << 3,
            mask_y: (((value >> 5) & 0x1f) as u8) << 3,
            offset_x: (((value >> 10) & 0x1f) as u8) << 3,
            offset_y: (((value >> 15) & 0x1f) as u8) << 3,
        }
    }

    fn apply(self, u: u8, v: u8) -> (u8, u8) {
        (
            (u & !self.mask_x) | (self.offset_x & self.mask_x),
            (v & !self.mask_y) | (self.offset_y & self.mask_y),
        )
    }
}

fn edge(a: Point, b: Point, c: Point) -> i64 {
    ((c.x - a.x) as i64 * (b.y - a.y) as i64) - ((c.y - a.y) as i64 * (b.x - a.x) as i64)
}

fn integral_rect(integral: &[u32], x: usize, y: usize, width: usize, height: usize) -> u64 {
    let stride = VRAM_WIDTH + 1;
    let right = x + width;
    let bottom = y + height;
    let value = integral[bottom * stride + right] + integral[y * stride + x]
        - integral[y * stride + right]
        - integral[bottom * stride + x];
    value as u64
}

fn stepped_positions(max: usize, step: usize) -> impl Iterator<Item = usize> {
    (0..=max)
        .step_by(step)
        .chain(std::iter::once(max))
        .scan(None, |previous, value| {
            if previous.is_some_and(|previous| previous == value) {
                return Some(None);
            }
            *previous = Some(value);
            Some(Some(value))
        })
        .flatten()
}

fn png_rgb(width: usize, height: usize, filtered_rgb_rows: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &zlib_uncompressed(filtered_rgb_rows));
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn write_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn zlib_uncompressed(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut remaining = data;
    while !remaining.is_empty() {
        let chunk_len = remaining.len().min(u16::MAX as usize);
        let final_block = chunk_len == remaining.len();
        output.push(if final_block { 0x01 } else { 0x00 });
        output.extend_from_slice(&(chunk_len as u16).to_le_bytes());
        output.extend_from_slice(&(!(chunk_len as u16)).to_le_bytes());
        output.extend_from_slice(&remaining[..chunk_len]);
        remaining = &remaining[chunk_len..];
    }
    if data.is_empty() {
        output.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in data {
        a = (a + *byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn rgb555_to_rgb888(value: u16) -> u32 {
    let r = (value & 0x1f) as u32;
    let g = ((value >> 5) & 0x1f) as u32;
    let b = ((value >> 10) & 0x1f) as u32;
    ((r << 3) | (r >> 2)) << 16 | ((g << 3) | (g >> 2)) << 8 | ((b << 3) | (b >> 2))
}

fn rgb888_to_rgb555(value: u32) -> u16 {
    let r = ((value >> 19) & 0x1f) as u16;
    let g = ((value >> 11) & 0x1f) as u16;
    let b = ((value >> 3) & 0x1f) as u16;
    r | (g << 5) | (b << 10)
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{NativeFrameBuffer, Point};

    #[test]
    fn framebuffer_exports_png_base64() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.fill_rect(0, 0, 8, 8, 0x00ff_0000);

        let png = framebuffer.png_base64(0, 0, 8, 8);

        assert!(png.starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn framebuffer_draws_clipped_triangle() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.draw_triangle(
            Point { x: -10, y: -10 },
            Point { x: 20, y: 4 },
            Point { x: 4, y: 20 },
            0x0000_ff00,
        );

        let png = framebuffer.png_base64(0, 0, 16, 16);

        assert!(png.starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn framebuffer_writes_rgb555_images_and_copies_rects() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.write_rgb555_image(4, 4, 2, 1, &[0x03e0_001f]);
        framebuffer.copy_rect(4, 4, 8, 8, 2, 1);

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_finds_densest_nonzero_window() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.fill_rect(512, 24, 32, 32, 0x00ff_ffff);

        let window = framebuffer
            .densest_window(320, 240, 8)
            .expect("densest window");

        assert_eq!(window.x, 224);
        assert_eq!(window.y, 0);
        assert_eq!(window.stats.nonzero_pixels, 1024);
    }
}
