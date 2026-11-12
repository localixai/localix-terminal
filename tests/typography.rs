//! The type scale of docs/ui.md, checked on the stylesheets of the app's own pages: a size comes
//! from the scale and names its own line height in whole points, so a column of text keeps one
//! rhythm. A size with a ratio, or with a line inherited from another style, would drift.

// The helpers sit in a test module, where the workspace lints let a failed read panic.
#[cfg(test)]
mod scale {
    use std::fs;

    /// The sizes of the scale, in points (the line each takes is in the table of docs/ui.md).
    const SIZES: [u32; 6] = [10, 11, 12, 13, 15, 22];

    const PAGES: [&str; 5] = [
        "src/browser/frame.css",
        "src/browser/tree.css",
        "src/browser/strip.html",
        "src/browser/settings.html",
        "src/board/page.html",
    ];

    /// The declaration blocks of a page's own styles: the text between `{` and `}` with none inside.
    fn blocks(page: &str) -> Vec<String> {
        let text = fs::read_to_string(page).unwrap();
        let styles = if page.ends_with(".css") {
            text
        } else {
            text.split("<style>")
                .skip(1)
                .filter_map(|part| part.split("</style>").next())
                .collect()
        };
        let mut out = Vec::new();
        let mut open = None;
        for (at, c) in styles.char_indices() {
            match c {
                '{' => open = Some(at + 1),
                '}' => {
                    if let Some(from) = open.take() {
                        out.push(styles.get(from..at).unwrap_or_default().to_string());
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The value of property `name` in a block, when it has one (`font-size`, not `font-size-adjust`).
    fn value<'a>(block: &'a str, name: &str) -> Option<&'a str> {
        block.split(';').find_map(|declaration| {
            let (key, value) = declaration.split_once(':')?;
            (key.trim() == name).then_some(value.trim())
        })
    }

    fn points(text: &str) -> Option<u32> {
        text.strip_suffix("px")?.parse().ok()
    }

    /// The size and line height a block names: from `font-size` and `line-height`, or from a `font`
    /// shorthand's `size/line`. A size that is a variable (the terminal's own) is not the scale's.
    fn text(block: &str) -> Option<(u32, Option<String>)> {
        if let Some(size) = value(block, "font-size") {
            return Some((points(size)?, value(block, "line-height").map(String::from)));
        }
        let font = value(block, "font")?;
        // A shorthand's size is the word that ends in `px`, with its line after a slash.
        font.split_whitespace().find_map(|word| {
            let (size, line) = match word.split_once('/') {
                Some((size, line)) => (size, Some(line.to_string())),
                None => (word, None),
            };
            Some((points(size)?, line))
        })
    }

    #[test]
    fn every_size_is_on_the_scale_and_has_a_line_of_whole_points() {
        let mut wrong = Vec::new();
        for page in PAGES {
            for block in blocks(page) {
                let Some((size, line)) = text(&block) else {
                    continue;
                };
                if !SIZES.contains(&size) {
                    wrong.push(format!("{page}: {size}px is not on the scale: {block}"));
                }
                // A glyph in a fixed box (a button's sign) sets its line to 1; a control's text may
                // take its box's height. A size with none would inherit another style's ratio.
                if line.is_none() {
                    wrong.push(format!("{page}: {size}px has no line height: {block}"));
                }
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn no_line_height_is_a_ratio_and_nothing_is_tracked_by_hand() {
        let mut wrong = Vec::new();
        for page in PAGES {
            for block in blocks(page) {
                if let Some(line) = value(&block, "line-height") {
                    // A size that follows the person's setting (the code preview) adds to it.
                    if line != "1" && points(line).is_none() && !line.starts_with("calc(") {
                        wrong.push(format!("{page}: line-height {line}: {block}"));
                    }
                }
                if value(&block, "letter-spacing").is_some() {
                    wrong.push(format!("{page}: letter-spacing: {block}"));
                }
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
