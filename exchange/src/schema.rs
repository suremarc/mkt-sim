// @generated automatically by Diesel CLI.

diesel::table! {
    equities (id) {
        id -> Integer,
        ticker -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    equity_options (underlying, expiration_date, contract_type, strike_price, exercise_style) {
        underlying -> Integer,
        expiration_date -> Date,
        contract_type -> Text,
        strike_price -> Integer,
        exercise_style -> Text,
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
