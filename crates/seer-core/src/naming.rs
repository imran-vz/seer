//! Clean names for film and episode files.
//!
//! Release names carry quality, codec, language and site tags that a media
//! library does not want. This works out the title and year from such a name
//! and writes them the way Jellyfin and Plex expect:
//! `Title (Year).ext`, `Title (Year) [imdbid-tt1234567].ext`, `Show S01E02.ext`.
//! It runs offline and never invents anything that is not in the name.

/// What a file name says about the film or episode it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub title: String,
    pub year: Option<u16>,
    /// An IMDb id such as "tt1375666". Only ever copied from the file name.
    pub imdb: Option<String>,
    /// Season and episode, for series.
    pub episode: Option<(u32, u32)>,
}

impl Identity {
    /// The library-friendly file name for this identity.
    pub fn file_name(&self, extension: Option<&str>) -> String {
        let mut name = sanitize(&self.title);
        match self.episode {
            Some((season, episode)) => name = format!("{name} S{season:02}E{episode:02}"),
            None => {
                if let Some(year) = self.year {
                    name = format!("{name} ({year})");
                }
                if let Some(imdb) = &self.imdb {
                    name = format!("{name} [imdbid-{imdb}]");
                }
            }
        }
        match extension {
            Some(ext) => format!("{name}.{ext}"),
            None => name,
        }
    }
}

/// A cleaner name for `file_name`, or `None` when it is already clean or
/// nothing recognisable can be taken from it.
pub fn suggest(file_name: &str) -> Option<String> {
    let (stem, extension) = split_extension(file_name);
    let suggestion = parse(stem)?.file_name(extension);
    (suggestion != file_name).then_some(suggestion)
}

/// Reads title, year, IMDb id and episode numbers out of a release name
/// (without its extension). `None` if there is no marker to cut the title at.
pub fn parse(stem: &str) -> Option<Identity> {
    let tokens = tokenize(stem);
    let imdb = tokens.iter().find(|t| is_imdb_id(t)).map(|t| {
        let id = t.to_lowercase();
        id.trim_start_matches("imdbid-")
            .trim_start_matches("imdb-")
            .to_owned()
    });

    // A title ends at the first of: an episode marker, the year, a release tag.
    let episode = tokens
        .iter()
        .enumerate()
        .find_map(|(i, t)| episode_marker(t).map(|e| (i, e)));
    let first_tag = tokens
        .iter()
        .position(|t| is_release_tag(t) || is_imdb_id(t));
    let limit = first_tag.unwrap_or(tokens.len());
    // The last year before any release tag, so "Blade Runner 2049 2017" keeps
    // 2049 in the title. A year in first place is the title itself ("1917").
    let year = tokens[..limit]
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, t)| as_year(t).map(|y| (i, y)))
        .next_back();

    let (end, year, episode) = match (episode, year, first_tag) {
        (Some((i, numbers)), _, _) if i > 0 => (i, None, Some(numbers)),
        (_, Some((i, year)), _) => (i, Some(year), None),
        (_, None, Some(i)) if i > 0 => (i, None, None),
        _ => return None,
    };

    let title = tokens[..end]
        .iter()
        .filter(|t| !is_separator(t))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let title = tidy_title(&title);
    (!title.is_empty()).then_some(Identity {
        title,
        year,
        imdb,
        episode,
    })
}

fn split_extension(file_name: &str) -> (&str, Option<&str>) {
    match file_name.rsplit_once('.') {
        // Extensions are short and alphanumeric; anything else is part of the name.
        Some((stem, ext))
            if !stem.is_empty()
                && (1..=5).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            (stem, Some(ext))
        }
        _ => (file_name, None),
    }
}

/// Splits a release name into words, dropping brackets and a leading site tag.
fn tokenize(stem: &str) -> Vec<String> {
    // Names written with dots or underscores instead of spaces.
    let dotted = !stem.contains(' ');
    let mut words: Vec<String> = Vec::new();
    for raw in stem.split(|c: char| c.is_whitespace() || c == '_') {
        if is_site(raw, dotted) {
            continue;
        }
        let cleaned: String = raw
            .chars()
            .map(|c| if "()[]{}".contains(c) { ' ' } else { c })
            .collect();
        for word in cleaned.split_whitespace() {
            match dotted {
                true => words.extend(split_dotted(word)),
                false => words.push(word.to_owned()),
            }
        }
    }
    // A site tag usually leaves a dash behind it.
    let start = words
        .iter()
        .position(|w| !is_separator(w))
        .unwrap_or(words.len());
    words.split_off(start)
}

