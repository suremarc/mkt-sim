CREATE TABLE users (
    id BINARY(128) NOT NULL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    role_flags BIGINT NOT NULL
);
