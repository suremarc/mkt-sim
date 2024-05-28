// @generated automatically by Diesel CLI.

diesel::table! {
    equities (id) {
        id -> Integer,
        asset_id -> Integer,
        ticker -> Text,
        description -> Nullable<Text>,
        created -> Timestamp,
    }
}

diesel::table! {
    equity_options (id) {
        id -> Integer,
        asset_id -> Integer,
        underlying -> Integer,
        expiration_date -> Date,
        contract_type -> Text,
        strike_price -> Integer,
        created -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Binary,
        email -> Text,
        password -> Text,
        role_flags -> BigInt,
    }
}

diesel::joinable!(equity_options -> equities (underlying));

diesel::allow_tables_to_appear_in_same_query!(
    equities,
    equity_options,
    users,
);
