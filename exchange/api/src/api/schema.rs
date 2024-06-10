// @generated automatically by Diesel CLI.

diesel::table! {
    equities (id) {
        id -> Int4,
        ticker -> Text,
        description -> Nullable<Text>,
        created -> Timestamptz,
    }
}

diesel::table! {
    equity_options (id) {
        id -> Int4,
        underlying -> Int4,
        expiration_date -> Date,
        contract_type -> Text,
        strike_price -> Int4,
        created -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        email -> Text,
        password -> Text,
        role_flags -> Int8,
    }
}

diesel::joinable!(equity_options -> equities (underlying));

diesel::allow_tables_to_appear_in_same_query!(
    equities,
    equity_options,
    users,
);
