table! {
    codes (id) {
        id -> Int4,
        added_by -> Text,
        code -> Json,
        instance_id -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
