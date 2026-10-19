//! Embedded image metadata: XMP (APP1) and IPTC-IIM (APP13) in every served JPEG.
//!
//! > *"For with God nothing shall be impossible."* - Luke 1:37, KJV
//!
//! WHY in the file and not only on the page: an image travels without its page. Saved,
//! shared or re-posted, each image still says who published it, that the artwork itself
//! is public domain (no copyright is claimed in it), what notice covers the site's own
//! work, and which Scripture it illustrates - in the two metadata blocks that Photoshop,
//! Lightroom, exiftool, Google Images and the operating system file panels all read.
//!
//! The segments are written by hand (no codec involved): existing XMP and Photoshop
//! segments are dropped, and ours are inserted after the JFIF header, so the pixels and
//! every other segment are byte-identical to the derivative.

use crate::{Error, Result};

/// What one image says about itself.
#[derive(Debug, Clone)]
pub struct ImageMeta {
    /// Work title.
    pub title: String,
    /// Artist with life dates.
    pub creator: String,
    /// Year or date of the work.
    pub date_created: String,
    /// Caption: scene, passage and verses (KJV, verbatim).
    pub description: String,
    /// Rights statement (truthful public-domain status plus the site notice).
    pub rights: String,
    /// The same statement within IIM's 128-byte copyright-notice cap.
    pub rights_short: String,
    /// Usage terms.
    pub usage_terms: String,
    /// Credit line.
    pub credit: String,
    /// Where the artwork came from (Commons file page).
    pub source: String,
    /// Commons file title, recorded as the artwork's source inventory id.
    pub source_id: String,
    /// The site page that states the licences.
    pub web_statement: String,
    /// The image's own page on the site.
    pub page_url: String,
    /// Search keywords.
    pub keywords: Vec<String>,
}

/// XML-escapes text for an XMP attribute or element.
#[must_use]
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn alt(tag: &str, value: &str) -> String {
    format!(
        "<{tag}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{tag}>",
        xml_escape(value)
    )
}

/// Builds the XMP packet.
#[must_use]
pub fn xmp_packet(m: &ImageMeta) -> String {
    let e = xml_escape;
    let keywords = m.keywords.iter().fold(String::new(), |mut acc, k| {
        acc.push_str("<rdf:li>");
        acc.push_str(&e(k));
        acc.push_str("</rdf:li>");
        acc
    });
    format!(
        concat!(
            "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>",
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">",
            "<rdf:Description rdf:about=\"\"",
            " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
            " xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"",
            " xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"",
            " xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\"",
            " xmlns:Iptc4xmpExt=\"http://iptc.org/std/Iptc4xmpExt/2008-02-29/\"",
            " photoshop:Credit=\"{credit}\" photoshop:Source=\"{source}\" photoshop:DateCreated=\"{date}\"",
            " xmpRights:Marked=\"False\" xmpRights:WebStatement=\"{web}\">",
            "{title}{desc}{rights}{terms}",
            "<dc:creator><rdf:Seq><rdf:li>{creator}</rdf:li></rdf:Seq></dc:creator>",
            "<dc:subject><rdf:Bag>{keywords}</rdf:Bag></dc:subject>",
            "<Iptc4xmpCore:CreatorContactInfo rdf:parseType=\"Resource\"><Iptc4xmpCore:CiUrlWork>{page}</Iptc4xmpCore:CiUrlWork></Iptc4xmpCore:CreatorContactInfo>",
            "<Iptc4xmpExt:ArtworkOrObject><rdf:Bag><rdf:li rdf:parseType=\"Resource\">",
            "{ao_title}",
            "<Iptc4xmpExt:AOCreator><rdf:Seq><rdf:li>{creator}</rdf:li></rdf:Seq></Iptc4xmpExt:AOCreator>",
            "<Iptc4xmpExt:AODateCreated>{date}</Iptc4xmpExt:AODateCreated>",
            "<Iptc4xmpExt:AOSource>Wikimedia Commons</Iptc4xmpExt:AOSource>",
            "<Iptc4xmpExt:AOSourceInvNo>{source_id}</Iptc4xmpExt:AOSourceInvNo>",
            "<Iptc4xmpExt:AOCopyrightNotice>Public domain</Iptc4xmpExt:AOCopyrightNotice>",
            "</rdf:li></rdf:Bag></Iptc4xmpExt:ArtworkOrObject>",
            "</rdf:Description></rdf:RDF></x:xmpmeta>",
            "<?xpacket end=\"w\"?>"
        ),
        credit = e(&m.credit),
        source = e(&m.source),
        date = e(&m.date_created),
        web = e(&m.web_statement),
        title = alt("dc:title", &m.title),
        desc = alt("dc:description", &m.description),
        rights = alt("dc:rights", &m.rights),
        terms = alt("xmpRights:UsageTerms", &m.usage_terms),
        creator = e(&m.creator),
        keywords = keywords,
        page = e(&m.page_url),
        ao_title = alt("Iptc4xmpExt:AOTitle", &m.title),
        source_id = e(&m.source_id),
    )
}

