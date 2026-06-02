mod common;

use cityjson_index::resolve_dataset;
use common::cityjsonseq_root;

#[test]
fn cityjson_seq_layout_autodetects_city_jsonl_stream() {
    let resolved =
        resolve_dataset(&cityjsonseq_root(), None).expect("CityJSONSeq layout should resolve");

    assert_eq!(
        serde_json::to_value(resolved.layout).expect("layout should serialize"),
        "cityjson-seq"
    );
}
