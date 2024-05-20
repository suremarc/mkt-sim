CREATE TABLE roles (
    name TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE users (
    id TEXT NOT NULL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password BLOB NOT NULL
);

CREATE TABLE user_roles (
    user_id INTEGER NOT NULL,
    role_name TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id),
    FOREIGN KEY(role_name) REFERENCES roles(name)
);

CREATE TABLE role_permissions (
    role_name TEXT NOT NULL,
    scope TEXT NOT NULL,
    admin INTEGER NOT NULL,
    FOREIGN KEY(role_name) REFERENCES roles(name)
);