/// Truncates to at most `max` bytes on a character boundary (IIM fields have byte caps).
#[must_use]
pub fn cap(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.get(..end).unwrap_or("")
}

fn iim(out: &mut Vec<u8>, record: u8, dataset: u8, value: &[u8]) -> Result<()> {
    let len =
        u16::try_from(value.len()).map_err(|_| Error::Data("IIM field over 32 KB".to_string()))?;
    out.extend_from_slice(&[0x1C, record, dataset]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

/// Builds the IPTC-IIM block (UTF-8 declared) wrapped as Photoshop resource 0x0404.
///
/// # Errors
/// Fails only if a field exceeds the IIM two-byte length.
pub fn iptc_resource(m: &ImageMeta) -> Result<Vec<u8>> {
    let mut iim_bytes = Vec::new();
    iim(&mut iim_bytes, 1, 90, b"\x1B%G")?; // coded character set: UTF-8
    iim(&mut iim_bytes, 2, 0, &[0, 4])?; // record version 4
    iim(&mut iim_bytes, 2, 5, cap(&m.title, 64).as_bytes())?; // object name
    for k in &m.keywords {
        iim(&mut iim_bytes, 2, 25, cap(k, 64).as_bytes())?; // keywords
    }
    if m.date_created.len() == 4 && m.date_created.chars().all(|c| c.is_ascii_digit()) {
        // IIM dates are CCYYMMDD; 00 marks an unknown month and day.
        iim(
            &mut iim_bytes,
            2,
            55,
            format!("{}0000", m.date_created).as_bytes(),
        )?;
    }
    iim(&mut iim_bytes, 2, 80, cap(&m.creator, 32).as_bytes())?; // by-line
    iim(&mut iim_bytes, 2, 110, cap(&m.credit, 32).as_bytes())?; // credit
    iim(&mut iim_bytes, 2, 115, cap(&m.source, 32).as_bytes())?; // source
    iim(&mut iim_bytes, 2, 116, cap(&m.rights_short, 128).as_bytes())?; // copyright notice
    iim(&mut iim_bytes, 2, 120, cap(&m.description, 2000).as_bytes())?; // caption
    let mut res = Vec::new();
    res.extend_from_slice(b"8BIM");
    res.extend_from_slice(&0x0404u16.to_be_bytes());
    res.extend_from_slice(&[0, 0]); // empty Pascal name, padded to even
    let size = u32::try_from(iim_bytes.len())
        .map_err(|_| Error::Data("IPTC block too large".to_string()))?;
    res.extend_from_slice(&size.to_be_bytes());
    res.extend_from_slice(&iim_bytes);
    if iim_bytes.len() % 2 == 1 {
        res.push(0);
    }
    Ok(res)
}

const XMP_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PS_HEADER: &[u8] = b"Photoshop 3.0\0";

fn segment(marker: u8, payload_parts: &[&[u8]]) -> Result<Vec<u8>> {
    let body: usize = payload_parts.iter().map(|p| p.len()).sum();
    let len = u16::try_from(body + 2)
        .map_err(|_| Error::Data(format!("JPEG segment FF{marker:02X} over 64 KB")))?;
    let mut out = vec![0xFF, marker];
    out.extend_from_slice(&len.to_be_bytes());
    for p in payload_parts {
        out.extend_from_slice(p);
    }
    Ok(out)
}

/// Returns a copy of `jpeg` with our XMP and IPTC segments and without any previous ones.
///
/// # Errors
/// Fails when the bytes are not a JPEG or a segment length runs past the end.
pub fn embed(jpeg: &[u8], m: &ImageMeta) -> Result<Vec<u8>> {
    if jpeg.get(..2) != Some(&[0xFF, 0xD8][..]) {
        return Err(Error::Data("not a JPEG (no SOI)".to_string()));
    }
    let xmp = xmp_packet(m);
    let app1 = segment(0xE1, &[XMP_HEADER, xmp.as_bytes()])?;
    let app13 = segment(0xED, &[PS_HEADER, &iptc_resource(m)?])?;
    let mut out = Vec::with_capacity(jpeg.len() + app1.len() + app13.len());
    out.extend_from_slice(&[0xFF, 0xD8]);
    let mut pos = 2usize;
    let mut inserted = false;
    loop {
        let marker = jpeg
            .get(pos..pos + 2)
            .ok_or_else(|| Error::Data("truncated JPEG".to_string()))?;
        if marker.first() != Some(&0xFF) {
            return Err(Error::Data(format!("bad JPEG marker at byte {pos}")));
        }
        let kind = marker.get(1).copied().unwrap_or(0);
        if kind == 0xDA || kind == 0xD9 {
            // Start of scan: insert before it if no APP0 came first, then copy the rest.
            if !inserted {
                out.extend_from_slice(&app1);
                out.extend_from_slice(&app13);
            }
            out.extend_from_slice(jpeg.get(pos..).unwrap_or(&[]));
            return Ok(out);
        }
        let len_bytes = jpeg
            .get(pos + 2..pos + 4)
            .ok_or_else(|| Error::Data("truncated JPEG length".to_string()))?;
        let len = usize::from(u16::from_be_bytes([
            len_bytes.first().copied().unwrap_or(0),
            len_bytes.get(1).copied().unwrap_or(0),
        ]));
        let seg = jpeg
            .get(pos..pos + 2 + len)
            .ok_or_else(|| Error::Data("JPEG segment runs past the end".to_string()))?;
        let payload = seg.get(4..).unwrap_or(&[]);
        let is_old_xmp = kind == 0xE1 && payload.starts_with(XMP_HEADER);
        let is_old_ps = kind == 0xED && payload.starts_with(PS_HEADER);
        if !inserted && kind != 0xE0 {
            out.extend_from_slice(&app1);
            out.extend_from_slice(&app13);
            inserted = true;
        }
        if !is_old_xmp && !is_old_ps {
            out.extend_from_slice(seg);
        }
        pos += 2 + len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ImageMeta {
        ImageMeta {
            title: "The Creation of Light".into(),
            creator: "Gustave Doré (1832–1883)".into(),
            date_created: "1866".into(),
            description: "\"For with God nothing shall be impossible.\" - Luke 1:37 (KJV)".into(),
            rights: "Artwork: public domain.".into(),
            rights_short: "Artwork public domain.".into(),
            usage_terms: "Public domain".into(),
            credit: "The Scriptures, Goroshi LLC".into(),
            source: "Wikimedia Commons".into(),
            source_id: "File:001.The Creation of Light.jpg".into(),
            web_statement: "https://example.org/about/".into(),
            page_url: "https://example.org/art/x/".into(),
            keywords: vec!["Bible".into()],
        }
    }

    #[test]
    fn embed_inserts_after_jfif_and_keeps_the_scan() {
        // SOI, APP0(JFIF, len 16), SOS(len 2), data, EOI
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02, 0x12, 0x34, 0xFF, 0xD9]);
        let out = embed(&jpeg, &sample()).unwrap_or_default();
        assert_eq!(out.get(..2), Some(&[0xFF, 0xD8][..]));
        assert_eq!(out.get(2..4), Some(&[0xFF, 0xE0][..]));
        assert_eq!(out.get(20..22), Some(&[0xFF, 0xE1][..]));
        assert!(out.ends_with(&[0xFF, 0xDA, 0x00, 0x02, 0x12, 0x34, 0xFF, 0xD9]));
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("For with God nothing shall be impossible."));
        assert!(text.contains("The Scriptures, Goroshi LLC"));
    }

    #[test]
    fn cap_never_splits_a_character() {
        assert_eq!(cap("Doré", 4), "Dor");
        assert_eq!(cap("abc", 10), "abc");
    }
}
