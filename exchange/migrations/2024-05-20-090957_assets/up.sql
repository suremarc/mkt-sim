CREATE TABLE IF NOT EXISTS equities (
    id INTEGER NOT NULL PRIMARY KEY,
    asset_id INTEGER NOT NULL UNIQUE GENERATED ALWAYS AS (id | 0x00000000) STORED,
    ticker TEXT NOT NULL UNIQUE,
    description TEXT,
    created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS equity_options (
    id INTEGER NOT NULL PRIMARY KEY,
    asset_id INTEGER NOT NULL UNIQUE GENERATED ALWAYS AS (id | 0x01000000) STORED,
    underlying UNSIGNED INTEGER NOT NULL,
    expiration_date DATE NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price INT NOT NULL,
    created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(underlying, expiration_date, contract_type, strike_price),
    FOREIGN KEY(underlying) REFERENCES equities(id)
);
