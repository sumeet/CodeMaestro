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

table! {
    service_configs (id) {
        id -> Int4,
        nickname -> Text,
        instance_id -> Int4,
        service_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        config -> Jsonb,
    }
}

allow_tables_to_appear_in_same_query!(
    codes,
    service_configs,
);
