use super::{BlobRadii, BlobRadius};

#[test]
fn either_order_spans_the_same_range() {
    let forward = BlobRadii::spanning([BlobRadius(3), BlobRadius(9)]);
    let backward = BlobRadii::spanning([BlobRadius(9), BlobRadius(3)]);
    assert_eq!(forward, backward);
    assert_eq!(
        (forward.min(), forward.max()),
        (BlobRadius(3), BlobRadius(9))
    );
}
