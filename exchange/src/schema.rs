// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Binary,
        email -> Text,
        password -> Text,
        role_flags -> BigInt,
    }
}
