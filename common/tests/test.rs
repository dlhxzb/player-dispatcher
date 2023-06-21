use common::AABB;

#[test]
fn test_aabb() {
    let aabb1 = AABB {
        xmin: 0.0,
        xmax: 100.0,
        ymin: -100.0,
        ymax: 0.0,
    };

    assert!(aabb1.contains(100.0, -100.0));
    assert!(!aabb1.contains(100.1, -100.0));

    let aabb2 = AABB {
        xmin: -10.0,
        xmax: 120.0,
        ymin: -50.0,
        ymax: 1.0,
    };
    assert_eq!(
        aabb1.get_intersection(&aabb2).unwrap(),
        AABB {
            xmin: -0.0,
            xmax: 100.0,
            ymin: -50.0,
            ymax: 0.0,
        }
    );

    let aabb3 = AABB {
        xmin: -10.0,
        xmax: -5.0,
        ymin: 5.0,
        ymax: 100.0,
    };
    assert!(aabb1.get_intersection(&aabb3).is_none());
}
