#![allow(
    clippy::doc_markdown,
    reason = "test docstrings use domain terminology plainly"
)]

mod common;

use cityjson_index::resolve_dataset;
use common::cityjsonseq_root;

/// Input: the tracked tests/data/cityjsonseq fixture root containing a .city.jsonl stream.
/// Assertions: dataset resolution serializes the layout kind as cityjson-seq.
#[test]
fn cityjson_seq_layout_autodetects_city_jsonl_stream() {
    let resolved =
        resolve_dataset(&cityjsonseq_root(), None).expect("CityJSONSeq layout should resolve");

    assert_eq!(
        serde_json::to_value(resolved.layout).expect("layout should serialize"),
        "cityjson-seq"
    );
}
