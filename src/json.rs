//! Pure formatting helpers for Anthology JSON.

use std::sync::LazyLock;

/// Collapse string-valued objects in multiline `names` arrays to one line each.
/// Other whitespace and object properties remain byte-for-byte unchanged.
pub fn collapse_names(input: &[u8]) -> Vec<u8> {
    static ARR: LazyLock<regex::bytes::Regex> = LazyLock::new(|| {
        regex::bytes::Regex::new(r#"(?s)("names":\s*\[\n)(.*?)(\n[ \t]*\])"#).unwrap()
    });

    static OBJ: LazyLock<regex::bytes::Regex> =
        LazyLock::new(|| regex::bytes::Regex::new(r"(?s)\{\n(.*?)\n[ \t]*\}").unwrap());

    static PAIR: LazyLock<regex::bytes::Regex> = LazyLock::new(|| {
        regex::bytes::Regex::new(r#""(?:[^"\\]|\\.)*":\s*"(?:[^"\\]|\\.)*""#).unwrap()
    });
    ARR.replace_all(input, |a: &regex::bytes::Captures<'_>| {
        let body = OBJ.replace_all(&a[2], |o: &regex::bytes::Captures<'_>| {
            let mut out = vec![b'{'];
            for (i, p) in PAIR.find_iter(&o[1]).enumerate() {
                if i > 0 {
                    out.extend_from_slice(b", ");
                }
                out.extend_from_slice(p.as_bytes());
            }
            out.push(b'}');
            out
        });
        let mut out = a[1].to_vec();
        out.extend_from_slice(&body);
        out.extend_from_slice(&a[3]);
        out
    })
    .into_owned()
}
