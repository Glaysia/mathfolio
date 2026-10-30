//! TrueType PDF subset retaining original GIDs and hint programs. PDF positions
//! and CIDToGIDMap already describe shaping, so layout/variation tables are not
//! needed. Composite dependencies must remain even when not directly printed.
use std::collections::{BTreeMap, BTreeSet};

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(
        data.get(at..at + 2)
            .ok_or("잘린 TrueType u16")?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        data.get(at..at + 4)
            .ok_or("잘린 TrueType u32")?
            .try_into()
            .unwrap(),
    ))
}
fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

pub(super) fn subset(data: &[u8], used: impl IntoIterator<Item = u16>) -> Result<Vec<u8>, String> {
    if !data.starts_with(&[0, 1, 0, 0]) {
        return Err("TrueType subset에는 개별 SFNT가 필요합니다.".into());
    }
    let mut tables = BTreeMap::new();
    for n in 0..usize::from(u16_at(data, 4)?) {
        let record = 12 + n * 16;
        let tag: [u8; 4] = data
            .get(record..record + 4)
            .ok_or("잘린 TrueType 표 디렉터리")?
            .try_into()
            .unwrap();
        let offset = u32_at(data, record + 8)? as usize;
        let len = u32_at(data, record + 12)? as usize;
        let value = data
            .get(offset..offset.checked_add(len).ok_or("TrueType 표 크기 초과")?)
            .ok_or("잘린 TrueType 표")?;
        if tables.insert(tag, value).is_some() {
            return Err("중복된 TrueType 표".into());
        }
    }
    let get = |tag: &[u8; 4]| {
        tables
            .get(tag)
            .copied()
            .ok_or("필수 TrueType 표 누락".to_owned())
    };
    let head = get(b"head")?;
    let maxp = get(b"maxp")?;
    let loca = get(b"loca")?;
    let glyf = get(b"glyf")?;
    let count = usize::from(u16_at(maxp, 4)?);
    let long = u16_at(head, 50)?;
    if long > 1 {
        return Err("잘못된 TrueType loca 형식".into());
    }
    let offset = |gid: usize| -> Result<usize, String> {
        Ok(if long == 1 {
            u32_at(loca, gid * 4)? as usize
        } else {
            usize::from(u16_at(loca, gid * 2)?) * 2
        })
    };
    let mut selected = BTreeSet::new();
    let mut queue = vec![0];
    queue.extend(used);
    while let Some(gid) = queue.pop() {
        if usize::from(gid) >= count {
            return Err("TrueType subset 글리프 번호가 범위를 벗어납니다.".into());
        }
        if !selected.insert(gid) {
            continue;
        }
        let glyph = glyf
            .get(offset(usize::from(gid))?..offset(usize::from(gid) + 1)?)
            .ok_or("잘못된 TrueType 글리프 범위")?;
        if glyph.is_empty() {
            continue;
        }
        if (u16_at(glyph, 0)? as i16) >= 0 {
            continue;
        }
        let mut at = 10;
        loop {
            let flags = u16_at(glyph, at)?;
            queue.push(u16_at(glyph, at + 2)?);
            at += 4 + if flags & 1 != 0 { 4 } else { 2 };
            at += if flags & 8 != 0 {
                2
            } else if flags & 0x40 != 0 {
                4
            } else if flags & 0x80 != 0 {
                8
            } else {
                0
            };
            if at > glyph.len() {
                return Err("잘린 TrueType 복합 글리프".into());
            }
            if flags & 0x20 == 0 {
                break;
            }
        }
    }
    let mut outlines = Vec::new();
    let mut locations = Vec::with_capacity((count + 1) * 4);
    for gid in 0..count {
        locations.extend_from_slice(&(outlines.len() as u32).to_be_bytes());
        if selected.contains(&(gid as u16)) {
            outlines.extend_from_slice(
                glyf.get(offset(gid)?..offset(gid + 1)?)
                    .ok_or("잘못된 TrueType 글리프 범위")?,
            );
            while outlines.len() % 4 != 0 {
                outlines.push(0);
            }
        }
    }
    locations.extend_from_slice(&(outlines.len() as u32).to_be_bytes());
    let mut kept: BTreeMap<[u8; 4], Vec<u8>> = BTreeMap::new();
    for tag in [
        *b"head", *b"hhea", *b"maxp", *b"hmtx", *b"cmap", *b"name", *b"OS/2", *b"post", *b"cvt ",
        *b"fpgm", *b"prep", *b"gasp",
    ] {
        if let Some(value) = tables.get(&tag) {
            kept.insert(tag, value.to_vec());
        }
    }
    let head = kept.get_mut(b"head").ok_or("TrueType head 누락")?;
    head.get_mut(8..12).ok_or("잘린 TrueType head")?.fill(0);
    head[50..52].copy_from_slice(&1u16.to_be_bytes());
    // PDF uses explicit GIDs; retaining every PostScript glyph name is wasteful.
    if let Some(post) = kept.get_mut(b"post")
        && post.len() >= 32
    {
        post.truncate(32);
        post[..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
    }
    kept.insert(*b"glyf", outlines);
    kept.insert(*b"loca", locations);
    let count = kept.len() as u16;
    let power = 1u16 << count.ilog2();
    let mut output = Vec::new();
    output.extend_from_slice(&[0, 1, 0, 0]);
    for value in [
        count,
        power * 16,
        count.ilog2() as u16,
        count * 16 - power * 16,
    ] {
        output.extend_from_slice(&value.to_be_bytes());
    }
    output.resize(12 + usize::from(count) * 16, 0);
    let mut head_offset = 0;
    for (index, (tag, bytes)) in kept.iter().enumerate() {
        let record = 12 + index * 16;
        let offset = output.len();
        output[record..record + 4].copy_from_slice(tag);
        output[record + 4..record + 8].copy_from_slice(&checksum(bytes).to_be_bytes());
        output[record + 8..record + 12].copy_from_slice(&(offset as u32).to_be_bytes());
        output[record + 12..record + 16].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
        output.extend_from_slice(bytes);
        while output.len() % 4 != 0 {
            output.push(0);
        }
        if tag == b"head" {
            head_offset = offset;
        }
    }
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&output));
    output[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use skrifa::{
        FontRef, MetadataProvider,
        instance::{LocationRef, Size},
    };
    #[test]
    fn retained_composite_outlines_metrics_and_font_checksums_match() {
        let fonts = eframe::egui::FontDefinitions::default();
        let bytes = &fonts.font_data["Ubuntu-Light"].font;
        let original = FontRef::from_index(bytes, 0).unwrap();
        let glyphs: Vec<_> = "AéÅﬃ"
            .chars()
            .map(|c| original.charmap().map(c).unwrap())
            .collect();
        let subset = subset(bytes, glyphs.iter().map(|g| g.to_u32() as u16)).unwrap();
        assert!(subset.len() < bytes.len() / 3);
        assert_eq!(checksum(&subset), 0xB1B0_AFBA);
        let font = FontRef::from_index(&subset, 0).unwrap();
        for gid in glyphs {
            let mut a = super::super::PdfPen::default();
            let mut b = super::super::PdfPen::default();
            let settings =
                skrifa::outline::DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
            original
                .outline_glyphs()
                .get(gid)
                .unwrap()
                .draw(settings, &mut a)
                .unwrap();
            font.outline_glyphs()
                .get(gid)
                .unwrap()
                .draw(
                    skrifa::outline::DrawSettings::unhinted(
                        Size::unscaled(),
                        LocationRef::default(),
                    ),
                    &mut b,
                )
                .unwrap();
            assert_eq!(a.path, b.path);
            assert_eq!(
                original
                    .glyph_metrics(Size::unscaled(), LocationRef::default())
                    .advance_width(gid),
                font.glyph_metrics(Size::unscaled(), LocationRef::default())
                    .advance_width(gid)
            );
        }
    }
}