/// Splits on dots, except inside a channel count such as "5.1" in "DD5.1".
fn split_dotted(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let digit = |i: usize| chars.get(i).is_some_and(char::is_ascii_digit);
    let mut parts = vec![String::new()];
    for (i, &c) in chars.iter().enumerate() {
        // One digit either side of the dot, and no more: "5.1", not "2049.2017".
        let channel_count =
            i >= 1 && digit(i - 1) && digit(i + 1) && !digit(i + 2) && !(i >= 2 && digit(i - 2));
        if c == '.' && !channel_count {
            parts.push(String::new());
        } else if let Some(last) = parts.last_mut() {
            last.push(c);
        }
    }
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

/// A web address, as release sites stamp on the front of a name. In a
/// dotted name only an explicit "www." counts, since every word has dots.
fn is_site(word: &str, dotted: bool) -> bool {
    let word = word
        .trim_matches(|c: char| "()[]{}".contains(c))
        .to_lowercase();
    word.starts_with("www.")
        || (!dotted
            && [".com", ".net", ".org", ".io", ".to", ".cc", ".tv", ".me"]
                .iter()
                .any(|tld| word.ends_with(tld) && word.len() > tld.len() + 1))
}

fn is_separator(word: &str) -> bool {
    word.chars().all(|c| "-–—|~+&,.".contains(c))
}

fn as_year(word: &str) -> Option<u16> {
    let year: u16 = (word.len() == 4).then(|| word.parse().ok()).flatten()?;
    (1900..=2099).contains(&year).then_some(year)
}

fn is_imdb_id(word: &str) -> bool {
    let word = word.to_lowercase();
    let digits = word
        .trim_start_matches("imdbid-")
        .trim_start_matches("imdb-");
    match digits.strip_prefix("tt") {
        Some(number) => {
            (7..=9).contains(&number.len()) && number.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// "S01E02", "s1e2" or "1x02".
fn episode_marker(word: &str) -> Option<(u32, u32)> {
    let word = word.to_lowercase();
    let (season, episode) = match word.strip_prefix('s') {
        Some(rest) => rest.split_once('e')?,
        None => word.split_once('x')?,
    };
    let number = |text: &str| -> Option<u32> {
        ((1..=3).contains(&text.len()) && text.chars().all(|c| c.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    Some((number(season)?, number(episode)?))
}

/// Words that describe the release, not the film.
fn is_release_tag(word: &str) -> bool {
    let word = word.to_lowercase();
    const TAGS: &[&str] = &[
        "480p",
        "576p",
        "720p",
        "1080p",
        "1080i",
        "1440p",
        "2160p",
        "4320p",
        "4k",
        "8k",
        "uhd",
        "hd",
        "hq",
        "sd",
        "hdr",
        "hdr10",
        "hdr10+",
        "dv",
        "dovi",
        "sdr",
        "10bit",
        "8bit",
        "bluray",
        "blu-ray",
        "bdrip",
        "brrip",
        "bdremux",
        "remux",
        "webrip",
        "web-dl",
        "webdl",
        "web",
        "hdrip",
        "dvdrip",
        "dvdscr",
        "hdtv",
        "hdcam",
        "cam",
        "hdts",
        "ts",
        "tc",
        "predvd",
        "pre-dvd",
        "proper",
        "repack",
        "extended",
        "unrated",
        "remastered",
        "imax",
        "x264",
        "x265",
        "h264",
        "h265",
        "h.264",
        "h.265",
        "hevc",
        "avc",
        "av1",
        "xvid",
        "divx",
        "vp9",
        "aac",
        "ac3",
        "eac3",
        "dd",
        "dd+",
        "ddp",
        "dts",
        "dts-hd",
        "truehd",
        "atmos",
        "flac",
        "mp3",
        "opus",
        "esub",
        "esubs",
        "msub",
        "msubs",
        "subs",
        "dual",
        "multi",
        "dubbed",
        "hindi",
        "tamil",
        "telugu",
        "malayalam",
        "kannada",
        "english",
        "bengali",
        "marathi",
        "punjabi",
        "korean",
        "japanese",
        "french",
        "german",
        "spanish",
        "italian",
        "org",
        "untouched",
        "true",
        "amzn",
        "nf",
        "dsnp",
        "hmax",
        "atvp",
        "jiohs",
        "zee5",
    ];
    TAGS.contains(&word.as_str())
        || word.starts_with("dd5") || word.starts_with("ddp5") || word.starts_with("aac2") || word.starts_with("aac5")
        // Sizes and rates: "2.7gb", "700mb", "384kbps".
        || ["gb", "mb", "kbps", "mbps"].iter().any(|unit| {
            word.strip_suffix(unit)
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.'))
        })
}

fn tidy_title(title: &str) -> String {
    let title = title.trim_matches(|c: char| c.is_whitespace() || "-–—|~,.".contains(c));
    // Dotted release names are often all lower case.
    if title.chars().any(char::is_uppercase) {
        return title.to_owned();
    }
    title
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Removes what a file name cannot hold on macOS or in a library path.
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c == '/' || c == ':' { '-' } else { c })
        .collect();
    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(name: &str) -> Option<String> {
        suggest(name)
    }

    #[test]
    fn strips_release_details() {
        assert_eq!(
            clean("Yezhu Kadal Yezhu Malai (2026) Tamil HQ HDRip - 1080p - x264 - (DD5.1 - 384Kbps & AAC) - 2.7GB - ESub.mkv").as_deref(),
            Some("Yezhu Kadal Yezhu Malai (2026).mkv")
        );
        assert_eq!(
            clean("The.Matrix.1999.1080p.BluRay.x264.DTS-HD.mkv").as_deref(),
            Some("The Matrix (1999).mkv")
        );
        assert_eq!(
            clean("inception_2010_720p_webrip.mp4").as_deref(),
            Some("Inception (2010).mp4")
        );
        assert_eq!(
            clean("www.1TamilMV.io - Leo (2023) Tamil TRUE WEB-DL - 4K.mkv").as_deref(),
            Some("Leo (2023).mkv")
        );
    }

    #[test]
    fn years_that_are_part_of_the_title() {
        assert_eq!(
            clean("Blade.Runner.2049.2017.2160p.UHD.mkv").as_deref(),
            Some("Blade Runner 2049 (2017).mkv")
        );
        assert_eq!(
            clean("1917.2019.1080p.BluRay.mkv").as_deref(),
            Some("1917 (2019).mkv")
        );
        assert_eq!(
            clean("2001 A Space Odyssey (1968) 1080p.mkv").as_deref(),
            Some("2001 A Space Odyssey (1968).mkv")
        );
        // No year to find: keep the number as the title.
        assert_eq!(clean("1917.1080p.BluRay.mkv").as_deref(), Some("1917.mkv"));
    }

    #[test]
    fn keeps_an_imdb_id_but_never_invents_one() {
        assert_eq!(
            clean("Inception.2010.tt1375666.1080p.mkv").as_deref(),
            Some("Inception (2010) [imdbid-tt1375666].mkv")
        );
        assert_eq!(parse("Inception 2010 1080p").unwrap().imdb, None);
    }

    #[test]
    fn episodes() {
        assert_eq!(
            clean("Show.Name.S01E02.720p.HDTV.x264.mkv").as_deref(),
            Some("Show Name S01E02.mkv")
        );
        assert_eq!(
            clean("Dark 2x05 German 1080p.mkv").as_deref(),
            Some("Dark S02E05.mkv")
        );
    }

    #[test]
    fn leaves_clean_and_unknown_names_alone() {
        assert_eq!(clean("Leo (2023).mkv"), None);
        assert_eq!(clean("Inception (2010) [imdbid-tt1375666].mkv"), None);
        assert_eq!(clean("holiday video.mp4"), None);
        assert_eq!(clean("IMG_4521.mov"), None);
    }

    #[test]
    fn unsafe_characters() {
        let identity = Identity {
            title: "Mission: Impossible / Fallout".into(),
            year: Some(2018),
            imdb: None,
            episode: None,
        };
        assert_eq!(
            identity.file_name(Some("mkv")),
            "Mission- Impossible - Fallout (2018).mkv"
        );
    }
}
