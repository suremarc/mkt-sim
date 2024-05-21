CREATE TABLE IF NOT EXISTS equities (
    id INTEGER NOT NULL PRIMARY KEY,
    created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ticker TEXT NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS equity_options (
    underlying INTEGER NOT NULL,
    expiration_date DATE NOT NULL,
    contract_type TEXT NOT NULL,
    strike_price INT NOT NULL,
    exercise_style TEXT NOT NULL,
    created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(underlying, expiration_date, contract_type, strike_price, exercise_style),
    FOREIGN KEY(underlying) REFERENCES equities(id)
);
