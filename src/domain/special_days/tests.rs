use super::{SpecialDay, SpecialDayKind, SpecialDayRepository};

#[test]
fn special_day_uses_local_canonical_id_and_kind() {
    let day = SpecialDay::new(
        "special-1".to_string(),
        "user-1".to_string(),
        "2026-05-02".to_string(),
        SpecialDayKind::Illness,
    );

    assert_eq!(day.special_day_id, "special-1");
    assert_eq!(day.user_id, "user-1");
    assert_eq!(day.date, "2026-05-02");
    assert_eq!(day.kind, SpecialDayKind::Illness);
}

fn assert_special_day_repository<T: SpecialDayRepository>() {}

#[test]
fn special_day_repository_trait_is_usable() {
    assert_special_day_repository::<super::ports::NoopSpecialDayRepository>();
}
